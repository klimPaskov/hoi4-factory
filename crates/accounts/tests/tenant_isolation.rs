//! T-ACCT-01 basics: two accounts cannot list, open, change or infer each other's projects,
//! through the repository API, the HTTP router and raw SQL.

mod common;

use std::sync::Arc;

use common::*;
use factory_accounts::{ApiError, Database, ManualClock, ProjectId, Role, Visibility};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
async fn repository_api_isolates_accounts() {
    let app = TestApp::new().await;
    let alice = app.user("alice@example.org").await;
    let bob = app.user("bob@example.org").await;
    let a = alice.repo(&app);
    let b = bob.repo(&app);

    let a1 = a
        .create_project("Alice One", Visibility::Private)
        .await
        .unwrap();
    let a2 = a
        .create_project("Alice Two", Visibility::Private)
        .await
        .unwrap();
    let b1 = b
        .create_project("Bob One", Visibility::Private)
        .await
        .unwrap();

    // Listing shows only your own.
    let alice_list: Vec<_> = a
        .list_projects(false)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect();
    let mut expected = vec![a1.id, a2.id];
    expected.sort(); // the clock is frozen in tests, so creation time ties
    assert_eq!(alice_list, expected);
    let bob_list: Vec<_> = b
        .list_projects(false)
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(bob_list, vec![b1.id]);

    // Opening or changing someone else's project is "not found", identical to a project that
    // never existed.
    let ghost = ProjectId::new();
    let foreign: Vec<ApiError> = vec![
        b.get_project(a1.id).await.unwrap_err(),
        b.rename_project(a1.id, "mine now").await.unwrap_err(),
        b.set_visibility(a1.id, Visibility::Public)
            .await
            .unwrap_err(),
        b.mark_for_deletion(a1.id).await.unwrap_err(),
        b.restore_project(a1.id).await.unwrap_err(),
        b.list_members(a1.id).await.unwrap_err(),
        b.add_member(a1.id, bob.id, Role::Maintainer)
            .await
            .unwrap_err(),
    ];
    let missing = b.get_project(ghost).await.unwrap_err();
    for err in &foreign {
        assert_eq!(
            (err.status, err.code),
            (missing.status, missing.code),
            "{err}"
        );
        assert_eq!(err.message, missing.message);
        assert_eq!(err.retry_class, missing.retry_class);
        assert!(err.affected_ids.is_empty());
    }
    assert_eq!(missing.code, "not_found");

    // Nothing changed.
    let still = a.get_project(a1.id).await.unwrap();
    assert_eq!(still.name, "Alice One");
    assert_eq!(still.visibility, Visibility::Private);
    assert!(!still.pending_deletion());
    assert_eq!(app.count("SELECT count(*) FROM project_members").await, 3);
}

