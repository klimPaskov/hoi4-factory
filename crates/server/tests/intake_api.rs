//! Uploads, GitHub import and the workspace descriptor through the assembled server, against a
//! real PostgreSQL (see `http_api.rs` for how to provide one with `FACTORY_TEST_ADMIN_URL`).
//!
//! GitHub is a local stand-in: the intake crate's own test double (`crates/intake/tests/common`,
//! included by path) answers the REST calls an App makes, checks the RS256 App JWT and
//! repository-scoped installation tokens, and serves real bare repositories through the real
//! `git http-backend`. Only the lookup of the signed-in user's installation (`GET
//! /app/installations`, which the double does not implement) is replaced by a fixed list here,
//! and `HttpAppInstallations` is exercised separately against a tiny stub.

#[path = "../../intake/tests/common/mod.rs"]
mod common;

include!("support/harness.rs");

// ----- the mod used by the upload tests ------------------------------------------------------

const DESCRIPTOR_MOD: &[u8] = b"\xEF\xBB\xBFname=\"Synthetic\"\r\nsupported_version=\"1.19.*\"\r\n";
const FOCUS_TXT: &[u8] = b"focus_tree = {\r\n\tid = x\r\n}\r\n";
const LF_TXT: &[u8] = b"one\ntwo\n";

fn mod_entries() -> Vec<ZEntry> {
    vec![
        ZEntry::file("descriptor.mod", DESCRIPTOR_MOD),
        ZEntry::file("common/national_focus/x.txt", FOCUS_TXT),
        ZEntry::file("notes.txt", LF_TXT),
    ]
}

fn entry_sha(descriptor: &Value, path: &str) -> String {
    descriptor["source_snapshot"]["inventory"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["path"] == path)
        .unwrap_or_else(|| panic!("{path} is in the inventory"))["sha256"]
        .as_str()
        .unwrap()
        .to_owned()
}

// ----- uploads -------------------------------------------------------------------------------

#[tokio::test]
async fn a_zip_upload_becomes_a_workspace_with_its_bytes_intact() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Synthetic").await;

    // Before anything is uploaded the workspace is empty, not an error.
    let empty = alice
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;
    assert_eq!(empty.status, 200, "{}", empty.text);
    assert_eq!(empty.body["summary"]["has_source"], false);
    assert!(empty.body["descriptor"].is_null());

    let zip = build_zip(&mod_entries());
    let resp = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "zip"),
                Part::text("git_init_consent", "true"),
                Part::file("synthetic.zip", zip),
            ],
        )
        .await;
    assert_eq!(resp.status, 201, "{}", resp.text);
    assert_eq!(resp.body["upload"]["kind"], "zip");
    assert_eq!(resp.body["upload"]["extracted"]["files"], 3);
    assert_eq!(resp.body["upload"]["git"]["state"], "initialised");
    assert_eq!(resp.body["workspace"]["files"], 3);
    assert_eq!(
        resp.body["workspace"]["source_locator"],
        format!("workspace:{project}/source")
    );

    // The bytes on disk are the uploaded bytes: BOM and CRLF untouched, LF stays LF.
    let source = h.workspace_source(&alice, &project);
    assert_eq!(
        std::fs::read(source.join("descriptor.mod")).unwrap(),
        DESCRIPTOR_MOD
    );
    assert_eq!(
        std::fs::read(source.join("common/national_focus/x.txt")).unwrap(),
        FOCUS_TXT
    );
    assert_eq!(std::fs::read(source.join("notes.txt")).unwrap(), LF_TXT);

    // The descriptor reports the same bytes by hash, a clean Git state and logical locators.
    let ws = alice
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;
    assert_eq!(ws.status, 200, "{}", ws.text);
    let summary = &ws.body["summary"];
    assert_eq!(summary["has_source"], true);
    assert_eq!(summary["origin"], "upload");
    assert_eq!(summary["file_count"], 3);
    assert_eq!(summary["ref"], "main");
    assert_eq!(summary["setup_state"], "not_started");
    assert_eq!(summary["revision"].as_str().unwrap().len(), 12);
    let descriptor = &ws.body["descriptor"];
    assert_eq!(descriptor["schema_version"], 1);
    assert_eq!(descriptor["workspace_id"], project);
    assert_eq!(descriptor["tenant"]["account_id"], alice.id);
    assert_eq!(descriptor["tenant"]["project_id"], project);
    assert_eq!(descriptor["tenant"]["member_roles"][0]["role"], "owner");
    assert_eq!(
        entry_sha(descriptor, "descriptor.mod"),
        sha256_hex(DESCRIPTOR_MOD)
    );
    assert_eq!(
        entry_sha(descriptor, "common/national_focus/x.txt"),
        sha256_hex(FOCUS_TXT)
    );
    assert_eq!(descriptor["git_state"]["is_repository"], true);
    assert_eq!(descriptor["git_state"]["branch"], "main");
    assert!(
        descriptor["source_snapshot"]["dirty_files"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let handles = descriptor["root_handles"].as_array().unwrap();
    assert!(handles.iter().any(
        |r| r["kind"] == "mod_source" && r["locator"] == format!("workspace:{project}/source")
    ));
    assert!(handles.iter().any(|r| r["kind"] == "game_reference"
        && r["locator"] == "/game"
        && r["access"] == "read_only"));
    // The server game reference is in the profile (identity metadata only).
    assert_eq!(
        descriptor["game_profile"]["reference"]["raw_version"],
        "1.19.3.0"
    );

    // No host path leaves the server.
    for host in [
        h.data_dir.path().to_string_lossy().replace('\\', "\\\\"),
        h.game_dir.path().to_string_lossy().replace('\\', "\\\\"),
        h.data_dir.path().to_string_lossy().into_owned(),
    ] {
        assert!(
            !ws.text.contains(&host),
            "host path {host} in the descriptor"
        );
        assert!(
            !resp.text.contains(&host),
            "host path {host} in the upload response"
        );
    }

    // The project owns one workspace: a second upload is a conflict and changes nothing.
    let again = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "zip"),
                Part::file("x.zip", build_zip(&mod_entries())),
            ],
        )
        .await;
    assert_eq!(
        (again.status, again.error_code()),
        (409, "workspace_exists")
    );
    assert_eq!(std::fs::read(source.join("notes.txt")).unwrap(), LF_TXT);
}

