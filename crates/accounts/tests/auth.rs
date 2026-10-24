//! Email and password accounts: sign-up, verification, sign-in, sessions, rate limits and the
//! cross-site guard.

mod common;

use std::time::Duration;

use common::*;
use factory_accounts::Rule;
use serde_json::json;

fn signup_body(email: &str) -> serde_json::Value {
    json!({"email": email, "password": GOOD_PASSWORD})
}

#[tokio::test]
async fn sign_up_creates_an_unverified_account_with_argon2id_and_an_opaque_session() {
    let app = TestApp::new().await;
    let client = app.client();
    let resp = client
        .post(
            "/api/v1/auth/signup",
            json!({"email": "  Player@Example.ORG ", "password": GOOD_PASSWORD, "display_name": "Player One"}),
        )
        .await;
    assert_eq!(resp.status, 201, "{}", resp.text);
    assert_eq!(
        resp.body["account"]["email"], "Player@Example.ORG",
        "kept as typed"
    );
    assert_eq!(resp.body["account"]["email_verified"], false);
    assert_eq!(resp.body["account"]["display_name"], "Player One");
    assert_eq!(resp.body["account"]["github_linked"], false);
    assert_eq!(resp.body["verification_email_sent"], true);
    // No secret material in the body.
    assert!(!resp.text.contains("argon2"));
    assert!(!resp.text.contains(GOOD_PASSWORD));
    assert!(resp.body["account"].get("password_hash").is_none());

    // Stored as Argon2id, never in clear.
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM accounts")
        .fetch_one(&app.db.admin)
        .await
        .unwrap();
    assert!(hash.starts_with("$argon2id$v=19$"), "{hash}");
    assert!(!hash.contains(GOOD_PASSWORD));

    // The cookie value is an opaque 256-bit token, and only its SHA-256 digest is stored.
    let token = client.cookie("hf_session").expect("session cookie");
    assert_eq!(token.len(), 43);
    let stored: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM sessions")
        .fetch_one(&app.db.admin)
        .await
        .unwrap();
    assert_eq!(stored.len(), 32);
    assert_ne!(stored, token.as_bytes());
    let all_text: String =
        sqlx::query_scalar("SELECT string_agg(row_to_json(s)::text, ' ') FROM sessions s")
            .fetch_one(&app.db.admin)
            .await
            .unwrap();
    assert!(!all_text.contains(&token));

    let me = client.get("/api/v1/auth/me").await;
    assert_eq!(me.status, 200);
    assert_eq!(me.body["account"]["email"], "Player@Example.ORG");
}

#[tokio::test]
async fn session_cookie_attributes_depend_on_the_environment() {
    let dev = TestApp::new().await;
    let resp = dev
        .client()
        .post("/api/v1/auth/signup", signup_body("dev@example.org"))
        .await;
    let cookie = resp.set_cookie("hf_session").unwrap();
    for attr in ["HttpOnly", "SameSite=Lax", "Path=/", "Max-Age="] {
        assert!(cookie.contains(attr), "{cookie}");
    }
    assert!(
        !cookie.contains("Secure"),
        "development cookies work over http: {cookie}"
    );

    let prod = TestApp::builder().production().build().await;
    let resp = prod
        .client()
        .post("/api/v1/auth/signup", signup_body("prod@example.org"))
        .await;
    let cookie = resp.set_cookie("hf_session").unwrap();
    for attr in ["HttpOnly", "SameSite=Lax", "Secure", "Path=/"] {
        assert!(cookie.contains(attr), "{cookie}");
    }
    let signin = prod
        .client()
        .post("/api/v1/auth/signin", signup_body("prod@example.org"))
        .await;
    assert!(signin.set_cookie("hf_session").unwrap().contains("Secure"));
    let signout = prod.client();
    signout
        .post("/api/v1/auth/signin", signup_body("prod@example.org"))
        .await;
    let out = signout.post_empty("/api/v1/auth/signout").await;
    let cleared = out.set_cookie("hf_session").unwrap();
    assert!(
        cleared.contains("Max-Age=0") && cleared.contains("Secure") && cleared.contains("HttpOnly")
    );
}

