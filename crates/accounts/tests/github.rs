//! GitHub sign-in through the OAuth web flow, against a local fake GitHub (no real network).

mod common;

use common::*;
use serde_json::json;

const CALLBACK: &str = "/api/v1/auth/github/callback";

/// Runs start + callback for `code` and returns the callback response.
async fn complete(client: &Client, code: &str) -> Resp {
    let start = client.get("/api/v1/auth/github/start").await;
    assert_eq!(start.status, 302, "{}", start.text);
    let state = authorize_state(&start);
    client
        .get(&format!("{CALLBACK}?code={code}&state={state}"))
        .await
}

fn authorize_state(start: &Resp) -> String {
    let location = start.location().expect("redirect to GitHub");
    url_param(&location, "state")
}

fn url_param(url: &str, key: &str) -> String {
    let query = url.split_once('?').expect("query string").1;
    query
        .split('&')
        .find_map(|kv| kv.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("{key} missing in {url}"))
        .to_owned()
}

async fn dump_everything(app: &TestApp) -> String {
    let mut out = String::new();
    for table in [
        "accounts",
        "sessions",
        "email_verification_tokens",
        "projects",
        "project_members",
    ] {
        let rows: Option<String> =
            sqlx::query_scalar(&format!("SELECT string_agg(t::text, ' ') FROM {table} t"))
                .fetch_one(&app.db.admin)
                .await
                .unwrap();
        out.push_str(&rows.unwrap_or_default());
    }
    out
}

#[tokio::test]
async fn start_redirects_to_github_with_state_cookie_and_without_secrets() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    let client = app.client();
    let start = client.get("/api/v1/auth/github/start").await;
    assert_eq!(start.status, 302);
    let location = start.location().unwrap();
    assert!(
        location.starts_with(&format!("{}/login/oauth/authorize?", fake.base)),
        "{location}"
    );
    assert_eq!(url_param(&location, "client_id"), GITHUB_CLIENT_ID);
    assert_eq!(url_param(&location, "state").len(), 43);
    assert!(
        url_param(&location, "redirect_uri").contains("%2Fapi%2Fv1%2Fauth%2Fgithub%2Fcallback")
    );
    assert!(
        !location.contains(&fake.client_secret),
        "the client secret never leaves the server"
    );

    let cookie = start.set_cookie("hf_gh_state").expect("state cookie");
    for attr in [
        "HttpOnly",
        "SameSite=Lax",
        "Path=/api/v1/auth/github",
        "Max-Age=600",
    ] {
        assert!(cookie.contains(attr), "{cookie}");
    }
    assert!(!cookie.contains("Secure"), "development");
    assert_eq!(
        client.cookie("hf_gh_state").unwrap(),
        url_param(&location, "state")
    );
    // Each start gets a fresh state.
    let other = app.client().get("/api/v1/auth/github/start").await;
    assert_ne!(
        url_param(&other.location().unwrap(), "state"),
        url_param(&location, "state")
    );

    let prod = TestApp::builder().production().github().build().await;
    let start = prod.client().get("/api/v1/auth/github/start").await;
    assert!(start.set_cookie("hf_gh_state").unwrap().contains("Secure"));
}

#[tokio::test]
async fn callback_creates_a_verified_account_and_never_stores_the_github_token() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    let code = fake.grant_code(FakeUser::verified(4242, "octocat", "Octo.Cat@Example.org"));
    let client = app.client();
    let resp = complete(&client, &code).await;
    assert_eq!(resp.status, 302, "{}", resp.text);
    assert_eq!(resp.location().as_deref(), Some("/"));
    let session = resp.set_cookie("hf_session").expect("session cookie");
    assert!(session.contains("HttpOnly") && session.contains("SameSite=Lax"));
    // The one-time state cookie is cleared.
    assert!(
        resp.set_cookie("hf_gh_state")
            .unwrap()
            .contains("Max-Age=0")
    );
    assert!(client.cookie("hf_gh_state").is_none());

    let me = client.get("/api/v1/auth/me").await;
    assert_eq!(me.status, 200);
    let account = &me.body["account"];
    assert_eq!(
        account["email"], "Octo.Cat@Example.org",
        "kept as GitHub reported it"
    );
    assert_eq!(
        account["email_verified"], true,
        "GitHub sign-in counts as verified"
    );
    assert_eq!(account["github_linked"], true);
    assert_eq!(account["has_password"], false);
    assert_eq!(account["display_name"], "octocat (GitHub)");
    assert_eq!(fake.token_requests(), 1);
    assert_eq!(
        fake.last_secret_seen().as_deref(),
        Some(fake.client_secret.as_str()),
        "secret sent to the token endpoint"
    );
    assert!(
        app.mailer.sent().is_empty(),
        "no verification mail for a verified account"
    );

    // Neither the GitHub user token nor the client secret is in any response or table.
    let tokens = fake.issued_tokens();
    assert_eq!(tokens.len(), 1);
    let stored = dump_everything(&app).await;
    for secret in [
        tokens[0].as_str(),
        fake.client_secret.as_str(),
        code.as_str(),
    ] {
        assert!(!stored.contains(secret), "secret leaked into the database");
        assert!(!resp.text.contains(secret) && !me.text.contains(secret));
        assert!(!format!("{:?}", resp.headers).contains(secret));
    }
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns WHERE table_name = 'accounts'",
    )
    .fetch_all(&app.db.admin)
    .await
    .unwrap();
    assert!(!columns.iter().any(|c| c.contains("token")), "{columns:?}");
    // Verified GitHub accounts pass the verified gate.
    let token = client.cookie("hf_session").unwrap();
    app.state
        .authenticate(&token)
        .await
        .unwrap()
        .require_verified()
        .unwrap();
}