#[tokio::test]
async fn a_folder_upload_keeps_relative_paths_and_skips_git_without_consent() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Folder").await;
    let resp = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "folder"),
                Part::file("MyMod/descriptor.mod", DESCRIPTOR_MOD),
                Part::file("MyMod/common/x.txt", FOCUS_TXT),
            ],
        )
        .await;
    assert_eq!(resp.status, 201, "{}", resp.text);
    assert_eq!(resp.body["upload"]["kind"], "folder");
    assert_eq!(resp.body["upload"]["git"]["state"], "not_initialised");
    let source = h.workspace_source(&alice, &project);
    // The folder name is not part of the mod: the mod root is the folder.
    let listing: Vec<_> = std::fs::read_dir(&source)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(!listing.is_empty(), "{listing:?}");
    let descriptor_path = if source.join("descriptor.mod").exists() {
        source.join("descriptor.mod")
    } else {
        source.join("MyMod/descriptor.mod")
    };
    assert_eq!(std::fs::read(descriptor_path).unwrap(), DESCRIPTOR_MOD);
    let ws = alice
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;
    assert_eq!(ws.body["summary"]["origin"], "upload");
    assert_eq!(ws.body["descriptor"]["git_state"]["is_repository"], false);
}

#[tokio::test]
async fn hostile_and_malformed_uploads_are_refused_with_stable_codes() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Hostile").await;
    let ws_path = format!("/api/v1/projects/{project}/workspace");

    // A traversal entry in a ZIP.
    let zip = build_zip(&[
        ZEntry::file("descriptor.mod", DESCRIPTOR_MOD),
        ZEntry::file("../escape.txt", b"x".to_vec()),
    ]);
    let resp = alice
        .client
        .upload(
            &project,
            &[Part::text("kind", "zip"), Part::file("evil.zip", zip)],
        )
        .await;
    assert_eq!(
        (resp.status, resp.error_code()),
        (422, "path_rejected"),
        "{}",
        resp.text
    );
    assert_eq!(resp.body["error"]["retry_class"], "terminal");
    assert_eq!(resp.body["error"]["affected_ids"]["project_id"], project);
    assert!(resp.body["error"]["affected_ids"]["upload_id"].is_string());

    // A traversal path in a folder upload.
    let resp = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "folder"),
                Part::file("../outside.txt", b"x".to_vec()),
            ],
        )
        .await;
    assert_eq!(
        (resp.status, resp.error_code()),
        (422, "path_rejected"),
        "{}",
        resp.text
    );

    // An executable.
    let resp = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "folder"),
                Part::file("tool.exe", b"MZ".to_vec()),
            ],
        )
        .await;
    assert_eq!(
        (resp.status, resp.error_code()),
        (422, "executable_refused")
    );

    // Not a ZIP.
    let resp = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "zip"),
                Part::file("fake.zip", b"plain text".to_vec()),
            ],
        )
        .await;
    assert_eq!((resp.status, resp.error_code()), (422, "not_a_zip_archive"));

    // Request-level mistakes.
    let none = alice
        .client
        .upload(&project, &[Part::text("kind", "zip")])
        .await;
    assert_eq!((none.status, none.error_code()), (400, "invalid_request"));
    let unknown = alice
        .client
        .upload(&project, &[Part::text("surprise", "1")])
        .await;
    assert_eq!(
        (unknown.status, unknown.error_code()),
        (400, "invalid_request")
    );
    let two_zips = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "zip"),
                Part::file("a.zip", build_zip(&mod_entries())),
                Part::file("b.zip", build_zip(&mod_entries())),
            ],
        )
        .await;
    assert_eq!(
        (two_zips.status, two_zips.error_code()),
        (400, "invalid_request")
    );
    let bad_kind = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "tarball"),
                Part::file("a", b"x".to_vec()),
            ],
        )
        .await;
    assert_eq!(bad_kind.status, 400);

    // Nothing was created, nothing escaped, and every refusal was recorded.
    let ws = alice.client.get(&ws_path).await;
    assert_eq!(ws.body["summary"]["has_source"], false);
    assert!(!h.data_dir.path().join("outside.txt").exists());
    assert!(
        !h.data_dir
            .path()
            .join("workspaces")
            .join("escape.txt")
            .exists()
    );
    let admin = h.admin().await;
    let rejected: Vec<String> = sqlx::query_scalar(
        "SELECT rejection_code FROM intake_uploads WHERE status = 'rejected' ORDER BY created_at",
    )
    .fetch_all(&admin)
    .await
    .unwrap();
    assert_eq!(
        rejected,
        [
            "path_rejected",
            "path_rejected",
            "executable_refused",
            "not_a_zip_archive"
        ]
    );
    let stuck: i64 =
        sqlx::query_scalar("SELECT count(*) FROM intake_uploads WHERE status = 'receiving'")
            .fetch_one(&admin)
            .await
            .unwrap();
    assert_eq!(stuck, 0);
    // The spool is empty again.
    let spool: Vec<_> = std::fs::read_dir(&h.state.intake.incoming_dir)
        .unwrap()
        .collect();
    assert!(spool.is_empty(), "{spool:?}");

    // The project is still usable after all of that.
    let ok = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "zip"),
                Part::file("ok.zip", build_zip(&mod_entries())),
            ],
        )
        .await;
    assert_eq!(ok.status, 201, "{}", ok.text);
}