#[tokio::test]
async fn verification_link_verifies_once_and_expires() {
    let app = TestApp::new().await;
    let user = app.user("player@example.org").await;
    let mail = app
        .mailer
        .last_to("player@example.org")
        .expect("verification mail");
    assert!(
        mail.text
            .contains("http://localhost:8080/verify-email?token="),
        "{}",
        mail.text
    );
    let token = token_from_mail(&mail);
    // Only the digest of the token is stored.
    assert_eq!(
        app.count("SELECT count(*) FROM email_verification_tokens")
            .await,
        1
    );
    let leaked: i64 = sqlx::query_scalar("SELECT count(*) FROM email_verification_tokens WHERE encode(token_hash, 'escape') LIKE '%' || $1 || '%'")
        .bind(&token)
        .fetch_one(&app.db.admin)
        .await
        .unwrap();
    assert_eq!(leaked, 0);

    assert_eq!(
        user.client.get("/api/v1/auth/me").await.body["account"]["email_verified"],
        false
    );
    let ok = user
        .client
        .post("/api/v1/auth/verify-email", json!({"token": token}))
        .await;
    assert_eq!(ok.status, 200, "{}", ok.text);
    assert_eq!(ok.body["account"]["email_verified"], true);
    assert_eq!(
        user.client.get("/api/v1/auth/me").await.body["account"]["email_verified"],
        true
    );

    // Single use.
    let again = user
        .client
        .post("/api/v1/auth/verify-email", json!({"token": token}))
        .await;
    assert_eq!((again.status, again.error_code()), (400, "invalid_token"));
    // Garbage and empty tokens fail the same way.
    for bad in ["", "nonsense", &"x".repeat(500)] {
        let resp = user
            .client
            .post("/api/v1/auth/verify-email", json!({"token": bad}))
            .await;
        assert_eq!((resp.status, resp.error_code()), (400, "invalid_token"));
    }

    // Expiry: a second account whose link is used a day and a minute late.
    let late = app.user("late@example.org").await;
    let token = token_from_mail(&app.mailer.last_to("late@example.org").unwrap());
    app.clock
        .advance(chrono::Duration::hours(24) + chrono::Duration::minutes(1));
    let resp = app
        .client()
        .post("/api/v1/auth/verify-email", json!({"token": token}))
        .await;
    assert_eq!((resp.status, resp.error_code()), (400, "invalid_token"));
    // Sessions still exist after 24 hours, so the owner can ask for a new link.
    let resend = late
        .client
        .post_empty("/api/v1/auth/resend-verification")
        .await;
    assert_eq!(resend.status, 202);
    assert_eq!(resend.body["sent"], true);
    let fresh = token_from_mail(&app.mailer.last_to("late@example.org").unwrap());
    assert_ne!(fresh, token);
    let ok = app
        .client()
        .post("/api/v1/auth/verify-email", json!({"token": fresh}))
        .await;
    assert_eq!(ok.status, 200);
}

#[tokio::test]
async fn resending_invalidates_the_previous_link_and_skips_verified_accounts() {
    let app = TestApp::new().await;
    let user = app.user("player@example.org").await;
    let first = token_from_mail(&app.mailer.last_to("player@example.org").unwrap());
    user.client
        .post_empty("/api/v1/auth/resend-verification")
        .await;
    let second = token_from_mail(&app.mailer.last_to("player@example.org").unwrap());
    assert_ne!(first, second);
    let stale = app
        .client()
        .post("/api/v1/auth/verify-email", json!({"token": first}))
        .await;
    assert_eq!(stale.error_code(), "invalid_token");
    assert_eq!(
        app.client()
            .post("/api/v1/auth/verify-email", json!({"token": second}))
            .await
            .status,
        200
    );
    let mails_before = app.mailer.sent().len();
    let resend = user
        .client
        .post_empty("/api/v1/auth/resend-verification")
        .await;
    assert_eq!(
        (resend.status, resend.body["already_verified"].clone()),
        (202, json!(true))
    );
    assert_eq!(
        app.mailer.sent().len(),
        mails_before,
        "no mail for a verified account"
    );
    // Resending requires a session.
    assert_eq!(
        app.client()
            .post_empty("/api/v1/auth/resend-verification")
            .await
            .status,
        401
    );
}

