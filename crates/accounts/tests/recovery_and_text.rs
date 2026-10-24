//! Password reset, "sign out everywhere", international text and the connection pool.

mod common;

use std::time::Duration;

use common::*;
use factory_accounts::{ClientInfo, Database, PoolSettings, Rule};
use serde_json::json;

const NEW_PASSWORD: &str = "a brand new passphrase";

fn client_info(ip: &str) -> ClientInfo<'_> {
    ClientInfo {
        ip,
        user_agent: None,
    }
}

async fn reset_token_for(app: &TestApp, email: &str) -> String {
    let before = app.mailer.sent().len();
    let handle = app
        .state
        .request_password_reset(email, &client_info("198.51.100.9"))
        .unwrap();
    handle.await.unwrap();
    let sent = app.mailer.sent();
    assert_eq!(sent.len(), before + 1, "a reset mail was sent to {email}");
    let mail = sent.last().unwrap();
    assert!(
        mail.text
            .contains("http://localhost:8080/reset-password?token="),
        "{}",
        mail.text
    );
    token_from_mail(mail)
}

#[tokio::test]
async fn reset_requests_answer_the_same_for_known_and_unknown_addresses() {
    let app = TestApp::new().await;
    app.user("player@example.org").await;
    let before = app.mailer.sent().len();

    let known = app
        .client()
        .post(
            "/api/v1/auth/password-reset/request",
            json!({"email": "player@example.org"}),
        )
        .await;
    let unknown = app
        .client()
        .post(
            "/api/v1/auth/password-reset/request",
            json!({"email": "nobody@example.org"}),
        )
        .await;
    let malformed = app
        .client()
        .post(
            "/api/v1/auth/password-reset/request",
            json!({"email": "not an address"}),
        )
        .await;
    for r in [&known, &unknown, &malformed] {
        assert_eq!(r.status, 202, "{}", r.text);
    }
    assert_eq!(known.body, unknown.body);
    assert_eq!(known.body, malformed.body);
    assert!(known.set_cookies().is_empty());

    // The mail goes out in the background, and only for the real account.
    for _ in 0..50 {
        if app.mailer.sent().len() > before {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let sent = app.mailer.sent();
    assert_eq!(sent.len(), before + 1);
    assert_eq!(sent.last().unwrap().to, "player@example.org");
    assert_eq!(
        app.count("SELECT count(*) FROM password_reset_tokens")
            .await,
        1
    );
}

#[tokio::test]
async fn disabled_accounts_and_the_rate_limit_reveal_nothing() {
    let app = TestApp::builder()
        .config(|c| {
            c.rate_limits.rules.insert(
                "reset_ip".into(),
                Rule {
                    capacity: 3,
                    refill_every: Duration::from_secs(60),
                },
            );
        })
        .build()
        .await;
    app.user("gone@example.org").await;
    sqlx::query("UPDATE accounts SET disabled_at = now()")
        .execute(&app.db.admin)
        .await
        .unwrap();
    let before = app.mailer.sent().len();
    app.state
        .request_password_reset("gone@example.org", &client_info("203.0.113.5"))
        .unwrap()
        .await
        .unwrap();
    assert_eq!(
        app.mailer.sent().len(),
        before,
        "no link for a disabled account"
    );
    assert_eq!(
        app.count("SELECT count(*) FROM password_reset_tokens")
            .await,
        0
    );

    // The limit applies per address to every request, known or not, and answers identically.
    let c = app.client().with_ip("203.0.113.77");
    let mut statuses = Vec::new();
    for email in [
        "a@example.org",
        "gone@example.org",
        "b@example.org",
        "c@example.org",
    ] {
        statuses.push(
            c.post(
                "/api/v1/auth/password-reset/request",
                json!({"email": email}),
            )
            .await
            .status,
        );
    }
    assert_eq!(statuses, vec![202, 202, 202, 429]);
}

#[tokio::test]
async fn a_reset_link_works_once_expires_and_ends_every_session() {
    let app = TestApp::new().await;
    let user = app.user("player@example.org").await;
    // A second browser is signed in too.
    let other = app.client();
    other
        .post(
            "/api/v1/auth/signin",
            json!({"email": "player@example.org", "password": GOOD_PASSWORD}),
        )
        .await;
    assert_eq!(other.get("/api/v1/auth/me").await.status, 200);

    let token = reset_token_for(&app, "PLAYER@example.org").await;
    // Only the digest is stored.
    let leaked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM password_reset_tokens WHERE encode(token_hash, 'escape') LIKE '%' || $1 || '%'",
    )
    .bind(&token)
    .fetch_one(&app.db.admin)
    .await
    .unwrap();
    assert_eq!(leaked, 0);

    // A weak password is refused and does not burn the link.
    let weak = app
        .client()
        .post(
            "/api/v1/auth/password-reset/confirm",
            json!({"token": token, "new_password": "short"}),
        )
        .await;
    assert_eq!((weak.status, weak.error_code()), (400, "invalid_request"));
    assert_eq!(
        user.client.get("/api/v1/auth/me").await.status,
        200,
        "nothing changed yet"
    );

    // A wrong token changes nothing.
    let wrong = app
        .client()
        .post(
            "/api/v1/auth/password-reset/confirm",
            json!({"token": "x".repeat(43), "new_password": NEW_PASSWORD}),
        )
        .await;
    assert_eq!((wrong.status, wrong.error_code()), (400, "invalid_token"));

    let ok = app
        .client()
        .post(
            "/api/v1/auth/password-reset/confirm",
            json!({"token": token, "new_password": NEW_PASSWORD}),
        )
        .await;
    assert_eq!(ok.status, 200, "{}", ok.text);
    assert!(
        ok.set_cookie("hf_session").is_none(),
        "the user signs in again"
    );

    // Every session is gone, the old password is dead, the new one works.
    assert_eq!(user.client.get("/api/v1/auth/me").await.status, 401);
    assert_eq!(other.get("/api/v1/auth/me").await.status, 401);
    assert_eq!(app.count("SELECT count(*) FROM sessions").await, 0);
    let old = app
        .client()
        .post(
            "/api/v1/auth/signin",
            json!({"email": "player@example.org", "password": GOOD_PASSWORD}),
        )
        .await;
    assert_eq!(old.error_code(), "invalid_credentials");
    let fresh = app.client();
    let signed_in = fresh
        .post(
            "/api/v1/auth/signin",
            json!({"email": "player@example.org", "password": NEW_PASSWORD}),
        )
        .await;
    assert_eq!(signed_in.status, 200);
    assert_eq!(
        signed_in.body["account"]["email_verified"], true,
        "the link proved the mailbox"
    );

    // Single use.
    let again = app
        .client()
        .post(
            "/api/v1/auth/password-reset/confirm",
            json!({"token": token, "new_password": "yet another phrase"}),
        )
        .await;
    assert_eq!((again.status, again.error_code()), (400, "invalid_token"));
    let still = app
        .client()
        .post(
            "/api/v1/auth/signin",
            json!({"email": "player@example.org", "password": NEW_PASSWORD}),
        )
        .await;
    assert_eq!(still.status, 200);
}

#[tokio::test]
async fn reset_links_expire_after_thirty_minutes_and_a_newer_request_replaces_the_old() {
    let app = TestApp::new().await;
    app.user("player@example.org").await;

    let first = reset_token_for(&app, "player@example.org").await;
    let second = reset_token_for(&app, "player@example.org").await;
    assert_ne!(first, second);
    assert_eq!(
        app.count("SELECT count(*) FROM password_reset_tokens WHERE consumed_at IS NULL")
            .await,
        1
    );
    let r = app
        .client()
        .post(
            "/api/v1/auth/password-reset/confirm",
            json!({"token": first, "new_password": NEW_PASSWORD}),
        )
        .await;
    assert_eq!(
        r.error_code(),
        "invalid_token",
        "the older link died with the newer request"
    );

    app.clock.advance(chrono::Duration::minutes(31));
    let late = app
        .client()
        .post(
            "/api/v1/auth/password-reset/confirm",
            json!({"token": second, "new_password": NEW_PASSWORD}),
        )
        .await;
    assert_eq!((late.status, late.error_code()), (400, "invalid_token"));
    let expiry: (chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as("SELECT created_at, expires_at FROM password_reset_tokens")
            .fetch_one(&app.db.admin)
            .await
            .unwrap();
    assert_eq!(expiry.1 - expiry.0, chrono::Duration::minutes(30));

    // A fresh request, used within the window, works.
    let third = reset_token_for(&app, "player@example.org").await;
    app.clock.advance(chrono::Duration::minutes(29));
    let ok = app
        .client()
        .post(
            "/api/v1/auth/password-reset/confirm",
            json!({"token": third, "new_password": NEW_PASSWORD}),
        )
        .await;
    assert_eq!(ok.status, 200);
}

#[tokio::test]
async fn sign_out_everywhere_ends_all_sessions_of_one_account_only() {
    let app = TestApp::new().await;
    let alice = app.user("alice@example.org").await;
    let bob = app.user("bob@example.org").await;
    let mut browsers = vec![];
    for _ in 0..2 {
        let c = app.client();
        assert_eq!(
            c.post(
                "/api/v1/auth/signin",
                json!({"email": "alice@example.org", "password": GOOD_PASSWORD})
            )
            .await
            .status,
            200
        );
        browsers.push(c);
    }
    assert_eq!(
        app.count(&format!(
            "SELECT count(*) FROM sessions WHERE account_id = '{}'",
            alice.id
        ))
        .await,
        3
    );

    let out = browsers[0].post_empty("/api/v1/auth/signout-all").await;
    assert_eq!(out.status, 200);
    assert_eq!(out.body["sessions_ended"], 3);
    assert!(out.set_cookie("hf_session").unwrap().contains("Max-Age=0"));
    for c in browsers.iter().chain([&alice.client]) {
        assert_eq!(c.get("/api/v1/auth/me").await.status, 401);
    }
    assert_eq!(
        bob.client.get("/api/v1/auth/me").await.status,
        200,
        "other accounts are untouched"
    );
    assert_eq!(app.post_unauthenticated_signout_all().await, 401);

    // The internal function does the same for administrators and security code.
    let again = app.client();
    again
        .post(
            "/api/v1/auth/signin",
            json!({"email": "bob@example.org", "password": GOOD_PASSWORD}),
        )
        .await;
    assert_eq!(app.state.revoke_all_sessions(bob.id).await.unwrap(), 2);
    assert_eq!(bob.client.get("/api/v1/auth/me").await.status, 401);
    assert_eq!(app.state.revoke_all_sessions(bob.id).await.unwrap(), 0);
}

impl TestApp {
    async fn post_unauthenticated_signout_all(&self) -> u16 {
        self.client()
            .post_empty("/api/v1/auth/signout-all")
            .await
            .status
    }
}

#[tokio::test]
async fn emails_keep_plus_tags_and_unicode_and_compare_without_case() {
    let app = TestApp::new().await;
    let addresses = [
        "first.last+hoi4@Example.org",
        "o'brien@example.org",
        "łukasz+żółć@example.pl",
        "用户@example.cn",
        "مستخدم@example.org",
    ];
    for address in addresses {
        let c = app.client();
        let r = c
            .post(
                "/api/v1/auth/signup",
                json!({"email": address, "password": GOOD_PASSWORD}),
            )
            .await;
        assert_eq!(r.status, 201, "{address}: {}", r.text);
        assert_eq!(r.body["account"]["email"], address, "kept exactly as typed");
        let stored: String =
            sqlx::query_scalar("SELECT email FROM accounts WHERE lower(email) = lower($1)")
                .bind(address)
                .fetch_one(&app.db.admin)
                .await
                .unwrap();
        assert_eq!(stored, address);
    }
    // Sign-in finds the account whatever the case.
    let c = app.client();
    let r = c
        .post(
            "/api/v1/auth/signin",
            json!({"email": "FIRST.LAST+HOI4@example.ORG", "password": GOOD_PASSWORD}),
        )
        .await;
    assert_eq!(r.status, 200);
    assert_eq!(r.body["account"]["email"], "first.last+hoi4@Example.org");

    // "+tag" is part of the address: the plain address is a different account.
    let plain = app
        .client()
        .post(
            "/api/v1/auth/signup",
            json!({"email": "first.last@example.org", "password": GOOD_PASSWORD}),
        )
        .await;
    assert_eq!(plain.status, 201);
    let clash = app
        .client()
        .post(
            "/api/v1/auth/signup",
            json!({"email": "FIRST.LAST+Hoi4@EXAMPLE.org", "password": GOOD_PASSWORD}),
        )
        .await;
    assert_eq!(clash.error_code(), "email_taken");

    // The verification mail went to the address as typed.
    assert_eq!(
        app.mailer.last_to("用户@example.cn").unwrap().to,
        "用户@example.cn"
    );
}

#[tokio::test]
async fn names_accept_international_text_and_limits_are_enforced_on_the_server() {
    let app = TestApp::new().await;
    let names = [
        "O'Brien's Weimar",
        "Café Ünïcode",
        "🇩🇪 Deutsches Reich 🚩",
        "עברית ומודל",
        "نموذج عربي",
        "日本語のMOD",
        "👨‍👩‍👧‍👦 family",
    ];
    let user = {
        let client = app.client();
        let r = client
            .post(
                "/api/v1/auth/signup",
                json!({"email": "intl@example.org", "password": GOOD_PASSWORD, "display_name": "Zoë O'Neil 🎮"}),
            )
            .await;
        assert_eq!(r.status, 201, "{}", r.text);
        assert_eq!(r.body["account"]["display_name"], "Zoë O'Neil 🎮");
        client
    };
    for name in names {
        let r = user.post("/api/v1/projects", json!({"name": name})).await;
        assert_eq!(r.status, 201, "{name}: {}", r.text);
        assert_eq!(r.body["project"]["name"], name);
    }
    let listed = user.get("/api/v1/projects").await;
    assert_eq!(
        listed.body["projects"].as_array().unwrap().len(),
        names.len()
    );

    // Over the limits, however they are measured: graphemes, bytes, control characters.
    let too_many_graphemes = "é".repeat(101);
    let too_many_bytes = "👨‍👩‍👧‍👦".repeat(100); // 100 graphemes, 2500 bytes
    for bad in [
        too_many_graphemes.as_str(),
        too_many_bytes.as_str(),
        "line\nbreak",
        "   ",
        "nul\u{0}",
    ] {
        let r = user.post("/api/v1/projects", json!({"name": bad})).await;
        assert_eq!(
            (r.status, r.error_code()),
            (400, "invalid_request"),
            "{bad:?}"
        );
    }
    let r = app
        .client()
        .post(
            "/api/v1/auth/signup",
            json!({"email": "long@example.org", "password": GOOD_PASSWORD, "display_name": "x".repeat(81)}),
        )
        .await;
    assert_eq!(r.status, 400);
    // The database refuses oversized names even if the application were bypassed.
    let raw = sqlx::query("INSERT INTO accounts (id, email, password_hash, display_name) VALUES (gen_random_uuid(), 'raw@example.org', 'x', $1)")
        .bind("x".repeat(321))
        .execute(&app.db.admin)
        .await;
    assert!(raw.is_err());
}

#[tokio::test]
async fn projects_are_soft_deleted_and_timestamps_are_timestamptz() {
    let app = TestApp::new().await;
    let user = app.user("player@example.org").await;
    let id = user.create_project("Soft").await;
    let path = format!("/api/v1/projects/{id}");
    assert_eq!(user.client.delete(&path).await.status, 202);
    // The row still exists, marked, with its purge date.
    let (deleted, purge): (
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as("SELECT deleted_at, delete_after FROM projects")
        .fetch_one(&app.db.admin)
        .await
        .unwrap();
    assert!(deleted.is_some() && purge.is_some());
    assert!(
        user.client.get("/api/v1/projects").await.body["projects"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let not_tz: Vec<String> = sqlx::query_scalar(
        "SELECT table_name || '.' || column_name FROM information_schema.columns
         WHERE table_schema = 'public' AND table_name <> '_sqlx_migrations'
           AND (column_name LIKE '%\\_at' OR column_name = 'delete_after')
           AND data_type <> 'timestamp with time zone'",
    )
    .fetch_all(&app.db.admin)
    .await
    .unwrap();
    assert!(not_tz.is_empty(), "{not_tz:?}");
}

#[tokio::test]
async fn the_connection_pool_honours_its_configured_size_and_timeout() {
    let app = TestApp::new().await;
    let options = app.db.app.connect_options();
    let url = format!(
        "postgres://{APP_ROLE}@{}:{}/{}",
        options.get_host(),
        options.get_port(),
        app.db.name
    );
    let settings = PoolSettings {
        max_connections: 2,
        acquire_timeout: Duration::from_millis(500),
        ..PoolSettings::default()
    };
    let db = Database::connect(&url, &settings, app.clock.clone())
        .await
        .unwrap();
    assert_eq!(db.pool().options().get_max_connections(), 2);
    let a = db.pool().acquire().await.unwrap();
    let b = db.pool().acquire().await.unwrap();
    let started = std::time::Instant::now();
    let third = db.pool().acquire().await;
    assert!(matches!(third, Err(sqlx::Error::PoolTimedOut)), "{third:?}");
    assert!(started.elapsed() < Duration::from_secs(3));
    drop((a, b));
    db.pool()
        .acquire()
        .await
        .expect("connections return to the pool");
}