/// Deterministic bytes that do not compress.
fn noise(len: usize) -> Vec<u8> {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

#[tokio::test]
async fn only_the_upload_route_has_the_large_body_limit() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Large").await;

    // 2 MiB is above the 1 MiB default of every other route and fine for an upload.
    let big = noise(2 * 1024 * 1024);
    let resp = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "folder"),
                Part::file("gfx/big.dds", big.clone()),
            ],
        )
        .await;
    assert_eq!(resp.status, 201, "{}", resp.text);
    assert_eq!(
        std::fs::read(h.workspace_source(&alice, &project).join("gfx/big.dds")).unwrap(),
        big
    );

    // A JSON route still stops at the default.
    let huge = json!({ "name": "x".repeat(2 * 1024 * 1024) });
    let refused = alice.client.post("/api/v1/projects", huge).await;
    assert!((400..500).contains(&refused.status), "{}", refused.text);
    let listed = alice.client.get("/api/v1/projects").await;
    assert_eq!(listed.body["projects"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn an_upload_over_the_configured_limit_is_refused_and_cleaned_up() {
    let h = Harness::start(Options {
        upload_max_bytes: Some(64 * 1024),
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Small").await;
    let resp = alice
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "folder"),
                Part::file("gfx/big.dds", noise(256 * 1024)),
            ],
        )
        .await;
    assert_eq!(resp.status, 413, "{}", resp.text);
    assert_eq!(resp.error_code(), "upload_too_large");
    assert!(!h.workspace_source(&alice, &project).exists());
    let spool: Vec<_> = std::fs::read_dir(&h.state.intake.incoming_dir)
        .unwrap()
        .collect();
    assert!(spool.is_empty(), "{spool:?}");
}

#[tokio::test]
async fn cross_account_and_signed_out_requests_get_the_same_404_or_401() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let bob = h.verified_user("bob@example.org").await;
    let project = h.project(&alice, "Alice's").await;
    let zip = build_zip(&mod_entries());
    let ok = alice
        .client
        .upload(
            &project,
            &[Part::text("kind", "zip"), Part::file("a.zip", zip.clone())],
        )
        .await;
    assert_eq!(ok.status, 201, "{}", ok.text);

    let missing = Uuid::new_v4().to_string();
    let calls: Vec<(&str, String)> = vec![
        ("GET", "workspace".into()),
        ("GET", "github/repositories".into()),
        ("GET", "github/link".into()),
        ("POST", "github/import".into()),
        ("POST", "uploads".into()),
    ];
    for (method, tail) in &calls {
        let foreign = hit(&bob.client, method, tail, &project).await;
        let absent = hit(&bob.client, method, tail, &missing).await;
        assert_eq!(
            (foreign.status, foreign.error_code()),
            (404, "not_found"),
            "{method} {tail}: {}",
            foreign.text
        );
        assert_eq!(
            (foreign.status, foreign.body.clone()),
            (absent.status, absent.body.clone()),
            "{method} {tail}: a foreign project looks exactly like a missing one"
        );
        let anonymous = hit(&h.client(), method, tail, &project).await;
        assert_eq!(
            (anonymous.status, anonymous.error_code()),
            (401, "unauthenticated"),
            "{method} {tail}"
        );
    }

    // Bob made no workspace for himself, and Alice's is untouched.
    let ws = alice
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;
    assert_eq!(ws.body["summary"]["file_count"], 3);
    let bobs_dir = h.data_dir.path().join("workspaces").join(&bob.id);
    assert!(!bobs_dir.exists());

    // A public project of another account is readable by the accounts API, not by these routes.
    let patch = alice
        .client
        .send_raw(
            "PATCH",
            &format!("/api/v1/projects/{project}"),
            Some("application/json"),
            json!({"visibility": "public"}).to_string().into_bytes(),
            &[],
        )
        .await;
    assert_eq!(patch.status, 200, "{}", patch.text);
    let public = bob
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;
    assert_eq!((public.status, public.error_code()), (404, "not_found"));
}

#[tokio::test]
async fn a_cross_site_upload_is_refused_before_anything_is_read() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Csrf").await;
    let parts = [
        Part::text("kind", "zip"),
        Part::file("a.zip", build_zip(&mod_entries())),
    ];
    let evil = alice
        .client
        .upload_with(&project, &parts, &[("origin", "https://evil.example")])
        .await;
    assert_eq!(
        (evil.status, evil.error_code()),
        (403, "origin_not_allowed")
    );
    let cross = alice
        .client
        .upload_with(&project, &parts, &[("sec-fetch-site", "cross-site")])
        .await;
    assert_eq!(cross.status, 403);
    assert!(!h.workspace_source(&alice, &project).exists());
    let vite = alice
        .client
        .upload_with(&project, &parts, &[("origin", "http://localhost:5173")])
        .await;
    assert_eq!(vite.status, 201, "{}", vite.text);
}

// ----- GitHub --------------------------------------------------------------------------------

