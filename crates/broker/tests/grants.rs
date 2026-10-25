//! C-08 grants against a real PostgreSQL with row-level security.

mod common;

use std::time::Duration;

use chrono::{Duration as Dur, Utc};
use common::Env;
use factory_accounts::{AccountId, ProjectId, Role};
use factory_broker::*;
use uuid::Uuid;

struct World {
    env: Env,
    owner: AccountId,
    maintainer: AccountId,
    contributor: AccountId,
    viewer: AccountId,
    outsider: AccountId,
    project: ProjectId,
}

async fn world() -> World {
    let env = Env::new().await;
    let owner = env.account("owner").await;
    let maintainer = env.account("maintainer").await;
    let contributor = env.account("contributor").await;
    let viewer = env.account("viewer").await;
    let outsider = env.account("outsider").await;
    let project = env.project(owner).await;
    env.member(owner, project, maintainer, Role::Maintainer)
        .await;
    env.member(owner, project, contributor, Role::Contributor)
        .await;
    env.member(owner, project, viewer, Role::Viewer).await;
    World {
        env,
        owner,
        maintainer,
        contributor,
        viewer,
        outsider,
        project,
    }
}

fn new_grant(w: &World, grantor: AccountId, classes: &[OperationClass]) -> NewGrant {
    NewGrant {
        project_id: w.project,
        owner_account_id: w.owner,
        grantor_account_id: grantor,
        task_id: Uuid::new_v4(),
        operation_classes: classes.to_vec(),
        allowed_roots: vec!["common".into(), "events/".into()],
        provider_destinations: vec!["anthropic:https://api.anthropic.com".into()],
        max_cost_micros: 200_000,
        max_turns: 20,
        max_changed_files: 30,
        expires_at: Utc::now() + Dur::hours(1),
        policy_version: "policy-1".into(),
        proposal_sha256: None,
    }
}

async fn issue(
    w: &World,
    grantor: AccountId,
    role: Role,
    classes: &[OperationClass],
) -> Result<Grant, GrantError> {
    let mut tx = w.env.tx(grantor).await;
    let r = issue_grant(&mut tx, new_grant(w, grantor, classes), role, Utc::now()).await;
    if r.is_ok() {
        tx.commit().await.unwrap();
    }
    r
}

use OperationClass::*;

#[tokio::test]
async fn roles_bound_what_a_grantor_can_issue() {
    let w = world().await;
    let editing = [
        ReadSource,
        RunReadOnlyTool,
        WriteTaskWorktree,
        ModelInference,
    ];

    // Owner and maintainer can issue editing classes.
    let g = issue(&w, w.owner, Role::Owner, &editing).await.unwrap();
    assert_eq!(g.revocation_epoch, 1);
    assert_eq!(
        g.allowed_roots,
        vec!["common".to_string(), "events".to_string()]
    );
    assert!(!g.is_proposal_bound());
    issue(&w, w.maintainer, Role::Maintainer, &editing)
        .await
        .unwrap();
    // Contributor: editing yes, publication no.
    issue(&w, w.contributor, Role::Contributor, &editing)
        .await
        .unwrap();
    for class in [
        LocalCommit,
        PushOwnedBranch,
        OpenPullRequest,
        CreateRepository,
        IntegrateDefaultBranch,
    ] {
        let err = issue(&w, w.contributor, Role::Contributor, &[ReadSource, class])
            .await
            .unwrap_err();
        assert!(
            matches!(err, GrantError::ExceedsRole { class: c, .. } if c == class),
            "{class:?}: {err:?}"
        );
    }
    // Viewer: read only.
    issue(&w, w.viewer, Role::Viewer, &[ReadSource, RunReadOnlyTool])
        .await
        .unwrap();
    for class in [WriteTaskWorktree, ModelInference] {
        let err = issue(&w, w.viewer, Role::Viewer, &[class])
            .await
            .unwrap_err();
        assert!(matches!(err, GrantError::ExceedsRole { .. }), "{err:?}");
    }
    // Maintainer may publish but not integrate... Publish yes, ApproveProposal yes per matrix.
    issue(
        &w,
        w.maintainer,
        Role::Maintainer,
        &[PushOwnedBranch, IntegrateDefaultBranch],
    )
    .await
    .unwrap();
    // Owner may issue publication classes too.
    issue(&w, w.owner, Role::Owner, &[CreateRepository])
        .await
        .unwrap();
}