#[tokio::test]
async fn a_failing_mail_transport_does_not_block_sign_up() {
    let app = TestApp::new().await;
    app.mailer.set_failing(true);
    let client = app.client();
    let resp = client
        .post("/api/v1/auth/signup", signup_body("player@example.org"))
        .await;
    assert_eq!(resp.status, 201);
    assert_eq!(resp.body["verification_email_sent"], false);
    assert_eq!(client.get("/api/v1/auth/me").await.status, 200);
    app.mailer.set_failing(false);
    let resend = client.post_empty("/api/v1/auth/resend-verification").await;
    assert_eq!(resend.body["sent"], true);
    assert!(app.mailer.last_to("player@example.org").is_some());
}

#[tokio::test]
async fn sign_in_wrong_password_unknown_account_and_unverified_paths() {
    let app = TestApp::new().await;
    app.user("player@example.org").await;

    // Unverified accounts can sign in and are flagged unverified.
    let browser = app.client();
    let ok = browser
        .post("/api/v1/auth/signin", signup_body("PLAYER@example.org"))
        .await;
    assert_eq!(ok.status, 200, "{}", ok.text);
    assert_eq!(ok.body["account"]["email_verified"], false);
    assert!(browser.cookie("hf_session").is_some());
    assert_eq!(
        browser.get("/api/v1/auth/me").await.body["account"]["email_verified"],
        false
    );

    // The verified gate used by later stages refuses this principal.
    let token = browser.cookie("hf_session").unwrap();
    let principal = app.state.authenticate(&token).await.unwrap();
    let err = principal.require_verified().unwrap_err();
    assert_eq!((err.code, err.status.as_u16()), ("email_not_verified", 403));
    app.client()
        .post(
            "/api/v1/auth/verify-email",
            json!({"token": token_from_mail(&app.mailer.last_to("player@example.org").unwrap())}),
        )
        .await;
    app.state
        .authenticate(&token)
        .await
        .unwrap()
        .require_verified()
        .unwrap();

    // Wrong password and unknown account are indistinguishable.
    let wrong = app
        .client()
        .post(
            "/api/v1/auth/signin",
            json!({"email": "player@example.org", "password": "definitely not it"}),
        )
        .await;
    let unknown = app
        .client()
        .post(
            "/api/v1/auth/signin",
            json!({"email": "nobody@example.org", "password": GOOD_PASSWORD}),
        )
        .await;
    let malformed = app
        .client()
        .post(
            "/api/v1/auth/signin",
            json!({"email": "not an email", "password": GOOD_PASSWORD}),
        )
        .await;
    for r in [&wrong, &unknown, &malformed] {
        assert_eq!((r.status, r.error_code()), (401, "invalid_credentials"));
        assert!(r.set_cookie("hf_session").is_none());
    }
    assert_eq!(wrong.body, unknown.body);
    assert_eq!(wrong.body, malformed.body);
    assert_eq!(wrong.body["error"]["retry_class"], "terminal");
    assert_eq!(
        app.count("SELECT count(*) FROM sessions").await,
        2,
        "failed attempts create no session"
    );
}

#[tokio::test]
async fn accounts_without_a_password_cannot_sign_in_with_one() {
    let app = TestApp::new().await;
    sqlx::query("INSERT INTO accounts (id, email, github_user_id, display_name, email_verified_at) VALUES (gen_random_uuid(), 'gh@example.org', 77, 'gh', now())")
        .execute(&app.db.admin)
        .await
        .unwrap();
    let resp = app
        .client()
        .post("/api/v1/auth/signin", signup_body("gh@example.org"))
        .await;
    assert_eq!(
        (resp.status, resp.error_code()),
        (401, "invalid_credentials")
    );
}

#[tokio::test]
async fn email_addresses_are_unique_ignoring_case() {
    let app = TestApp::new().await;
    app.user("Player@Example.org").await;
    let dup = app
        .client()
        .post("/api/v1/auth/signup", signup_body("player@EXAMPLE.org"))
        .await;
    assert_eq!((dup.status, dup.error_code()), (409, "email_taken"));
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 1);
    // The database enforces it as well, not only the application.
    let raw = sqlx::query("INSERT INTO accounts (id, email, password_hash, display_name) VALUES (gen_random_uuid(), 'PLAYER@example.org', 'x', 'dup')")
        .execute(&app.db.admin)
        .await;
    assert!(raw.is_err());
}