#[tokio::test]
async fn github_import_through_the_app_installation() {
    let gh = github_fixture().await;
    let h = Harness::start(Options {
        github: Some(gh),
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "From GitHub").await;
    let base = format!("/api/v1/projects/{project}");

    // An account that never linked GitHub gets a precise error and the install link.
    let unlinked = alice
        .client
        .get(&format!("{base}/github/repositories"))
        .await;
    assert_eq!(
        (unlinked.status, unlinked.error_code()),
        (409, "github_not_linked")
    );
    assert_eq!(
        unlinked.body["error"]["details"]["install_url"],
        INSTALL_URL
    );
    assert_eq!(
        unlinked.body["error"]["retry_class"],
        "requires_authorization"
    );

    // Linked, but the App is not installed on that GitHub account.
    h.link_github(&alice, 424_242).await;
    let none = alice
        .client
        .get(&format!("{base}/github/repositories"))
        .await;
    assert_eq!(
        (none.status, none.error_code()),
        (409, "github_app_not_installed")
    );
    assert_eq!(none.body["error"]["details"]["install_url"], INSTALL_URL);

    // Installed on all repositories: refused, nothing is listed or allowed.
    *h.installations.list.lock().unwrap() = vec![installation(424_242, "all")];
    let all = alice
        .client
        .get(&format!("{base}/github/repositories"))
        .await;
    assert_eq!(
        (all.status, all.error_code()),
        (403, "installation_not_selected")
    );
    let import_all = alice
        .client
        .post(
            &format!("{base}/github/import"),
            json!({"repository": REPO}),
        )
        .await;
    assert_eq!(import_all.error_code(), "installation_not_selected");
    let admin = h.admin().await;
    let allowed: i64 = sqlx::query_scalar("SELECT count(*) FROM github_repository_allowlist")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(allowed, 0, "a refused import adds nothing to the allowlist");

    // Someone else's installation does not count.
    *h.installations.list.lock().unwrap() = vec![installation(999, "selected")];
    let other = alice
        .client
        .get(&format!("{base}/github/repositories"))
        .await;
    assert_eq!(other.error_code(), "github_app_not_installed");

    // The real case: a selected-repositories installation of Alice's GitHub account.
    *h.installations.list.lock().unwrap() = vec![installation(424_242, "selected")];
    let listed = alice
        .client
        .get(&format!("{base}/github/repositories"))
        .await;
    assert_eq!(listed.status, 200, "{}", listed.text);
    let names: Vec<&str> = listed.body["repositories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["full_name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["klimPaskov/hoi4-factory-test-mod", "klimPaskov/other-repo"]
    );
    let first = &listed.body["repositories"][0];
    assert_eq!(first["default_branch"], "main");
    assert_eq!(first["private"], true);
    assert_eq!(first["allowed"], false);
    assert_eq!(listed.body["install_url"], INSTALL_URL);
    assert!(
        !listed.text.contains("ghs_"),
        "no token reaches the browser"
    );

    // No link yet.
    let nolink = alice.client.get(&format!("{base}/github/link")).await;
    assert_eq!(nolink.status, 200);
    assert!(nolink.body["link"].is_null());

    // A repository outside the installation is refused, and the allowlist does not keep it.
    let outside = alice
        .client
        .post(
            &format!("{base}/github/import"),
            json!({"repository": "klimPaskov/not-installed"}),
        )
        .await;
    assert_eq!(outside.status, 403, "{}", outside.text);
    assert_eq!(outside.error_code(), "repository_not_in_installation");
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM github_repository_allowlist")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(kept, 0);
    let bad_name = alice
        .client
        .post(
            &format!("{base}/github/import"),
            json!({"repository": "no-slash"}),
        )
        .await;
    assert_eq!(
        (bad_name.status, bad_name.error_code()),
        (400, "invalid_request")
    );
    let bad_branch = alice
        .client
        .post(
            &format!("{base}/github/import"),
            json!({"repository": REPO, "ref": "nope"}),
        )
        .await;
    assert_eq!(
        (bad_branch.status, bad_branch.error_code()),
        (422, "branch_not_found")
    );

    // Importing the chosen repository allows it and imports the default branch.
    let done = alice
        .client
        .post(
            &format!("{base}/github/import"),
            json!({"repository": REPO}),
        )
        .await;
    assert_eq!(done.status, 201, "{}", done.text);
    assert_eq!(done.body["link"]["state"], "active");
    assert_eq!(done.body["link"]["imported_branch"], "main");
    assert_eq!(done.body["link"]["repository_full_name"], REPO);
    assert_eq!(
        done.body["import"]["head"],
        h.github.as_ref().unwrap().remote.main_tip
    );
    assert!(done.body["link"].get("installation_ref").is_none());
    assert!(done.body["link"].get("account_id").is_none());
    assert_eq!(
        done.body["workspace"]["git"]["state"],
        "existing_repository"
    );

    // The workspace is the remote branch byte for byte: .gitattributes did not rewrite anything.
    let source = h.workspace_source(&alice, &project);
    assert_eq!(
        std::fs::read(source.join("notes.txt")).unwrap(),
        common::fake_github::NOTES_TXT
    );
    assert_eq!(
        std::fs::read(source.join("data/legacy.csv")).unwrap(),
        common::fake_github::LEGACY_CSV
    );
    assert_eq!(
        std::fs::read(source.join("interface/bom.gui")).unwrap(),
        common::fake_github::BOM_GUI
    );

    let ws = alice.client.get(&format!("{base}/workspace")).await;
    assert_eq!(ws.body["summary"]["origin"], "github");
    assert_eq!(ws.body["summary"]["ref"], "main");
    assert_eq!(
        ws.body["summary"]["revision"].as_str().unwrap(),
        &h.github.as_ref().unwrap().remote.main_tip[..12]
    );
    let remote = &ws.body["descriptor"]["remote_link"];
    assert!(
        remote
            .to_string()
            .contains("github.com/klimPaskov/hoi4-factory-test-mod"),
        "{remote}"
    );
    assert!(!ws.text.contains("x-access-token"));
    assert!(!ws.text.contains("ghs_"));

    // The repository list now flags it, the link is live, and a second import is a conflict.
    let relisted = alice
        .client
        .get(&format!("{base}/github/repositories"))
        .await;
    assert_eq!(relisted.body["repositories"][0]["allowed"], true);
    assert_eq!(relisted.body["repositories"][1]["allowed"], false);
    let link = alice.client.get(&format!("{base}/github/link")).await;
    assert_eq!(link.status, 200, "{}", link.text);
    assert_eq!(link.body["link"]["state"], "active");
    assert_eq!(link.body["live_check"], "ok");
    let twice = alice
        .client
        .post(
            &format!("{base}/github/import"),
            json!({"repository": REPO}),
        )
        .await;
    assert_eq!(
        (twice.status, twice.error_code()),
        (409, "project_already_linked")
    );

    // GitHub revokes the connection: the next check says so and keeps the workspace.
    h.github.as_ref().unwrap().fake.with(|s| s.revoked = true);
    let revoked = alice.client.get(&format!("{base}/github/link")).await;
    assert_eq!(revoked.body["live_check"], "revoked");
    assert_eq!(revoked.body["link"]["state"], "access_revoked");
    assert!(source.join("notes.txt").exists());
    let after = alice.client.get(&format!("{base}/github/link")).await;
    assert_eq!(after.body["link"]["state"], "access_revoked");
    assert_eq!(after.body["live_check"], "not_applicable");
}

#[tokio::test]
async fn github_features_report_unavailable_when_the_app_is_not_configured() {
    let h = Harness::start(Options::default()).await;
    assert!(h.state.intake.github.is_none());
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "No GitHub").await;
    let base = format!("/api/v1/projects/{project}");
    let list = alice
        .client
        .get(&format!("{base}/github/repositories"))
        .await;
    assert_eq!(
        (list.status, list.error_code()),
        (503, "github_not_configured")
    );
    let import = alice
        .client
        .post(
            &format!("{base}/github/import"),
            json!({"repository": REPO}),
        )
        .await;
    assert_eq!(
        (import.status, import.error_code()),
        (503, "github_not_configured")
    );
    // The rest of the server is fine.
    let link = alice.client.get(&format!("{base}/github/link")).await;
    assert_eq!(link.status, 200);
    assert!(link.body["link"].is_null());
    let ready = h.client().get("/readyz").await;
    assert_eq!(ready.status, 200, "{}", ready.text);
}