#[tokio::test]
async fn database_rejects_an_overstated_role_and_outsiders() {
    let w = world().await;
    // The contributor claims to be the owner: the policy compares with the membership row.
    let err = issue(&w, w.contributor, Role::Owner, &[ReadSource])
        .await
        .unwrap_err();
    assert!(matches!(err, GrantError::Database(_)), "{err:?}");
    // A grantor other than the caller.
    let mut tx = w.env.tx(w.contributor).await;
    let r = issue_grant(
        &mut tx,
        new_grant(&w, w.owner, &[ReadSource]),
        Role::Owner,
        Utc::now(),
    )
    .await;
    assert!(matches!(r, Err(GrantError::Database(_))), "{r:?}");
    drop(tx);
    // An account that is not a member.
    let mut tx = w.env.tx(w.outsider).await;
    let r = issue_grant(
        &mut tx,
        new_grant(&w, w.outsider, &[ReadSource]),
        Role::Viewer,
        Utc::now(),
    )
    .await;
    assert!(matches!(r, Err(GrantError::Database(_))), "{r:?}");
}

#[tokio::test]
async fn invalid_grants_are_refused() {
    let w = world().await;
    let now = Utc::now();
    let mut tx = w.env.tx(w.owner).await;
    let mut g = new_grant(&w, w.owner, &[ReadSource]);
    g.expires_at = now - Dur::seconds(1);
    assert!(matches!(
        issue_grant(&mut tx, g, Role::Owner, now).await,
        Err(GrantError::Invalid(_))
    ));
    for bad_root in ["../x", "/abs", "a\\b", "", "C:/x"] {
        let mut g = new_grant(&w, w.owner, &[ReadSource]);
        g.allowed_roots = vec![bad_root.into()];
        assert!(
            matches!(
                issue_grant(&mut tx, g, Role::Owner, now).await,
                Err(GrantError::Invalid(_))
            ),
            "{bad_root:?}"
        );
    }
    let mut g = new_grant(&w, w.owner, &[ReadSource]);
    g.provider_destinations = vec!["no colon".into()];
    assert!(matches!(
        issue_grant(&mut tx, g, Role::Owner, now).await,
        Err(GrantError::Invalid(_))
    ));
    let mut g = new_grant(&w, w.owner, &[ReadSource]);
    g.proposal_sha256 = Some("ABC".into());
    assert!(matches!(
        issue_grant(&mut tx, g, Role::Owner, now).await,
        Err(GrantError::Invalid(_))
    ));
    let mut g = new_grant(&w, w.owner, &[]);
    g.operation_classes.clear();
    assert!(matches!(
        issue_grant(&mut tx, g, Role::Owner, now).await,
        Err(GrantError::Invalid(_))
    ));
    let mut g = new_grant(&w, w.owner, &[ReadSource]);
    g.max_cost_micros = -1;
    assert!(matches!(
        issue_grant(&mut tx, g, Role::Owner, now).await,
        Err(GrantError::Invalid(_))
    ));
}

#[tokio::test]
async fn proposal_bound_grant_records_its_hash() {
    let w = world().await;
    let mut tx = w.env.tx(w.owner).await;
    let mut g = new_grant(&w, w.owner, &[LocalCommit]);
    g.proposal_sha256 = Some("a".repeat(64));
    let issued = issue_grant(&mut tx, g, Role::Owner, Utc::now())
        .await
        .unwrap();
    assert!(issued.is_proposal_bound());
    let again = get_grant(&mut tx, issued.id).await.unwrap().unwrap();
    assert_eq!(again, issued);
}