#[tokio::test]
async fn invalid_input_uses_the_error_envelope() {
    let app = TestApp::new().await;
    let c = app.client();
    let cases = [
        json!({"email": "player@example.org", "password": "short"}),
        json!({"email": "not-an-email", "password": GOOD_PASSWORD}),
        json!({"email": "player@example.org", "password": GOOD_PASSWORD, "is_admin": true}),
        json!({"email": "player@example.org"}),
        json!({"email": "player@example.org", "password": GOOD_PASSWORD, "display_name": "x".repeat(81)}),
    ];
    for body in cases {
        let resp = c.post("/api/v1/auth/signup", body).await;
        assert_eq!(
            (resp.status, resp.error_code()),
            (400, "invalid_request"),
            "{}",
            resp.text
        );
        let err = &resp.body["error"];
        assert!(err["message"].as_str().unwrap().len() > 3);
        assert_eq!(err["retry_class"], "terminal");
        assert!(err["affected_ids"].is_object());
        assert!(err["evidence_refs"].is_array());
    }
    // A validation error names the field the form should mark.
    let named = [
        (
            json!({"email": "player@example.org", "password": "short"}),
            "password",
        ),
        (
            json!({"email": "not-an-email", "password": GOOD_PASSWORD}),
            "email",
        ),
        (
            json!({"email": "player@example.org", "password": GOOD_PASSWORD, "display_name": "x".repeat(81)}),
            "display_name",
        ),
    ];
    for (body, field) in named {
        let resp = c.post("/api/v1/auth/signup", body).await;
        assert_eq!(resp.status, 400, "{}", resp.text);
        assert_eq!(resp.body["error"]["affected_ids"]["field"], field);
    }
    // Not JSON at all.
    let resp = c.send("POST", "/api/v1/auth/signup", None, &[]).await;
    assert_eq!((resp.status, resp.error_code()), (400, "invalid_request"));
    let resp = c.send("GET", "/api/v1/nope", None, &[]).await;
    assert_eq!((resp.status, resp.error_code()), (404, "not_found"));
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 0);
}

#[tokio::test]
async fn sign_out_ends_the_session_and_is_idempotent() {
    let app = TestApp::new().await;
    let user = app.user("player@example.org").await;
    let token = user.client.cookie("hf_session").unwrap();
    let out = user.client.post_empty("/api/v1/auth/signout").await;
    assert_eq!(out.status, 204);
    assert!(out.set_cookie("hf_session").unwrap().contains("Max-Age=0"));
    assert!(user.client.cookie("hf_session").is_none());
    assert_eq!(app.count("SELECT count(*) FROM sessions").await, 0);
    // The old token is dead even if a browser keeps sending it.
    user.client.set_cookie("hf_session", &token);
    let me = user.client.get("/api/v1/auth/me").await;
    assert_eq!((me.status, me.error_code()), (401, "unauthenticated"));
    assert_eq!(me.body["error"]["retry_class"], "requires_authorization");
    assert_eq!(
        app.client().post_empty("/api/v1/auth/signout").await.status,
        204
    );
}

#[tokio::test]
async fn sessions_expire_absolutely_and_when_idle() {
    let app = TestApp::new().await;
    let signup = app.client();
    signup
        .post("/api/v1/auth/signup", signup_body("player@example.org"))
        .await;

    // Idle: unused for more than 7 days.
    let idle = app.client();
    idle.post("/api/v1/auth/signin", signup_body("player@example.org"))
        .await;
    // Absolute: used daily, but 30 days is the limit.
    let busy = app.client();
    busy.post("/api/v1/auth/signin", signup_body("player@example.org"))
        .await;

    for day in 1..=29 {
        app.clock.advance(chrono::Duration::days(1));
        assert_eq!(busy.get("/api/v1/auth/me").await.status, 200, "day {day}");
        if day == 6 {
            assert_eq!(
                idle.get("/api/v1/auth/me").await.status,
                200,
                "6 idle days is fine"
            );
        }
    }
    // idle was last used on day 6, now day 29: gone. busy is on day 29 of 30.
    assert_eq!(idle.get("/api/v1/auth/me").await.status, 401);
    assert_eq!(busy.get("/api/v1/auth/me").await.status, 200);
    app.clock.advance(chrono::Duration::days(2));
    let gone = busy.get("/api/v1/auth/me").await;
    assert_eq!((gone.status, gone.error_code()), (401, "unauthenticated"));
    // A fresh sign-in works again.
    assert_eq!(
        app.client()
            .post("/api/v1/auth/signin", signup_body("player@example.org"))
            .await
            .status,
        200
    );
    // Maintenance removes the dead rows.
    assert!(app.state.purge_expired().await.unwrap() >= 3);
    assert_eq!(app.count("SELECT count(*) FROM sessions").await, 1);
}

