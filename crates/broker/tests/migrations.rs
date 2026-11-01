//! The down migrations run and leave the accounts schema intact.

mod common;

use common::Env;

#[tokio::test]
async fn down_migrations_run_and_can_be_reapplied() {
    let env = Env::new().await;
    let exists = |table: &'static str| {
        let admin = env.admin.clone();
        async move {
            sqlx::query_scalar::<_, bool>("SELECT to_regclass($1) IS NOT NULL")
                .bind(table)
                .fetch_one(&admin)
                .await
                .unwrap()
        }
    };
    for t in ["agent_grants", "user_api_keys", "model_usage"] {
        assert!(exists(t).await, "{t}");
    }
    let m = factory_broker::migrator();
    m.undo(&env.owner, 699)
        .await
        .expect("down migrations apply");
    for t in ["agent_grants", "user_api_keys", "model_usage"] {
        assert!(!exists(t).await, "{t} should be gone");
    }
    assert!(exists("accounts").await && exists("projects").await);
    let fn_left: bool =
        sqlx::query_scalar("SELECT to_regproc('broker_can_see_project') IS NOT NULL")
            .fetch_one(&env.admin)
            .await
            .unwrap();
    assert!(!fn_left);
    // Up again.
    m.run(&env.owner).await.expect("up migrations reapply");
    assert!(exists("model_usage").await);
}
