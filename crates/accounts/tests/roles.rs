//! Role permissions on projects (T-ACCT-02 basics): each role permits exactly its actions.

mod common;

use common::*;
use factory_accounts::{Permission, Role, Visibility};
use serde_json::json;
use uuid::Uuid;

const ALL_PERMISSIONS: [Permission; 8] = [
    Permission::ReadProject,
    Permission::CreateTask,
    Permission::ApproveProposal,
    Permission::Publish,
    Permission::RenameProject,
    Permission::ChangeVisibility,
    Permission::MarkForDeletion,
    Permission::ManageMembers,
];

/// One project owned by `owner`, with one member per role.
struct Fixture {
    app: TestApp,
    owner: User,
    project: factory_accounts::ProjectId,
    members: Vec<(Role, User)>,
    outsider: User,
}

async fn fixture() -> Fixture {
    let app = TestApp::new().await;
    let owner = app.user("owner@example.org").await;
    let project = owner
        .repo(&app)
        .create_project("Shared mod", Visibility::Private)
        .await
        .unwrap()
        .id;
    let mut members = Vec::new();
    for role in [Role::Maintainer, Role::Contributor, Role::Viewer] {
        let user = app.user(&format!("{}@example.org", role.as_str())).await;
        owner
            .repo(&app)
            .add_member(project, user.id, role)
            .await
            .unwrap();
        members.push((role, user));
    }
    let outsider = app.user("outsider@example.org").await;
    Fixture {
        app,
        owner,
        project,
        members,
        outsider,
    }
}

#[tokio::test]
async fn authorize_matches_the_permission_matrix_for_every_role() {
    let f = fixture().await;
    let mut everyone: Vec<(Role, &User)> = vec![(Role::Owner, &f.owner)];
    everyone.extend(f.members.iter().map(|(r, u)| (*r, u)));
    for (role, user) in everyone {
        let repo = user.repo(&f.app);
        assert_eq!(repo.get_project(f.project).await.unwrap().role, Some(role));
        for permission in ALL_PERMISSIONS {
            let result = repo.authorize(f.project, permission).await;
            if role.allows(permission) {
                assert!(result.is_ok(), "{role:?} should have {permission:?}");
            } else {
                let err = result.unwrap_err();
                assert_eq!(err.code, "forbidden", "{role:?} {permission:?}");
                assert_eq!(err.status.as_u16(), 403);
                assert_eq!(
                    err.retry_class,
                    factory_accounts::RetryClass::RequiresAuthorization
                );
                assert_eq!(
                    err.affected_ids.get("project_id"),
                    Some(&f.project.to_string())
                );
            }
        }
    }
    // A non-member holds no permission at all, and cannot even tell the project exists.
    let repo = f.outsider.repo(&f.app);
    for permission in ALL_PERMISSIONS {
        assert_eq!(
            repo.authorize(f.project, permission)
                .await
                .unwrap_err()
                .code,
            "not_found"
        );
    }
}