#[tokio::test]
async fn router_isolates_accounts_and_hides_existence() {
    let app = TestApp::new().await;
    let alice = app.user("alice@example.org").await;
    let bob = app.user("bob@example.org").await;
    let a1 = alice.create_project("Alice One").await;
    let b1 = bob.create_project("Bob One").await;

    let list = bob.client.get("/api/v1/projects").await;
    assert_eq!(list.status, 200);
    let ids: Vec<&str> = list.body["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![b1.as_str()]);
    assert!(
        !list.text.contains(&a1),
        "alice's id must not appear in bob's listing"
    );
    assert!(!list.text.contains("Alice One"));
    // The owner id is never part of the browser view.
    assert!(!list.text.contains(&alice.id.to_string()));

    // Same status and same body for: another tenant's project, a project that does not exist and
    // a malformed id, for every verb. Compare against the "never existed" answer.
    let nowhere = Uuid::new_v4().to_string();
    let hostile = "%27%20OR%201%3D1%20--";
    let routes: Vec<(&str, String, Option<serde_json::Value>)> = vec![
        ("GET", format!("/api/v1/projects/{a1}"), None),
        (
            "PATCH",
            format!("/api/v1/projects/{a1}"),
            Some(json!({"name": "stolen"})),
        ),
        (
            "PATCH",
            format!("/api/v1/projects/{a1}"),
            Some(json!({"visibility": "public"})),
        ),
        ("DELETE", format!("/api/v1/projects/{a1}"), None),
        ("POST", format!("/api/v1/projects/{a1}/restore"), None),
    ];
    for (method, path, body) in routes {
        let foreign = bob.client.send(method, &path, body.clone(), &[]).await;
        let ghost_path = path.replace(&a1, &nowhere);
        let ghost = bob
            .client
            .send(method, &ghost_path, body.clone(), &[])
            .await;
        let junk_path = path.replace(&a1, hostile);
        let junk = bob.client.send(method, &junk_path, body, &[]).await;
        assert_eq!(foreign.status, 404, "{method} {path}: {}", foreign.text);
        assert_eq!(foreign.body, ghost.body, "{method} {path}");
        assert_eq!(foreign.body, junk.body, "{method} {path}");
        assert_eq!(ghost.status, 404);
        assert_eq!(junk.status, 404);
        assert_eq!(foreign.error_code(), "not_found");
    }

    // Alice's project is untouched.
    let own = alice.client.get(&format!("/api/v1/projects/{a1}")).await;
    assert_eq!(own.status, 200);
    assert_eq!(own.body["project"]["name"], "Alice One");
    assert_eq!(own.body["project"]["visibility"], "private");
    assert_eq!(own.body["project"]["role"], "owner");
    assert!(own.body["project"]["delete_after"].is_null());

    // Unauthenticated callers get nothing, and not "not found" either.
    let anon = app.client();
    let resp = anon.get(&format!("/api/v1/projects/{a1}")).await;
    assert_eq!((resp.status, resp.error_code()), (401, "unauthenticated"));
    assert_eq!(anon.get("/api/v1/projects").await.status, 401);
    assert_eq!(
        anon.post("/api/v1/projects", json!({"name": "x"}))
            .await
            .status,
        401
    );
}

#[tokio::test]
async fn hostile_inputs_are_stored_as_data_and_never_widen_a_query() {
    let app = TestApp::new().await;
    let alice = app.user("alice@example.org").await;
    let bob = app.user("bob@example.org").await;
    alice.create_project("Secret mod").await;
    let name = "x'); DROP TABLE projects; SELECT ('";
    let id = bob.create_project(name).await;
    let list = bob.client.get("/api/v1/projects").await;
    assert_eq!(list.body["projects"].as_array().unwrap().len(), 1);
    assert_eq!(list.body["projects"][0]["name"], name);
    let rename = bob
        .client
        .patch(
            &format!("/api/v1/projects/{id}"),
            json!({"name": "' OR true --"}),
        )
        .await;
    assert_eq!(rename.status, 200);
    assert_eq!(app.count("SELECT count(*) FROM projects").await, 2);
    // Owner and role fields in a body are not accepted: the account always comes from the session.
    let spoof = bob
        .client
        .post(
            "/api/v1/projects",
            json!({"name": "spoof", "owner_account_id": alice.id, "account_id": alice.id}),
        )
        .await;
    assert_eq!((spoof.status, spoof.error_code()), (400, "invalid_request"));
    let other = bob
        .client
        .post("/api/v1/projects", json!({"name": "mine"}))
        .await;
    assert_eq!(other.status, 201);
    assert_eq!(
        app.count(&format!(
            "SELECT count(*) FROM projects WHERE owner_account_id = '{}'",
            alice.id
        ))
        .await,
        1
    );
}

#[tokio::test]
async fn rls_blocks_raw_queries_without_the_account_scope() {
    let app = TestApp::new().await;
    let alice = app.user("alice@example.org").await;
    let bob = app.user("bob@example.org").await;
    let a1 = alice
        .repo(&app)
        .create_project("Alice One", Visibility::Private)
        .await
        .unwrap();
    bob.repo(&app)
        .create_project("Bob One", Visibility::Private)
        .await
        .unwrap();
    assert_eq!(
        app.count("SELECT count(*) FROM projects").await,
        2,
        "ground truth as superuser"
    );

    // The application role is not a superuser and does not bypass RLS.
    let posture = app
        .state
        .db()
        .ensure_rls_enforced()
        .await
        .expect("app role posture");
    assert_eq!(posture.role, APP_ROLE);
    assert!(!posture.superuser && !posture.bypass_rls && posture.rls_forced);

    // No setting: nothing is visible and nothing can be written.
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
        .fetch_one(&app.db.app)
        .await
        .unwrap();
    assert_eq!(n, 0, "projects without scope");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM project_members")
        .fetch_one(&app.db.app)
        .await
        .unwrap();
    assert_eq!(n, 0, "members without scope");
    let insert =
        sqlx::query("INSERT INTO projects (id, owner_account_id, name) VALUES ($1, $2, 'sneaky')")
            .bind(Uuid::new_v4())
            .bind(alice.id.0)
            .execute(&app.db.app)
            .await;
    assert!(insert.is_err(), "unscoped insert must be refused");
    assert!(
        insert
            .unwrap_err()
            .to_string()
            .contains("row-level security")
    );

    // An empty or malformed setting fails closed too.
    let mut tx = app.db.app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', '', true)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(n, 0);
    tx.rollback().await.unwrap();
    let mut tx = app.db.app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', 'not-a-uuid', true)")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(
        sqlx::query("SELECT count(*) FROM projects")
            .fetch_one(&mut *tx)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();

    // Bob's scope: a deliberately unfiltered query still shows only Bob's rows, and cannot touch
    // Alice's.
    let mut tx = app.db.app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(bob.id.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let names: Vec<String> = sqlx::query("SELECT name FROM projects ORDER BY name")
        .fetch_all(&mut *tx)
        .await
        .unwrap()
        .iter()
        .map(|r| r.get("name"))
        .collect();
    assert_eq!(names, vec!["Bob One"]);
    let members: i64 = sqlx::query_scalar("SELECT count(*) FROM project_members")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(members, 1);
    let updated = sqlx::query("UPDATE projects SET name = 'pwned'")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(
        updated.rows_affected(),
        1,
        "only bob's own row is reachable"
    );
    let by_id = sqlx::query("UPDATE projects SET name = 'pwned' WHERE id = $1")
        .bind(a1.id.0)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(by_id.rows_affected(), 0);
    let steal = sqlx::query(
        "INSERT INTO projects (id, owner_account_id, name) VALUES ($1, $2, 'as alice')",
    )
    .bind(Uuid::new_v4())
    .bind(alice.id.0)
    .execute(&mut *tx)
    .await;
    assert!(steal.is_err(), "bob cannot create a project owned by alice");
    tx.rollback().await.unwrap();

    // Granting yourself a seat in someone else's project is refused by the member policy.
    let mut tx = app.db.app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(bob.id.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    let join = sqlx::query(
        "INSERT INTO project_members (project_id, account_id, project_owner_id, role) VALUES ($1, $2, $3, 'viewer')",
    )
    .bind(a1.id.0)
    .bind(bob.id.0)
    .bind(alice.id.0)
    .execute(&mut *tx)
    .await;
    assert!(join.is_err(), "bob cannot add himself to alice's project");
    tx.rollback().await.unwrap();

    // Alice's data survived all of that.
    assert_eq!(
        app.count("SELECT count(*) FROM projects WHERE name = 'pwned'")
            .await,
        0
    );
    assert_eq!(
        alice.repo(&app).get_project(a1.id).await.unwrap().name,
        "Alice One"
    );
}

#[tokio::test]
async fn application_role_cannot_reassign_owners_or_hard_delete() {
    let app = TestApp::new().await;
    let alice = app.user("alice@example.org").await;
    let bob = app.user("bob@example.org").await;
    let p = alice
        .repo(&app)
        .create_project("Alice One", Visibility::Private)
        .await
        .unwrap();
    let mut tx = app.db.app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(alice.id.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    // Column privileges: the owner column is immutable for the application role.
    let give_away = sqlx::query("UPDATE projects SET owner_account_id = $2 WHERE id = $1")
        .bind(p.id.0)
        .bind(bob.id.0)
        .execute(&mut *tx)
        .await;
    assert!(give_away.is_err());
    tx.rollback().await.unwrap();
    let mut tx = app.db.app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(alice.id.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    // No DELETE privilege on projects: removal happens in the retention job under another role.
    assert!(
        sqlx::query("DELETE FROM projects WHERE id = $1")
            .bind(p.id.0)
            .execute(&mut *tx)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    assert_eq!(app.count("SELECT count(*) FROM projects").await, 1);
}

#[tokio::test]
async fn forced_rls_binds_the_table_owner_too() {
    let app = TestApp::new().await;
    let alice = app.user("alice@example.org").await;
    alice
        .repo(&app)
        .create_project("Alice One", Visibility::Private)
        .await
        .unwrap();
    // The owner role ran the migrations and owns the tables, yet sees nothing without a scope.
    let owns: bool = sqlx::query_scalar(
        "SELECT tableowner = current_user FROM pg_tables WHERE tablename = 'projects'",
    )
    .fetch_one(&app.db.owner)
    .await
    .unwrap();
    assert!(owns);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
        .fetch_one(&app.db.owner)
        .await
        .unwrap();
    assert_eq!(n, 0, "FORCE ROW LEVEL SECURITY applies to the owner");
    // A privileged role is refused by the posture check, which production start-up runs.
    let privileged = Database::new(app.db.admin.clone(), Arc::new(ManualClock::starting_now()));
    let err = privileged.ensure_rls_enforced().await.unwrap_err();
    assert!(err.to_string().contains("superuser"), "{err}");
}

#[tokio::test]
async fn account_scope_does_not_leak_through_pooled_connections() {
    let app = TestApp::new().await;
    let alice = app.user("alice@example.org").await;
    let bob = app.user("bob@example.org").await;
    // One physical connection serves every call below.
    let single = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::postgres::PgConnectOptions::new()
                .host(app.db.app.connect_options().get_host())
                .port(app.db.app.connect_options().get_port())
                .username(APP_ROLE)
                .database(&app.db.name),
        )
        .await
        .unwrap();
    let db = Database::new(single.clone(), app.clock.clone());
    db.scoped(alice.id)
        .create_project("Alice One", Visibility::Private)
        .await
        .unwrap();
    // The same connection, now used for Bob and then for a raw query: SET LOCAL ended with the
    // transaction.
    assert!(
        db.scoped(bob.id)
            .list_projects(false)
            .await
            .unwrap()
            .is_empty()
    );
    let leaked: Option<String> =
        sqlx::query_scalar("SELECT current_setting('app.account_id', true)")
            .fetch_one(&single)
            .await
            .unwrap();
    assert!(
        leaked.unwrap_or_default().is_empty(),
        "scope must not survive the transaction"
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
        .fetch_one(&single)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn public_projects_are_read_only_for_everyone_else() {
    let app = TestApp::new().await;
    let alice = app.user("alice@example.org").await;
    let bob = app.user("bob@example.org").await;
    let id = alice.create_project("Open mod").await;
    assert_eq!(
        bob.client
            .get(&format!("/api/v1/projects/{id}"))
            .await
            .status,
        404
    );

    let made_public = alice
        .client
        .patch(
            &format!("/api/v1/projects/{id}"),
            json!({"visibility": "public"}),
        )
        .await;
    assert_eq!(made_public.status, 200, "{}", made_public.text);

    let seen = bob.client.get(&format!("/api/v1/projects/{id}")).await;
    assert_eq!(seen.status, 200);
    assert_eq!(seen.body["project"]["name"], "Open mod");
    assert!(
        seen.body["project"]["role"].is_null(),
        "read-only access carries no role"
    );
    // Public is not membership: it is not listed, and it cannot be changed.
    assert!(
        bob.client.get("/api/v1/projects").await.body["projects"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    for (method, body) in [
        ("PATCH", Some(json!({"name": "mine"}))),
        ("PATCH", Some(json!({"visibility": "private"}))),
        ("DELETE", None),
        ("POST", None),
    ] {
        let path = if method == "POST" {
            format!("/api/v1/projects/{id}/restore")
        } else {
            format!("/api/v1/projects/{id}")
        };
        let resp = bob.client.send(method, &path, body, &[]).await;
        assert_eq!(
            (resp.status, resp.error_code()),
            (403, "forbidden"),
            "{method}"
        );
    }

    // Back to private: invisible again.
    alice
        .client
        .patch(
            &format!("/api/v1/projects/{id}"),
            json!({"visibility": "private"}),
        )
        .await;
    assert_eq!(
        bob.client
            .get(&format!("/api/v1/projects/{id}"))
            .await
            .status,
        404
    );
}