#[tokio::test]
async fn signing_in_again_reuses_the_account() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    let first = app.client();
    complete(
        &first,
        &fake.grant_code(FakeUser::verified(7, "octocat", "octo@example.org")),
    )
    .await;
    let id = first.get("/api/v1/auth/me").await.body["account"]["id"].clone();
    first.post_empty("/api/v1/auth/signout").await;

    // The same GitHub user, even with a different address on GitHub now.
    let second = app.client();
    let resp = complete(
        &second,
        &fake.grant_code(FakeUser::verified(7, "octocat", "changed@example.org")),
    )
    .await;
    assert_eq!(resp.status, 302);
    assert_eq!(
        second.get("/api/v1/auth/me").await.body["account"]["id"],
        id
    );
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 1);
    assert_eq!(app.count("SELECT count(*) FROM sessions").await, 1);
}

#[tokio::test]
async fn matching_verified_email_links_to_the_existing_account() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    let user = app.user("player@example.org").await;
    app.client()
        .post(
            "/api/v1/auth/verify-email",
            json!({"token": token_from_mail(&app.mailer.last_to("player@example.org").unwrap())}),
        )
        .await;
    let project = user.create_project("Existing work").await;

    let browser = app.client();
    let resp = complete(
        &browser,
        &fake.grant_code(FakeUser::verified(99, "player", "player@example.org")),
    )
    .await;
    assert_eq!(resp.status, 302, "{}", resp.text);
    let me = browser.get("/api/v1/auth/me").await;
    assert_eq!(me.body["account"]["id"], user.id.to_string());
    assert_eq!(me.body["account"]["github_linked"], true);
    assert_eq!(
        me.body["account"]["has_password"], true,
        "a verified account keeps its password"
    );
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 1);
    // The linked sign-in sees the same projects.
    let list = browser.get("/api/v1/projects").await;
    assert_eq!(list.body["projects"][0]["id"], project);
    // And the original browser session is untouched.
    assert_eq!(user.client.get("/api/v1/auth/me").await.status, 200);
}

#[tokio::test]
async fn an_unverified_look_alike_account_cannot_hijack_a_github_sign_in() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    // Someone registers the victim's address first and never verifies it.
    let squatter = app.user("victim@example.org").await;
    squatter.create_project("planted").await;

    // The real owner proves the address through GitHub.
    let victim = app.client();
    let resp = complete(
        &victim,
        &fake.grant_code(FakeUser::verified(500, "victim", "victim@example.org")),
    )
    .await;
    assert_eq!(resp.status, 302, "{}", resp.text);
    let me = victim.get("/api/v1/auth/me").await;
    assert_eq!(me.body["account"]["email_verified"], true);
    assert_eq!(
        me.body["account"]["has_password"], false,
        "the squatter's password is gone"
    );

    // The squatter's session and password no longer work.
    assert_eq!(squatter.client.get("/api/v1/auth/me").await.status, 401);
    let attempt = app
        .client()
        .post(
            "/api/v1/auth/signin",
            json!({"email": "victim@example.org", "password": GOOD_PASSWORD}),
        )
        .await;
    assert_eq!(
        (attempt.status, attempt.error_code()),
        (401, "invalid_credentials")
    );
    // The squatter's pending verification link is dead as well.
    let stale = token_from_mail(&app.mailer.last_to("victim@example.org").unwrap());
    assert_eq!(
        app.client()
            .post("/api/v1/auth/verify-email", json!({"token": stale}))
            .await
            .error_code(),
        "invalid_token"
    );
}