#[tokio::test]
async fn revoke_increments_epoch_and_is_idempotent() {
    let w = world().await;
    let g = issue(&w, w.contributor, Role::Contributor, &[ReadSource])
        .await
        .unwrap();
    let t1 = Utc::now();

    // A different contributor-level member cannot revoke; the viewer is not the grantor or owner.
    {
        let mut tx = w.env.tx(w.viewer).await;
        let err = revoke_grant(&mut tx, g.id, w.viewer, t1).await.unwrap_err();
        assert!(matches!(err, GrantError::NotPermitted), "{err:?}");
    }
    // The project owner can revoke someone else's grant.
    let mut tx = w.env.tx(w.owner).await;
    let revoked = revoke_grant(&mut tx, g.id, w.owner, t1).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(revoked.revocation_epoch, 2);
    assert_eq!(revoked.revoked_by, Some(w.owner));
    assert!(revoked.is_revoked());

    // Second revoke (even by the grantor, later) returns the grant unchanged.
    let mut tx = w.env.tx(w.contributor).await;
    let again = revoke_grant(&mut tx, g.id, w.contributor, t1 + Dur::seconds(5))
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(again, revoked);
    assert_eq!(
        w.env
            .count(&format!(
                "SELECT revocation_epoch FROM agent_grants WHERE id = '{}'",
                g.id
            ))
            .await,
        2
    );
    // Unknown grant.
    let mut tx = w.env.tx(w.owner).await;
    assert!(matches!(
        revoke_grant(&mut tx, GrantId::new(), w.owner, t1).await,
        Err(GrantError::NotFound)
    ));
}

#[tokio::test]
async fn effect_checks_refuse_stale_revoked_expired_and_ungranted() {
    let w = world().await;
    let g = issue(
        &w,
        w.maintainer,
        Role::Maintainer,
        &[ReadSource, WriteTaskWorktree],
    )
    .await
    .unwrap();
    let now = Utc::now();
    let mut tx = w.env.tx(w.maintainer).await;

    let snap = check_for_effect(&mut tx, g.id, 1, WriteTaskWorktree, now)
        .await
        .unwrap();
    assert_eq!(snap.grant.id, g.id);
    assert_eq!(snap.class, WriteTaskWorktree);
    authorize_paths(&snap, &["common/a.txt"]).unwrap();
    authorize_destination(&snap, "anthropic:https://api.anthropic.com").unwrap();

    assert!(matches!(
        check_for_effect(&mut tx, g.id, 1, ModelInference, now).await,
        Err(GrantRefusal::ClassNotGranted(ModelInference))
    ));
    assert!(matches!(
        check_for_effect(&mut tx, g.id, 1, PushOwnedBranch, now).await,
        Err(GrantRefusal::ClassNotGranted(PushOwnedBranch))
    ));
    assert!(matches!(
        check_for_effect(&mut tx, g.id, 7, ReadSource, now).await,
        Err(GrantRefusal::StaleEpoch {
            presented: 7,
            current: 1
        })
    ));
    assert!(matches!(
        check_for_effect(&mut tx, g.id, 1, ReadSource, now + Dur::hours(2)).await,
        Err(GrantRefusal::Expired)
    ));
    assert!(matches!(
        check_for_effect(&mut tx, GrantId::new(), 1, ReadSource, now).await,
        Err(GrantRefusal::NotFound)
    ));
    tx.rollback().await.unwrap();

    // After revocation the old epoch and even the new one are refused.
    let mut tx = w.env.tx(w.maintainer).await;
    revoke_grant(&mut tx, g.id, w.maintainer, now)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut tx = w.env.tx(w.maintainer).await;
    assert!(matches!(
        check_for_effect(&mut tx, g.id, 1, ReadSource, now).await,
        Err(GrantRefusal::Revoked)
    ));
    assert!(matches!(
        check_for_effect(&mut tx, g.id, 2, ReadSource, now).await,
        Err(GrantRefusal::Revoked)
    ));
}

#[tokio::test]
async fn grants_are_visible_to_project_members_only() {
    let w = world().await;
    let g = issue(&w, w.owner, Role::Owner, &[ReadSource])
        .await
        .unwrap();
    let mut tx = w.env.tx(w.viewer).await;
    assert!(get_grant(&mut tx, g.id).await.unwrap().is_some());
    tx.rollback().await.unwrap();
    let mut tx = w.env.tx(w.outsider).await;
    assert!(get_grant(&mut tx, g.id).await.unwrap().is_none());
    assert!(matches!(
        check_for_effect(&mut tx, g.id, 1, ReadSource, Utc::now()).await,
        Err(GrantRefusal::NotFound)
    ));
    assert!(matches!(
        revoke_grant(&mut tx, g.id, w.outsider, Utc::now()).await,
        Err(GrantError::NotFound)
    ));
}