#[tokio::test]
async fn disabled_accounts_lose_access_at_once() {
    let app = TestApp::new().await;
    let user = app.user("player@example.org").await;
    assert_eq!(user.client.get("/api/v1/auth/me").await.status, 200);
    sqlx::query(
        "UPDATE accounts SET disabled_at = now(), delete_after = now() + interval '30 days'",
    )
    .execute(&app.db.admin)
    .await
    .unwrap();
    assert_eq!(user.client.get("/api/v1/auth/me").await.status, 401);
    assert_eq!(user.client.get("/api/v1/projects").await.status, 401);
    let resp = app
        .client()
        .post("/api/v1/auth/signin", signup_body("player@example.org"))
        .await;
    assert_eq!((resp.status, resp.error_code()), (403, "account_disabled"));
    // A wrong password still gets the generic answer, so disabled status is not probeable.
    let resp = app
        .client()
        .post(
            "/api/v1/auth/signin",
            json!({"email": "player@example.org", "password": "wrong password!"}),
        )
        .await;
    assert_eq!(resp.error_code(), "invalid_credentials");
}

#[tokio::test]
async fn sign_in_is_rate_limited_per_address_and_per_account() {
    let app = TestApp::builder()
        .config(|c| {
            c.rate_limits.rules.insert(
                "signin_ip".into(),
                Rule {
                    capacity: 4,
                    refill_every: Duration::from_secs(30),
                },
            );
            c.rate_limits.rules.insert(
                "signin_account".into(),
                Rule {
                    capacity: 3,
                    refill_every: Duration::from_secs(60),
                },
            );
        })
        .build()
        .await;
    app.user("player@example.org").await;
    let wrong = json!({"email": "player@example.org", "password": "wrong wrong wrong"});

    // Per account: three tries from three different addresses, the fourth is refused even from
    // a fresh address, and even with the right password.
    for i in 1..=3 {
        let c = app.client().with_ip(&format!("203.0.113.{i}"));
        assert_eq!(
            c.post("/api/v1/auth/signin", wrong.clone()).await.status,
            401
        );
    }
    let c = app.client().with_ip("203.0.113.99");
    let blocked = c
        .post("/api/v1/auth/signin", signup_body("player@example.org"))
        .await;
    assert_eq!(
        (blocked.status, blocked.error_code()),
        (429, "rate_limited")
    );
    assert_eq!(blocked.body["error"]["retry_class"], "safe_same_inputs");
    let retry: u64 = blocked
        .headers
        .get("retry-after")
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=60).contains(&retry), "{retry}");
    // Other accounts are unaffected by this account's bucket.
    assert_eq!(
        c.post(
            "/api/v1/auth/signin",
            json!({"email": "other@example.org", "password": GOOD_PASSWORD})
        )
        .await
        .status,
        401
    );
    // After the refill the account can sign in again.
    app.clock.advance(chrono::Duration::seconds(61));
    assert_eq!(
        c.post("/api/v1/auth/signin", signup_body("player@example.org"))
            .await
            .status,
        200
    );

    // Per address: one source guessing many accounts is stopped by its own bucket.
    let noisy = app.client().with_ip("192.0.2.7");
    let mut statuses = Vec::new();
    for i in 0..6 {
        let r = noisy
            .post(
                "/api/v1/auth/signin",
                json!({"email": format!("guess{i}@example.org"), "password": "wrong wrong wrong"}),
            )
            .await;
        statuses.push(r.status);
    }
    assert_eq!(statuses, vec![401, 401, 401, 401, 429, 429]);
}