#[tokio::test]
async fn github_emails_that_are_missing_unverified_or_secondary_are_not_trusted() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    let cases = [
        // The App lacks the email permission.
        FakeUser {
            id: 1,
            login: "noperm".into(),
            name: None,
            emails: None,
        },
        // Only an unverified primary.
        FakeUser {
            id: 2,
            login: "unverified".into(),
            name: None,
            emails: Some(vec![("u@example.org".into(), true, false)]),
        },
        // A verified address that is not the primary one.
        FakeUser {
            id: 3,
            login: "secondary".into(),
            name: None,
            emails: Some(vec![
                ("s@example.org".into(), false, true),
                ("p@example.org".into(), true, false),
            ]),
        },
        FakeUser {
            id: 4,
            login: "empty".into(),
            name: None,
            emails: Some(vec![]),
        },
    ];
    for user in cases {
        let client = app.client();
        let resp = complete(&client, &fake.grant_code(user.clone())).await;
        assert_eq!(
            (resp.status, resp.error_code()),
            (422, "github_email_unavailable"),
            "{}",
            user.login
        );
        assert!(resp.set_cookie("hf_session").is_none());
        assert_eq!(client.get("/api/v1/auth/me").await.status, 401);
    }
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 0);
}

#[tokio::test]
async fn a_forged_missing_or_replayed_state_is_refused_before_any_github_call() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    let code = fake.grant_code(FakeUser::verified(1, "octocat", "octo@example.org"));

    // No cookie at all (a link followed in another browser).
    let resp = app
        .client()
        .get(&format!("{CALLBACK}?code={code}&state=whatever"))
        .await;
    assert_eq!(
        (resp.status, resp.error_code()),
        (400, "github_state_mismatch")
    );

    // A cookie that does not match the returned state.
    let client = app.client();
    let start = client.get("/api/v1/auth/github/start").await;
    let real = authorize_state(&start);
    let wrong = client
        .get(&format!("{CALLBACK}?code={code}&state=forged"))
        .await;
    assert_eq!(
        (wrong.status, wrong.error_code()),
        (400, "github_state_mismatch")
    );
    assert!(
        client.cookie("hf_gh_state").is_none(),
        "state is single use even on failure"
    );
    let missing = client.get(&format!("{CALLBACK}?code={code}")).await;
    assert_eq!(missing.error_code(), "github_state_mismatch");

    assert_eq!(fake.token_requests(), 0, "no code was exchanged");
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 0);

    // The matching state works once; replaying the same callback does not.
    let client = app.client();
    client.set_cookie("hf_gh_state", &real);
    let ok = client
        .get(&format!("{CALLBACK}?code={code}&state={real}"))
        .await;
    assert_eq!(ok.status, 302);
    client.set_cookie("hf_gh_state", &real);
    let replay = client
        .get(&format!("{CALLBACK}?code={code}&state={real}"))
        .await;
    assert_eq!(
        (replay.status, replay.error_code()),
        (502, "github_code_rejected")
    );
    assert_eq!(replay.body["error"]["retry_class"], "refresh_preconditions");
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 1);

    // The user pressing "Cancel" on GitHub.
    let client = app.client();
    let start = client.get("/api/v1/auth/github/start").await;
    let state = authorize_state(&start);
    let denied = client
        .get(&format!("{CALLBACK}?error=access_denied&state={state}"))
        .await;
    assert_eq!((denied.status, denied.error_code()), (400, "github_denied"));
}