#[tokio::test]
async fn grants_cannot_be_edited_or_deleted_by_the_application_role() {
    let w = world().await;
    let g = issue(&w, w.owner, Role::Owner, &[ReadSource])
        .await
        .unwrap();
    let mut tx = w.env.tx(w.owner).await;
    // Column grants: no other column can change.
    let r = sqlx::query("UPDATE agent_grants SET max_cost_micros = 999999999 WHERE id = $1")
        .bind(g.id.0)
        .execute(&mut *tx)
        .await;
    assert!(r.is_err());
    drop(tx);
    let mut tx = w.env.tx(w.owner).await;
    let r = sqlx::query("DELETE FROM agent_grants WHERE id = $1")
        .bind(g.id.0)
        .execute(&mut *tx)
        .await;
    assert!(r.is_err());
    drop(tx);
    // The only allowed update is a revocation (policy WITH CHECK): bumping the epoch alone fails.
    let mut tx = w.env.tx(w.owner).await;
    let r = sqlx::query("UPDATE agent_grants SET revocation_epoch = 99 WHERE id = $1")
        .bind(g.id.0)
        .execute(&mut *tx)
        .await;
    assert!(r.is_err());
}

#[tokio::test]
async fn concurrent_revoke_waits_for_the_effect_transaction() {
    let w = world().await;
    let g = issue(&w, w.owner, Role::Owner, &[WriteTaskWorktree])
        .await
        .unwrap();
    let now = Utc::now();

    // The effect's transaction checks the grant and holds the share lock.
    let mut effect = w.env.tx(w.owner).await;
    check_for_effect(&mut effect, g.id, 1, WriteTaskWorktree, now)
        .await
        .unwrap();

    // The revoker, in another transaction, must wait.
    let env_db = w.env.database.clone();
    let owner = w.owner;
    let id = g.id;
    let revoker = tokio::spawn(async move {
        let mut tx = env_db.scoped(owner).begin().await.unwrap();
        let revoked = revoke_grant(&mut tx, id, owner, Utc::now()).await.unwrap();
        tx.commit().await.unwrap();
        revoked
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !revoker.is_finished(),
        "revoke must wait while the effect transaction holds FOR SHARE"
    );

    // The effect completes (its own writes would happen here) and commits.
    effect.commit().await.unwrap();
    let revoked = tokio::time::timeout(Duration::from_secs(10), revoker)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(revoked.revocation_epoch, 2);

    // Afterwards the old epoch is refused.
    let mut tx = w.env.tx(w.owner).await;
    assert!(matches!(
        check_for_effect(&mut tx, g.id, 1, WriteTaskWorktree, now).await,
        Err(GrantRefusal::Revoked)
    ));
}

#[tokio::test]
async fn effect_check_after_a_committed_revoke_in_flight_is_refused() {
    // The reverse order: the revoker holds the row; the effect check waits, then sees it revoked.
    let w = world().await;
    let g = issue(&w, w.owner, Role::Owner, &[WriteTaskWorktree])
        .await
        .unwrap();
    let now = Utc::now();
    let mut revoke_tx = w.env.tx(w.owner).await;
    revoke_grant(&mut revoke_tx, g.id, w.owner, now)
        .await
        .unwrap();

    let db = w.env.database.clone();
    let owner = w.owner;
    let id = g.id;
    let effect = tokio::spawn(async move {
        let mut tx = db.scoped(owner).begin().await.unwrap();
        check_for_effect(&mut tx, id, 1, WriteTaskWorktree, Utc::now())
            .await
            .map(|_| ())
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !effect.is_finished(),
        "the effect check must wait for the revoker"
    );
    revoke_tx.commit().await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(10), effect)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(result, Err(GrantRefusal::Revoked)), "{result:?}");
}