#[tokio::test]
async fn members_see_the_project_in_their_list_outsiders_do_not() {
    let f = fixture().await;
    for (_, user) in &f.members {
        let list = user.repo(&f.app).list_projects(false).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, f.project);
    }
    assert!(
        f.outsider
            .repo(&f.app)
            .list_projects(false)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn rename_visibility_and_deletion_follow_the_roles_through_the_router() {
    let f = fixture().await;
    let path = format!("/api/v1/projects/{}", f.project);
    let by_role = |role: Role| {
        f.members
            .iter()
            .find(|(r, _)| *r == role)
            .map(|(_, u)| u)
            .unwrap()
    };

    // Viewer and contributor can read, nothing else.
    for role in [Role::Viewer, Role::Contributor] {
        let user = by_role(role);
        assert_eq!(user.client.get(&path).await.status, 200);
        let rename = user.client.patch(&path, json!({"name": "hijack"})).await;
        assert_eq!(
            (rename.status, rename.error_code()),
            (403, "forbidden"),
            "{role:?}"
        );
        assert_eq!(
            rename.body["error"]["retry_class"],
            "requires_authorization"
        );
        assert_eq!(
            user.client
                .patch(&path, json!({"visibility": "public"}))
                .await
                .status,
            403
        );
        assert_eq!(user.client.delete(&path).await.status, 403);
        assert_eq!(
            user.client
                .post_empty(&format!("{path}/restore"))
                .await
                .status,
            403
        );
    }

    // A maintainer may rename but not publish the project to the world or delete it.
    let maintainer = by_role(Role::Maintainer);
    let renamed = maintainer
        .client
        .patch(&path, json!({"name": "Renamed by maintainer"}))
        .await;
    assert_eq!(renamed.status, 200, "{}", renamed.text);
    assert_eq!(renamed.body["project"]["name"], "Renamed by maintainer");
    assert_eq!(renamed.body["project"]["role"], "maintainer");
    assert_eq!(
        maintainer
            .client
            .patch(&path, json!({"visibility": "public"}))
            .await
            .status,
        403
    );
    // A combined change is refused as a whole: the allowed half is not applied.
    let mixed = maintainer
        .client
        .patch(
            &path,
            json!({"name": "Half applied", "visibility": "public"}),
        )
        .await;
    assert_eq!(mixed.status, 403);
    assert_eq!(
        f.owner.client.get(&path).await.body["project"]["name"],
        "Renamed by maintainer"
    );
    assert_eq!(maintainer.client.delete(&path).await.status, 403);

    // The owner may do all of it.
    assert_eq!(
        f.owner
            .client
            .patch(&path, json!({"visibility": "public"}))
            .await
            .status,
        200
    );
    let deleted = f.owner.client.delete(&path).await;
    assert_eq!(deleted.status, 202);
    assert!(deleted.body["project"]["delete_after"].is_string());

    // Marked projects are hidden from listings and cannot be edited until restored.
    assert!(
        f.owner.client.get("/api/v1/projects").await.body["projects"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let listed = f
        .owner
        .client
        .get("/api/v1/projects?include_deleted=true")
        .await;
    assert_eq!(listed.body["projects"].as_array().unwrap().len(), 1);
    let blocked = maintainer
        .client
        .patch(&path, json!({"name": "too late"}))
        .await;
    assert_eq!(
        (blocked.status, blocked.error_code()),
        (409, "project_pending_deletion")
    );
    assert_eq!(
        f.owner
            .client
            .post_empty(&format!("{path}/restore"))
            .await
            .status,
        200
    );
    assert_eq!(
        maintainer
            .client
            .patch(&path, json!({"name": "back"}))
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn deletion_grace_period_is_thirty_days_and_restoring_is_bounded_by_it() {
    let f = fixture().await;
    let repo = f.owner.repo(&f.app);
    let marked = repo.mark_for_deletion(f.project).await.unwrap();
    let requested = marked.deleted_at.unwrap();
    assert_eq!(
        marked.delete_after.unwrap() - requested,
        chrono::Duration::days(30)
    );
    // Marking twice keeps the original dates.
    f.app.clock.advance(chrono::Duration::days(1));
    let again = repo.mark_for_deletion(f.project).await.unwrap();
    assert_eq!(again.delete_after, marked.delete_after);
    // Members still see it (for the restore button), listings hide it.
    let viewer = &f
        .members
        .iter()
        .find(|(r, _)| *r == Role::Viewer)
        .unwrap()
        .1;
    assert!(
        viewer
            .repo(&f.app)
            .get_project(f.project)
            .await
            .unwrap()
            .pending_deletion()
    );
    assert!(
        viewer
            .repo(&f.app)
            .list_projects(false)
            .await
            .unwrap()
            .is_empty()
    );
    // After the grace period the project can no longer be restored.
    f.app.clock.advance(chrono::Duration::days(30));
    let err = repo.restore_project(f.project).await.unwrap_err();
    assert_eq!(err.code, "grace_period_over");
}

#[tokio::test]
async fn only_the_owner_manages_members_and_the_owner_seat_is_fixed() {
    let f = fixture().await;
    let repo = f.owner.repo(&f.app);

    let members = repo.list_members(f.project).await.unwrap();
    assert_eq!(members.len(), 4);
    assert_eq!(members.iter().filter(|m| m.role == Role::Owner).count(), 1);

    // Nobody else may list or change members.
    for (_, user) in &f.members {
        let r = user.repo(&f.app);
        assert_eq!(
            r.list_members(f.project).await.unwrap_err().code,
            "forbidden"
        );
        assert_eq!(
            r.add_member(f.project, user.id, Role::Owner)
                .await
                .unwrap_err()
                .code,
            "invalid_request"
        );
        assert_eq!(
            r.add_member(f.project, user.id, Role::Maintainer)
                .await
                .unwrap_err()
                .code,
            "forbidden"
        );
    }
    // The owner seat cannot be handed out or taken away.
    assert_eq!(
        repo.add_member(f.project, f.outsider.id, Role::Owner)
            .await
            .unwrap_err()
            .code,
        "invalid_request"
    );
    assert_eq!(
        repo.add_member(f.project, f.owner.id, Role::Viewer)
            .await
            .unwrap_err()
            .code,
        "invalid_request"
    );
    // Unknown accounts cannot be added.
    let ghost = factory_accounts::AccountId(Uuid::new_v4());
    assert_eq!(
        repo.add_member(f.project, ghost, Role::Viewer)
            .await
            .unwrap_err()
            .code,
        "not_found"
    );

    // Changing a role takes effect at once: demote the maintainer to viewer.
    let (_, maintainer) = f
        .members
        .iter()
        .find(|(r, _)| *r == Role::Maintainer)
        .unwrap();
    repo.add_member(f.project, maintainer.id, Role::Viewer)
        .await
        .unwrap();
    let after = maintainer.repo(&f.app);
    assert_eq!(
        after.get_project(f.project).await.unwrap().role,
        Some(Role::Viewer)
    );
    assert_eq!(
        after
            .rename_project(f.project, "no longer")
            .await
            .unwrap_err()
            .code,
        "forbidden"
    );

    // Raw SQL as the maintainer's former scope cannot rewrite the member table either.
    let mut tx = f.app.db.app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(maintainer.id.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let promote = sqlx::query("UPDATE project_members SET role = 'owner'")
        .execute(&mut *tx)
        .await;
    assert!(promote.is_err() || promote.unwrap().rows_affected() == 0);
    let seats: i64 = sqlx::query_scalar("SELECT count(*) FROM project_members")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(seats, 1, "a member sees only their own seat");
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn project_names_are_validated() {
    let f = fixture().await;
    let repo = f.owner.repo(&f.app);
    for bad in ["", "   ", "line\nbreak", &"x".repeat(101)] {
        assert_eq!(
            repo.create_project(bad, Visibility::Private)
                .await
                .unwrap_err()
                .code,
            "invalid_request"
        );
        assert_eq!(
            repo.rename_project(f.project, bad).await.unwrap_err().code,
            "invalid_request"
        );
    }
    let ok = repo.rename_project(f.project, "  Trimmed  ").await.unwrap();
    assert_eq!(ok.name, "Trimmed");
    // Projects default to private through the API.
    let resp = f
        .owner
        .client
        .post("/api/v1/projects", json!({"name": "Default"}))
        .await;
    assert_eq!(resp.body["project"]["visibility"], "private");
    let empty_patch = f
        .owner
        .client
        .patch(&format!("/api/v1/projects/{}", f.project), json!({}))
        .await;
    assert_eq!(empty_patch.status, 400);
}