#[tokio::test]
async fn linking_attaches_github_to_the_signed_in_account_only() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    let alice = app.user("alice@example.org").await;
    let bob = app.user("bob@example.org").await;

    // Linking needs a session.
    assert_eq!(
        app.client()
            .get("/api/v1/auth/github/start?link=1")
            .await
            .status,
        401
    );

    // Alice links her GitHub account (its address differs from her sign-up address).
    let start = alice.client.get("/api/v1/auth/github/start?link=1").await;
    assert_eq!(start.status, 302);
    let state = authorize_state(&start);
    let code = fake.grant_code(FakeUser::verified(
        1001,
        "alice-gh",
        "alice@elsewhere.example",
    ));
    let done = alice
        .client
        .get(&format!("{CALLBACK}?code={code}&state={state}"))
        .await;
    assert_eq!(done.status, 302, "{}", done.text);
    let me = alice.client.get("/api/v1/auth/me").await;
    assert_eq!(me.body["account"]["id"], alice.id.to_string());
    assert_eq!(me.body["account"]["github_linked"], true);
    assert_eq!(
        me.body["account"]["email"], "alice@example.org",
        "linking does not change the address"
    );
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 2);

    // Bob cannot claim the same GitHub account.
    let start = bob.client.get("/api/v1/auth/github/start?link=1").await;
    let state = authorize_state(&start);
    let code = fake.grant_code(FakeUser::verified(
        1001,
        "alice-gh",
        "alice@elsewhere.example",
    ));
    let clash = bob
        .client
        .get(&format!("{CALLBACK}?code={code}&state={state}"))
        .await;
    assert_eq!(
        (clash.status, clash.error_code()),
        (409, "github_already_linked")
    );
    assert_eq!(
        bob.client.get("/api/v1/auth/me").await.body["account"]["github_linked"],
        false
    );

    // A link flow cannot be finished from a different signed-in account's browser.
    let start = alice.client.get("/api/v1/auth/github/start?link=1").await;
    let state = authorize_state(&start);
    let hijack = app.client();
    hijack.set_cookie("hf_gh_state", &alice.client.cookie("hf_gh_state").unwrap());
    hijack.set_cookie("hf_session", &bob.client.cookie("hf_session").unwrap());
    let code = fake.grant_code(FakeUser::verified(1002, "someone", "x@example.org"));
    let resp = hijack
        .get(&format!("{CALLBACK}?code={code}&state={state}"))
        .await;
    assert_eq!(
        (resp.status, resp.error_code()),
        (400, "github_state_mismatch")
    );
}

#[tokio::test]
async fn an_address_owned_by_another_github_identity_is_a_conflict() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    complete(
        &app.client(),
        &fake.grant_code(FakeUser::verified(1, "first", "shared@example.org")),
    )
    .await;
    let resp = complete(
        &app.client(),
        &fake.grant_code(FakeUser::verified(2, "second", "shared@example.org")),
    )
    .await;
    assert_eq!(
        (resp.status, resp.error_code()),
        (409, "github_email_conflict")
    );
    assert_eq!(app.count("SELECT count(*) FROM accounts").await, 1);
}

#[tokio::test]
async fn disabled_accounts_cannot_sign_in_with_github() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    complete(
        &app.client(),
        &fake.grant_code(FakeUser::verified(1, "octocat", "octo@example.org")),
    )
    .await;
    sqlx::query("UPDATE accounts SET disabled_at = now()")
        .execute(&app.db.admin)
        .await
        .unwrap();
    let resp = complete(
        &app.client(),
        &fake.grant_code(FakeUser::verified(1, "octocat", "octo@example.org")),
    )
    .await;
    assert_eq!((resp.status, resp.error_code()), (403, "account_disabled"));
}

#[tokio::test]
async fn bad_client_credentials_and_unreachable_github_are_reported_without_leaking() {
    let app = TestApp::builder().github().build().await;
    let fake = app.github.as_ref().unwrap();
    // A code the fake never issued.
    let resp = complete(&app.client(), "never-issued").await;
    assert_eq!(
        (resp.status, resp.error_code()),
        (502, "github_code_rejected")
    );
    assert!(!resp.text.contains(&fake.client_secret));

    // GitHub down: the token endpoint refuses connections.
    let down = TestApp::builder()
        .github()
        .config(|c| {
            let gh = c.github.as_mut().unwrap();
            gh.token_url = "http://127.0.0.1:1/login/oauth/access_token".to_owned();
        })
        .build()
        .await;
    let resp = complete(&down.client(), "any").await;
    assert_eq!(
        (resp.status, resp.error_code()),
        (502, "github_unreachable")
    );
    assert_eq!(resp.body["error"]["retry_class"], "refresh_preconditions");
    assert!(!resp.text.contains("127.0.0.1"));
}

#[tokio::test]
async fn github_routes_report_when_the_server_has_no_github_app() {
    let app = TestApp::new().await;
    let start = app.client().get("/api/v1/auth/github/start").await;
    assert_eq!(
        (start.status, start.error_code()),
        (404, "github_not_configured")
    );
    let cb = app
        .client()
        .get(&format!("{CALLBACK}?code=x&state=y"))
        .await;
    assert_eq!(cb.status, 400, "state is checked first");
}