#[tokio::test]
async fn an_unusable_key_file_disables_github_without_stopping_the_server() {
    let gh = github_fixture().await;
    std::fs::write(&gh.key_file, "this is not a PEM").unwrap();
    let h = Harness::start(Options {
        github: Some(gh),
        ..Options::default()
    })
    .await;
    assert!(h.state.intake.github.is_none());
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Bad key").await;
    let list = alice
        .client
        .get(&format!("/api/v1/projects/{project}/github/repositories"))
        .await;
    assert_eq!(list.error_code(), "github_not_configured");
}

#[tokio::test]
async fn the_installation_lookup_calls_github_with_an_app_jwt_and_reads_every_page() {
    use axum::extract::Query;
    use axum::routing::get;
    use std::collections::HashMap;

    type Calls = Arc<Mutex<Vec<(String, Option<String>)>>>;
    let seen: Calls = Arc::default();
    let log = seen.clone();
    let stub = Router::new().route(
        "/app/installations",
        get(
            move |headers: HeaderMap, Query(q): Query<HashMap<String, String>>| {
                let log = log.clone();
                async move {
                    let page: u32 = q["page"].parse().unwrap();
                    log.lock().unwrap().push((
                        format!("page={page}&per_page={}", q["per_page"]),
                        headers
                            .get("authorization")
                            .map(|v| v.to_str().unwrap().to_owned()),
                    ));
                    let count = if page == 1 { 100 } else { 2 };
                    let items: Vec<Value> = (0..count)
                        .map(|i| {
                            let id = u64::from(page) * 1000 + i;
                            json!({"id": id, "repository_selection": "selected",
                               "suspended_at": null,
                               "account": {"id": id + 1, "login": format!("u{id}"), "type": "User"},
                               "extra": "ignored"})
                        })
                        .collect();
                    axum::Json(items)
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, stub).await.unwrap();
    });
    let endpoints = factory_intake::GithubEndpoints::custom(
        &format!("http://127.0.0.1:{port}"),
        &format!("http://127.0.0.1:{port}"),
    )
    .unwrap();
    let client = HttpAppInstallations::new(&endpoints).unwrap();
    let all = client.list("jwt-value").await.unwrap();
    assert_eq!(all.len(), 102);
    assert_eq!(all[0].account.id, 1001);
    let calls = seen.lock().unwrap().clone();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "page=1&per_page=100");
    assert_eq!(calls[1].0, "page=2&per_page=100");
    assert!(
        calls
            .iter()
            .all(|c| c.1.as_deref() == Some("Bearer jwt-value"))
    );

    // A refusal from GitHub is an error, not an empty list.
    let refused = Router::new().route(
        "/app/installations",
        get(|| async { (axum::http::StatusCode::UNAUTHORIZED, "{}") }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, refused).await.unwrap();
    });
    let endpoints = factory_intake::GithubEndpoints::custom(
        &format!("http://127.0.0.1:{port}"),
        &format!("http://127.0.0.1:{port}"),
    )
    .unwrap();
    let err = HttpAppInstallations::new(&endpoints)
        .unwrap()
        .list("x")
        .await
        .unwrap_err();
    assert!(err.is_revocation(), "{err:?}");
}

// ----- migrations ----------------------------------------------------------------------------

const KEYS: &[&str] = &[
    "intake_uploads_account_fk",
    "intake_uploads_project_fk",
    "repository_links_account_fk",
    "repository_links_project_fk",
    "github_repository_allowlist_account_fk",
    "github_repository_allowlist_project_fk",
];

async fn key_names(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT conname FROM pg_constraint WHERE contype = 'f' AND conname = ANY($1) ORDER BY conname",
    )
    .bind(KEYS)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn the_intake_foreign_keys_hold_under_row_level_security_and_the_migration_reverses() {
    let h = Harness::start(Options::default()).await;
    let admin = h.admin().await;
    let mut expected: Vec<String> = KEYS.iter().map(|s| (*s).to_owned()).collect();
    expected.sort();
    assert_eq!(key_names(&admin).await, expected);
    let report = &h.state.migrations;
    assert!(
        report
            .components
            .iter()
            .any(|c| c.name == "intake" && c.latest_version == 303)
    );

    // As the application role, with a valid scope, a row for a project that does not exist (or
    // that belongs to someone else) cannot be inserted.
    let app = PgPool::connect(&url_for(APP_ROLE, &h.database))
        .await
        .unwrap();
    let alice = h.verified_user("alice@example.org").await;
    let bob = h.verified_user("bob@example.org").await;
    let project = h.project(&alice, "Keys").await;
    for (scope, project_id) in [
        (alice.id.clone(), Uuid::new_v4().to_string()),
        (bob.id.clone(), project.clone()),
    ] {
        let mut tx = app.begin().await.unwrap();
        sqlx::query("SELECT set_config('app.account_id', $1, true)")
            .bind(&scope)
            .execute(&mut *tx)
            .await
            .unwrap();
        let err = sqlx::query(
            "INSERT INTO intake_uploads (id, account_id, project_id, kind, status)
             VALUES ($1, $2::uuid, $3::uuid, 'zip', 'receiving')",
        )
        .bind(Uuid::new_v4())
        .bind(&scope)
        .bind(&project_id)
        .execute(&mut *tx)
        .await
        .unwrap_err();
        match err {
            sqlx::Error::Database(db) => assert_eq!(db.code().as_deref(), Some("23503")),
            other => panic!("expected a foreign key violation, got {other:?}"),
        }
    }
    // The real owner can.
    let mut tx = app.begin().await.unwrap();
    sqlx::query("SELECT set_config('app.account_id', $1, true)")
        .bind(&alice.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO github_repository_allowlist (account_id, project_id, repository_full_name)
         VALUES ($1::uuid, $2::uuid, 'a/b')",
    )
    .bind(&alice.id)
    .bind(&project)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // The migration reverses and applies again.
    sqlx::raw_sql(include_str!(
        "../../../migrations/0303_intake_foreign_keys.down.sql"
    ))
    .execute(&admin)
    .await
    .unwrap();
    assert!(key_names(&admin).await.is_empty());
    sqlx::raw_sql(include_str!(
        "../../../migrations/0303_intake_foreign_keys.up.sql"
    ))
    .execute(&admin)
    .await
    .unwrap();
    assert_eq!(key_names(&admin).await, expected);

    // The cascade: removing the project (operator role) removes its intake rows.
    sqlx::query("DELETE FROM project_members WHERE project_id = $1::uuid")
        .bind(&project)
        .execute(&admin)
        .await
        .unwrap();
    sqlx::query("DELETE FROM projects WHERE id = $1::uuid")
        .bind(&project)
        .execute(&admin)
        .await
        .unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM github_repository_allowlist")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(left, 0);
}