#[tokio::test]
async fn sign_up_is_rate_limited_per_address() {
    let app = TestApp::builder()
        .config(|c| {
            c.rate_limits.rules.insert(
                "signup_ip".into(),
                Rule {
                    capacity: 2,
                    refill_every: Duration::from_secs(60),
                },
            );
        })
        .build()
        .await;
    let c = app.client().with_ip("192.0.2.50");
    assert_eq!(
        c.post("/api/v1/auth/signup", signup_body("a@example.org"))
            .await
            .status,
        201
    );
    assert_eq!(
        c.post("/api/v1/auth/signup", signup_body("b@example.org"))
            .await
            .status,
        201
    );
    let blocked = c
        .post("/api/v1/auth/signup", signup_body("c@example.org"))
        .await;
    assert_eq!(
        (blocked.status, blocked.error_code()),
        (429, "rate_limited")
    );
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 2);
    // Another address is fine.
    let other = app.client().with_ip("192.0.2.51");
    assert_eq!(
        other
            .post("/api/v1/auth/signup", signup_body("c@example.org"))
            .await
            .status,
        201
    );
}

#[tokio::test]
async fn forwarded_addresses_are_ignored_unless_a_proxy_is_trusted() {
    let app = TestApp::builder()
        .config(|c| {
            c.trust_forwarded_for = false;
            c.rate_limits.rules.insert(
                "signin_ip".into(),
                Rule {
                    capacity: 2,
                    refill_every: Duration::from_secs(60),
                },
            );
        })
        .build()
        .await;
    // Rotating the header does not escape the limit when the header is not trusted.
    let mut last = 0;
    for i in 0..4 {
        let c = app.client().with_ip(&format!("10.1.1.{i}"));
        last = c
            .post(
                "/api/v1/auth/signin",
                json!({"email": "x@example.org", "password": "wrong wrong wrong"}),
            )
            .await
            .status;
    }
    assert_eq!(last, 429);
}

#[tokio::test]
async fn state_changing_requests_from_other_origins_are_refused() {
    let app = TestApp::builder()
        .config(|c| c.allowed_origins = vec!["http://localhost:5173".to_owned()])
        .build()
        .await;
    let mut c = app.client();
    c.origin = Some("https://evil.example".to_owned());
    let resp = c
        .post("/api/v1/auth/signup", signup_body("player@example.org"))
        .await;
    assert_eq!(
        (resp.status, resp.error_code()),
        (403, "origin_not_allowed")
    );
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 0);

    // The browser's own declaration is honoured when no Origin is sent.
    let plain = app.client();
    let resp = plain
        .send(
            "POST",
            "/api/v1/auth/signup",
            Some(signup_body("player@example.org")),
            &[("sec-fetch-site", "cross-site")],
        )
        .await;
    assert_eq!(resp.status, 403);

    // The service's own origin and the configured dev origin pass; reads are never blocked.
    for (i, origin) in ["http://localhost:8080", "http://localhost:5173"]
        .into_iter()
        .enumerate()
    {
        let mut ok = app.client();
        ok.origin = Some(origin.to_owned());
        let email = format!("p{i}@example.org");
        assert_eq!(
            ok.post("/api/v1/auth/signup", signup_body(&email))
                .await
                .status,
            201,
            "{origin}"
        );
    }
    let mut reader = app.client();
    reader.origin = Some("https://evil.example".to_owned());
    assert_eq!(reader.get("/api/v1/auth/me").await.status, 401);

    // A project mutation from another origin is refused even with a valid session.
    let user = app.user("owner@example.org").await;
    let id = user.create_project("Mine").await;
    let mut cross = Client::clone_session(&user.client, &app);
    cross.origin = Some("https://evil.example".to_owned());
    let resp = cross.delete(&format!("/api/v1/projects/{id}")).await;
    assert_eq!(resp.status, 403);
    assert_eq!(
        user.client
            .get(&format!("/api/v1/projects/{id}"))
            .await
            .body["project"]["deleted_at"],
        json!(null)
    );
}

trait CloneSession {
    fn clone_session(from: &Client, app: &TestApp) -> Client;
}

impl CloneSession for Client {
    fn clone_session(from: &Client, app: &TestApp) -> Client {
        let c = app.client();
        c.set_cookie("hf_session", &from.cookie("hf_session").unwrap());
        c
    }
}