/// One call to a project route, with a body that would be valid for the caller who may use it.
async fn hit(client: &Client, method: &str, tail: &str, project: &str) -> Resp {
    let path = format!("/api/v1/projects/{project}/{tail}");
    match (method, tail) {
        ("POST", "uploads") => {
            client
                .upload(
                    project,
                    &[Part::text("kind", "zip"), Part::file("a.zip", Vec::new())],
                )
                .await
        }
        ("POST", _) => client.post(&path, json!({"repository": REPO})).await,
        _ => client.get(&path).await,
    }
}

// ----- Setup routes --------------------------------------------------------------------------

async fn uploaded_project(h: &Harness, user: &User, name: &str) -> String {
    let project = h.project(user, name).await;
    let resp = user
        .client
        .upload(
            &project,
            &[
                Part::text("kind", "zip"),
                Part::text("git_init_consent", "true"),
                Part::file("synthetic.zip", build_zip(&mod_entries())),
            ],
        )
        .await;
    assert_eq!(resp.status, 201, "{}", resp.text);
    project
}

#[tokio::test]
async fn a_scan_of_an_uploaded_mod_is_read_only_and_no_plan_exists_before_the_review() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = uploaded_project(&h, &alice, "Scanned").await;
    let base = format!("/api/v1/projects/{project}/setup");
    let source = h.workspace_source(&alice, &project);
    let before = alice
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;

    // Nothing to plan or apply before a scan.
    let status0 = alice.client.get(&format!("{base}/status")).await;
    assert_eq!(status0.status, 200, "{}", status0.text);
    assert_eq!(status0.body["session_open"], false);
    assert!(status0.body["status"]["latest_scan"].is_null());
    let apply0 = alice
        .client
        .post(&format!("{base}/apply"), json!({"plan_id": Uuid::new_v4()}))
        .await;
    assert_eq!(
        (apply0.status, apply0.error_code()),
        (409, "setup_session_missing")
    );

    // The scan.
    let scan = alice
        .client
        .post(&format!("{base}/scan"), json!({"mode": "existing"}))
        .await;
    assert_eq!(scan.status, 200, "{}", scan.text);
    let scan_id = scan.body["scan_id"].as_str().expect("scan id").to_owned();
    assert_eq!(scan.body["incomplete"], false);
    assert_eq!(scan.body["workspace_unchanged"], true);
    let facts = scan.body["facts"].as_array().unwrap();
    let fact = |label: &str| {
        facts
            .iter()
            .find(|f| f["label"] == label)
            .unwrap_or_else(|| panic!("fact {label} in {facts:?}"))["value"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(fact("Coverage"), "complete");
    assert!(
        fact("Files scanned").parse::<u32>().unwrap() >= 1,
        "{facts:?}"
    );
    assert_eq!(fact("descriptor: mod_name"), "Synthetic");
    let labels: Vec<&str> = facts.iter().map(|f| f["label"].as_str().unwrap()).collect();
    let unique: std::collections::BTreeSet<_> = labels.iter().collect();
    assert_eq!(
        labels.len(),
        unique.len(),
        "fact labels are unique: {labels:?}"
    );
    assert!(
        !scan
            .text
            .contains(&h.data_dir.path().to_string_lossy().into_owned())
    );

    // Read-only: bytes and the descriptor of the workspace are exactly what they were.
    assert_eq!(
        std::fs::read(source.join("descriptor.mod")).unwrap(),
        DESCRIPTOR_MOD
    );
    assert_eq!(std::fs::read(source.join("notes.txt")).unwrap(), LF_TXT);
    let after = alice
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;
    assert_eq!(
        before.body["descriptor"]["source_snapshot"]["inventory_hash"],
        after.body["descriptor"]["source_snapshot"]["inventory_hash"]
    );

    // Status remembers the scan; recovery has nothing to say.
    let status = alice.client.get(&format!("{base}/status")).await;
    assert_eq!(status.status, 200, "{}", status.text);
    assert_eq!(status.body["session_open"], true);
    assert_eq!(status.body["status"]["latest_scan"]["id"], scan_id);
    let caps = status.body["status"]["capabilities"]["operations"]
        .as_array()
        .unwrap();
    // Planning is available (Setup's default plan builder is injected); analysis is not, because
    // this server has no model key.
    let cap = |name: &str| caps.iter().find(|c| c["operation"] == name).unwrap();
    assert_eq!(cap("planSetup")["available"], true);
    assert_eq!(cap("analyzeSetup")["available"], false);
    // The scan lists the excerpts the model could see; none is approved yet.
    assert_eq!(scan.body["evidence"]["required"], true);
    assert!(
        !scan.body["evidence"]["candidates"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let recovery = alice.client.get(&format!("{base}/recovery")).await;
    assert_eq!(recovery.status, 200, "{}", recovery.text);
    assert!(recovery.body["recovery"].is_null());

    // No plan was built: nothing is invented, and the reason is a refreshable precondition.
    let plan = alice.client.get(&format!("{base}/plan")).await;
    assert_eq!(
        (plan.status, plan.error_code()),
        (409, "plan_not_built"),
        "{}",
        plan.text
    );
    // Nothing downstream of the scan works without the steps in between.
    let analyze = alice
        .client
        .post(&format!("{base}/analyze"), json!({}))
        .await;
    assert_eq!(
        (analyze.status, analyze.error_code()),
        (501, "capability_unavailable"),
        "{}",
        analyze.text
    );
    let build = alice.client.post(&format!("{base}/plan"), json!({})).await;
    assert_eq!(
        (build.status, build.error_code()),
        (409, "analysis_not_confirmed"),
        "{}",
        build.text
    );

    // A second scan replaces the session and reports a new scan.
    let again = alice
        .client
        .post(&format!("{base}/scan"), json!({"mode": "existing"}))
        .await;
    assert_eq!(again.status, 200, "{}", again.text);
    assert_ne!(again.body["scan_id"].as_str().unwrap(), scan_id);

    // Request mistakes name the field.
    let bad_mode = alice
        .client
        .post(&format!("{base}/scan"), json!({"mode": "everything"}))
        .await;
    assert_eq!(
        (bad_mode.status, bad_mode.error_code()),
        (400, "invalid_request")
    );
    assert_eq!(bad_mode.body["error"]["affected_ids"]["field"], "mode");
    let unknown = alice
        .client
        .post(
            &format!("{base}/scan"),
            json!({"mode": "existing", "path": "/etc"}),
        )
        .await;
    assert_eq!(unknown.status, 400, "no path comes from the caller");
}

#[tokio::test]
async fn apply_is_refused_without_an_approved_plan_and_changes_nothing() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = uploaded_project(&h, &alice, "Unapproved").await;
    let base = format!("/api/v1/projects/{project}/setup");
    let scan = alice
        .client
        .post(&format!("{base}/scan"), json!({"mode": "existing"}))
        .await;
    assert_eq!(scan.status, 200, "{}", scan.text);
    let inventory_before = alice
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await
        .body["descriptor"]["source_snapshot"]["inventory_hash"]
        .clone();

    // A plan id nobody built, with and without the hash an approval would carry, and the id of
    // the scan, which is not a plan.
    for body in [
        json!({"plan_id": Uuid::new_v4()}),
        json!({"plan_id": Uuid::new_v4(), "plan_sha256": "0".repeat(64)}),
        json!({"plan_id": scan.body["scan_id"]}),
    ] {
        let resp = alice.client.post(&format!("{base}/apply"), body).await;
        assert!(
            (400..500).contains(&resp.status),
            "refused, not applied: {} {}",
            resp.status,
            resp.text
        );
        assert_ne!(resp.body["status"], "applied");
        assert!(resp.body["error"]["code"].is_string(), "{}", resp.text);
    }

    let after = alice
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;
    assert_eq!(
        after.body["descriptor"]["source_snapshot"]["inventory_hash"],
        inventory_before
    );
    assert!(
        after.body["descriptor"]["source_snapshot"]["dirty_files"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let admin = h.admin().await;
    let transactions: i64 = sqlx::query_scalar("SELECT count(*) FROM setup_transactions")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(transactions, 0, "no transaction was started");
    let approvals: i64 = sqlx::query_scalar("SELECT count(*) FROM setup_approvals")
        .fetch_one(&admin)
        .await
        .unwrap();
    assert_eq!(approvals, 0);
}

#[tokio::test]
async fn a_new_mod_scan_starts_from_an_empty_workspace_and_existing_needs_a_mod() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Brand new").await;
    let base = format!("/api/v1/projects/{project}/setup");

    let none = alice
        .client
        .post(&format!("{base}/scan"), json!({"mode": "existing"}))
        .await;
    assert_eq!((none.status, none.error_code()), (409, "no_source"));

    let created = alice
        .client
        .post(
            &format!("{base}/scan"),
            json!({"mode": "create", "description": "A small focus tree mod"}),
        )
        .await;
    assert_eq!(created.status, 200, "{}", created.text);
    assert!(h.workspace_source(&alice, &project).is_dir());
    let ws = alice
        .client
        .get(&format!("/api/v1/projects/{project}/workspace"))
        .await;
    assert_eq!(ws.body["summary"]["has_source"], false);
    assert_eq!(ws.body["summary"]["file_count"], 0);
    let too_long = alice
        .client
        .post(
            &format!("{base}/scan"),
            json!({"mode": "create", "description": "x".repeat(5000)}),
        )
        .await;
    assert_eq!(
        too_long.body["error"]["affected_ids"]["field"],
        "description"
    );
}

#[tokio::test]
async fn setup_routes_of_another_account_are_the_same_404_and_signed_out_is_401() {
    let h = Harness::start(Options::default()).await;
    let alice = h.verified_user("alice@example.org").await;
    let bob = h.verified_user("bob@example.org").await;
    let project = uploaded_project(&h, &alice, "Alice's").await;
    let alice_scan = alice
        .client
        .post(
            &format!("/api/v1/projects/{project}/setup/scan"),
            json!({"mode": "existing"}),
        )
        .await;
    assert_eq!(alice_scan.status, 200, "{}", alice_scan.text);
    let missing = Uuid::new_v4().to_string();
    let calls: Vec<(&str, &str, Value)> = vec![
        ("POST", "scan", json!({"mode": "existing"})),
        ("POST", "scan", json!({"mode": "create"})),
        ("GET", "plan", Value::Null),
        ("POST", "plan", json!({})),
        (
            "POST",
            "evidence",
            json!({"scan_id": Uuid::new_v4(), "references": []}),
        ),
        ("POST", "analyze", json!({})),
        (
            "POST",
            "confirm",
            json!({"analysis_id": Uuid::new_v4(), "values": {
                "description": "", "folder_profile": ["common"],
                "identity": {"display_name": "x", "project_id": "x", "author": "x", "version": "0.1.0",
                    "supported_game_version": "1.19.*", "script_prefix": "x", "primary_namespace": "x",
                    "default_branch": "main", "descriptor_tags": []},
                "conventions": {"agents_profile": "x", "localisation_convention": "x", "documentation_convention": "x"}
            }}),
        ),
        (
            "POST",
            "conflicts",
            json!({"plan_id": Uuid::new_v4(), "decisions": []}),
        ),
        (
            "POST",
            "approve",
            json!({"plan_id": Uuid::new_v4(), "plan_sha256": "0"}),
        ),
        ("POST", "apply", json!({"plan_id": Uuid::new_v4()})),
        ("GET", "status", Value::Null),
        ("GET", "recovery", Value::Null),
        (
            "POST",
            "recover",
            json!({"journal_id": Uuid::new_v4(), "variant": "manual_review"}),
        ),
    ];
    for (method, tail, body) in &calls {
        let foreign = setup_call(&bob.client, method, &project, tail, body).await;
        let absent = setup_call(&bob.client, method, &missing, tail, body).await;
        assert_eq!(
            (foreign.status, foreign.error_code()),
            (404, "not_found"),
            "{method} {tail}: {}",
            foreign.text
        );
        assert_eq!(
            foreign.body, absent.body,
            "{method} {tail}: a foreign project looks exactly like a missing one"
        );
        let anonymous_client = h.client();
        let anonymous = setup_call(&anonymous_client, method, &project, tail, body).await;
        assert_eq!(
            (anonymous.status, anonymous.error_code()),
            (401, "unauthenticated"),
            "{method} {tail}"
        );
    }
    // Alice is unaffected: her bytes, her session and her workspace.
    let status = alice
        .client
        .get(&format!("/api/v1/projects/{project}/setup/status"))
        .await;
    assert_eq!(status.body["session_open"], true);
    assert_eq!(
        std::fs::read(h.workspace_source(&alice, &project).join("descriptor.mod")).unwrap(),
        DESCRIPTOR_MOD
    );
    assert!(!h.data_dir.path().join("workspaces").join(&bob.id).exists());
    // A cross-site request is refused like every other state-changing route.
    let csrf = alice
        .client
        .send_raw(
            "POST",
            &format!("/api/v1/projects/{project}/setup/scan"),
            Some("application/json"),
            json!({"mode": "existing"}).to_string().into_bytes(),
            &[("origin", "https://evil.example")],
        )
        .await;
    assert_eq!(
        (csrf.status, csrf.error_code()),
        (403, "origin_not_allowed")
    );
}

#[tokio::test]
async fn the_setup_migrations_run_in_order_after_intake() {
    let h = Harness::start(Options::default()).await;
    let names: Vec<&str> = h
        .state
        .migrations
        .components
        .iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(names, ["accounts", "intake", "setup-adapter"]);
    let latest: Vec<i64> = h
        .state
        .migrations
        .components
        .iter()
        .map(|c| c.latest_version)
        .collect();
    assert!(latest.windows(2).all(|w| w[0] < w[1]), "{latest:?}");
    let ready = h.client().get("/readyz").await;
    assert_eq!(ready.status, 200, "{}", ready.text);
    assert_eq!(ready.body["checks"]["migrations"]["ok"], true);
}

// ----- the model client ----------------------------------------------------------------------

type SeenRequests = Arc<Mutex<Vec<(HeaderMap, Value)>>>;

async fn fake_messages(
    axum::extract::State(seen): axum::extract::State<SeenRequests>,
    headers: HeaderMap,
    axum::Json(body): axum::Json<Value>,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    let fail = headers.get("x-api-key").and_then(|v| v.to_str().ok()) == Some("sk-wrong");
    seen.lock().unwrap().push((headers, body));
    if fail {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}})),
        )
            .into_response();
    }
    let events = [
        (
            "message_start",
            json!({"type": "message_start", "message": {"id": "msg_test", "model": "claude-haiku-5-5", "usage": {"input_tokens": 11, "output_tokens": 0}}}),
        ),
        (
            "content_block_start",
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        ),
        (
            "content_block_delta",
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "{\"summary\":"}}),
        ),
        (
            "content_block_delta",
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": " \"ok\"}"}}),
        ),
        (
            "content_block_stop",
            json!({"type": "content_block_stop", "index": 0}),
        ),
        (
            "message_delta",
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 7}}),
        ),
        ("message_stop", json!({"type": "message_stop"})),
    ];
    let body: String = events
        .iter()
        .map(|(name, data)| format!("event: {name}\ndata: {data}\n\n"))
        .collect();
    ([(header::CONTENT_TYPE, "text/event-stream")], body).into_response()
}

#[tokio::test]
async fn the_model_client_sends_setups_contract_as_native_structured_output() {
    use axum::routing::post;
    use factory_server::config::AnthropicSettings;
    use factory_server::model::AnthropicModel;
    use factory_setup_adapter::{
        ANTHROPIC_UNSUPPORTED_KEYWORDS, ModelClient, ModelRequest, analysis_contract,
    };

    let seen: SeenRequests = Arc::default();
    let app = Router::new()
        .route("/v1/messages", post(fake_messages))
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let (system, schema) = analysis_contract("claude-haiku-5-5").expect("Setup's contract");
    let request = |model: &str| ModelRequest {
        provider: "claude".into(),
        model: model.into(),
        reasoning_effort: "high".into(),
        endpoint: "ignored".into(),
        prompt: "the evidence".into(),
        system_prompt: system.clone(),
        output_schema: schema.clone(),
        input_sha256: "0".repeat(64),
        credential: None,
    };
    let settings = |key: &str| AnthropicSettings {
        api_key: factory_accounts::Redacted(key.to_owned()),
        model: "claude-haiku-5-5".to_owned(),
        base_url: base.clone(),
    };

    let model = AnthropicModel::new(&settings("sk-test-key"));
    assert!(!format!("{model:?}").contains("sk-test-key"));
    let reply = model.complete(request("some-other-model")).await.unwrap();
    assert_eq!(reply.reply, json!({"summary": "ok"}));
    assert_eq!(reply.response_id.as_deref(), Some("msg_test"));
    assert_eq!(reply.usage.unwrap()["output_tokens"], 7);

    let (headers, body) = seen.lock().unwrap()[0].clone();
    assert_eq!(headers["x-api-key"], "sk-test-key");
    assert!(headers.contains_key("anthropic-version"));
    assert_eq!(
        body["model"], "claude-haiku-5-5",
        "the server picks the model"
    );
    assert_eq!(body["system"], system.as_str());
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "the evidence");
    assert_eq!(body["output_config"]["format"]["type"], "json_schema");
    let sent_schema = &body["output_config"]["format"]["schema"];
    let expected =
        factory_setup_adapter::strip_schema_keywords(&schema, ANTHROPIC_UNSUPPORTED_KEYWORDS);
    assert_eq!(sent_schema, &expected);
    let text = sent_schema.to_string();
    for keyword in [
        "\"minimum\":",
        "\"maximum\":",
        "\"maxLength\":",
        "\"minLength\":",
    ] {
        assert!(
            !text.contains(keyword),
            "{keyword} must be stripped from {text}"
        );
    }
    assert!(body.get("temperature").is_none());

    // A provider refusal is an error that does not repeat the key.
    let wrong = AnthropicModel::new(&settings("sk-wrong"));
    let err = wrong.complete(request("x")).await.unwrap_err();
    assert!(!err.to_string().contains("sk-wrong"), "{err}");
}

/// One call to a Setup route.
async fn setup_call(
    client: &Client,
    method: &str,
    project: &str,
    tail: &str,
    body: &Value,
) -> Resp {
    let path = format!("/api/v1/projects/{project}/setup/{tail}");
    if method == "POST" {
        client.post(&path, body.clone()).await
    } else {
        client.get(&path).await
    }
}
