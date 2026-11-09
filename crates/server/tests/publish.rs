//! S04-W5 publication through C-10 against the fake GitHub (a real `git http-backend` over bare
//! repositories): a reviewed commit on the task branch, a separate publication grant, a push of
//! the owned branch and a pull request into the default branch. A lost push reply and a lost
//! pull request reply are reconciled by reading state, with no duplicate push or pull request.
//! The default branch is never written.

#![allow(dead_code, unused_imports)]

#[path = "../../intake/tests/common/mod.rs"]
mod common;

include!("support/harness.rs");

fn git_out(dir: &std::path::Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git").args(["-c", "core.autocrlf=false"]).args(args).current_dir(dir).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

#[tokio::test]
async fn an_owned_branch_is_pushed_and_a_pull_request_opened_once_even_after_lost_replies() {
    let gh = github_fixture().await;
    let bare = gh.remote.bare.clone();
    let main_before = gh.remote.main_tip.clone();
    let h = Harness::start(Options {
        github: Some(gh),
        anthropic_key: Some("sk-ant-not-used"),
        agent: AgentOptions { worker: None, settings: None, tools: Some(None), skip_recovery: true },
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "From GitHub").await;
    h.link_github(&alice, 424_242).await;
    *h.installations.list.lock().unwrap() = vec![installation(424_242, "selected")];
    let imported = alice.client.post(&format!("/api/v1/projects/{project}/github/import"), json!({"repository": REPO})).await;
    assert!(imported.status == 200 || imported.status == 201, "{}", imported.text);

    let t = alice
        .client
        .post(&format!("/api/v1/projects/{project}/tasks"), json!({ "objective": "Publish a change", "mode": "edit", "allowed_paths": ["notes.txt", "common"] }))
        .await;
    assert_eq!(t.status, 201, "{}", t.text);
    let task_id = t.body["id"].as_str().unwrap().to_owned();
    let base = format!("/api/v1/projects/{project}/tasks/{task_id}");

    // The person's change, as a file proposal, reviewed and committed.
    let rt = h.state.agent.clone().unwrap();
    let account = factory_accounts::AccountId(alice.id.parse().unwrap());
    let mut tx = rt.tx(account).await.unwrap();
    let task = factory_tasks::get_task(&mut tx, factory_tasks::TaskId(task_id.parse().unwrap())).await.unwrap();
    tx.commit().await.unwrap();
    let keys = factory_server::focus::store::TaskKeys::of(&task, account);
    // Opening the proposals list creates the worktree through the same path as every route.
    let _ = alice.client.post(&format!("{base}/check"), json!({})).await;
    let wt = h.data_dir.path().join("workspaces").join(&alice.id).join(&project).join("tasks").join(&task_id);
    std::fs::write(wt.join("notes.txt"), "published from the factory\n").unwrap();
    let proposal = factory_server::focus::apply::record_file_proposal(
        &rt,
        &keys,
        &wt,
        factory_server::focus::store::ActorKind::Person(account),
        &["notes.txt".to_owned()],
    )
    .await
    .unwrap()
    .unwrap();
    let c = alice
        .client
        .post(
            &format!("{base}/commit"),
            json!({ "proposals": [{"id": proposal["id"], "proposal_hash": proposal["proposalHash"]}], "message": "Update the notes", "idempotency_key": "c-1", "accept_incomplete": true }),
        )
        .await;
    assert_eq!(c.status, 201, "{}", c.text);
    let commit = c.body["result"]["commit"].as_str().unwrap().to_owned();
    let branch = format!("hoi4factory/{task_id}");

    // Publication needs its own grant: the editing grant or none does not do.
    let no_grant = alice
        .client
        .post(&format!("{base}/push"), json!({ "grant_id": uuid::Uuid::new_v4(), "epoch": 1, "idempotency_key": "p-0" }))
        .await;
    assert_eq!(no_grant.status, 403, "{}", no_grant.text);
    let g = alice
        .client
        .post(&format!("{base}/publication-grants"), json!({ "operation_classes": ["push_owned_branch", "open_pull_request"] }))
        .await;
    assert_eq!(g.status, 201, "{}", g.text);
    assert_eq!(g.body["branch"], json!(branch));
    let wide = alice.client.post(&format!("{base}/publication-grants"), json!({ "operation_classes": ["integrate_default_branch"] })).await;
    assert_eq!(wide.status, 400, "default-branch integration is never granted here: {}", wide.text);
    let grant = g.body["grant_id"].clone();
    let epoch = g.body["epoch"].clone();

    // A push whose reply is lost: GitHub fails every Git request after the push itself, so the
    // read-back fails and the host cannot tell what happened.
    {
        let fake = &h.github.as_ref().unwrap().fake;
        let served = fake.with(|s| s.git_calls.len());
        fake.with(|s| s.git_error_after = Some(served + 2));
    }
    let lost = alice.client.post(&format!("{base}/push"), json!({ "grant_id": grant, "epoch": epoch, "idempotency_key": "p-1" })).await;
    assert!(lost.status >= 500 || lost.status == 403, "{}", lost.text);
    assert_eq!(git_out(&bare, &["rev-parse", &format!("refs/heads/{branch}")]), commit, "the push itself landed");
    h.github.as_ref().unwrap().fake.with(|s| s.git_error_after = None);
    let pushes_before = h.github.as_ref().unwrap().fake.with(|s| s.git_calls.iter().filter(|c| c.path.ends_with("git-receive-pack") && c.query.is_empty()).count());
    // The same request again reconciles by reading the remote branch. It does not push again.
    let again = alice.client.post(&format!("{base}/push"), json!({ "grant_id": grant, "epoch": epoch, "idempotency_key": "p-1" })).await;
    assert_eq!(again.status, 200, "{}", again.text);
    assert_eq!(again.body["result"]["remote_commit"], json!(commit));
    assert_eq!(again.body["result"]["reconciled"], true);
    let pushes_after = h.github.as_ref().unwrap().fake.with(|s| s.git_calls.iter().filter(|c| c.path.ends_with("git-receive-pack") && c.query.is_empty()).count());
    assert_eq!(pushes_after, pushes_before, "no second push");

    // A pull request whose reply is lost: created on GitHub, answered 502.
    h.github.as_ref().unwrap().fake.with(|s| s.lose_next_pull_reply = true);
    let pr_body = json!({ "grant_id": grant, "epoch": epoch, "idempotency_key": "pr-1", "title": "Update the notes", "body": "Made in HOI4 Factory." });
    let lost_pr = alice.client.post(&format!("{base}/pull-request"), pr_body.clone()).await;
    assert!(lost_pr.status >= 500, "{}", lost_pr.text);
    let again_pr = alice.client.post(&format!("{base}/pull-request"), pr_body.clone()).await;
    assert_eq!(again_pr.status, 200, "{}", again_pr.text);
    assert_eq!(again_pr.body["result"]["pull_request"]["number"], 1);
    assert_eq!(again_pr.body["result"]["pull_request"]["base"], "main");
    assert_eq!(again_pr.body["result"]["pull_request"]["head"], json!(branch));
    // A new request for the same branch finds the open pull request instead of opening another.
    let other_key = alice.client.post(&format!("{base}/pull-request"), json!({ "grant_id": grant, "epoch": epoch, "idempotency_key": "pr-2", "title": "Again" })).await;
    assert_eq!(other_key.status, 200, "{}", other_key.text);
    assert_eq!(h.github.as_ref().unwrap().fake.with(|s| s.pull_creates), 1, "exactly one pull request");

    // The default branch never moved.
    assert_eq!(git_out(&bare, &["rev-parse", "refs/heads/main"]), main_before);
    let pubs = alice.client.get(&format!("{base}/publications")).await;
    let kinds: Vec<&str> = pubs.body["publications"].as_array().unwrap().iter().map(|p| p["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds.iter().filter(|k| **k == "push").count(), 1);

    // Another account cannot publish this task.
    let mallory = h.verified_user("mallory@example.org").await;
    let refused = mallory.client.post(&format!("{base}/publication-grants"), json!({ "operation_classes": ["push_owned_branch"] })).await;
    assert_eq!(refused.status, 404, "{}", refused.text);
}

/// Gate 6: the mod leaves the factory (a ZIP of the task branch, and the branch on GitHub), is
/// changed outside it (a commit pushed to the task branch from a plain Git clone), and comes back
/// with the outside change detected and taken in.
#[tokio::test]
async fn work_done_outside_the_factory_is_detected_when_the_branch_comes_back() {
    let gh = github_fixture().await;
    let git_root = gh.remote.root.clone();
    let bare = gh.remote.bare.clone();
    let h = Harness::start(Options {
        github: Some(gh),
        anthropic_key: Some("sk-ant-not-used"),
        agent: AgentOptions { worker: None, settings: None, tools: Some(None), skip_recovery: true },
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Round trip").await;
    h.link_github(&alice, 424_242).await;
    *h.installations.list.lock().unwrap() = vec![installation(424_242, "selected")];
    let imported = alice.client.post(&format!("/api/v1/projects/{project}/github/import"), json!({"repository": REPO})).await;
    assert!(imported.status == 200 || imported.status == 201, "{}", imported.text);
    let t = alice
        .client
        .post(&format!("/api/v1/projects/{project}/tasks"), json!({ "objective": "Round trip", "mode": "edit", "allowed_paths": ["notes.txt"] }))
        .await;
    let task_id = t.body["id"].as_str().unwrap().to_owned();
    let base = format!("/api/v1/projects/{project}/tasks/{task_id}");
    let branch = format!("hoi4factory/{task_id}");

    // Download: a ZIP of the committed files of the task branch.
    let zip = alice.client.get(&format!("{base}/download")).await;
    assert_eq!(zip.status, 200);
    assert!(zip.text.starts_with("PK"), "a ZIP archive");
    assert!(zip.text.contains("notes.txt"), "it holds the mod files");

    // Publish the branch, so a modder can clone it.
    let g = alice.client.post(&format!("{base}/publication-grants"), json!({ "operation_classes": ["push_owned_branch"] })).await;
    assert_eq!(g.status, 201, "{}", g.text);
    let pushed = alice
        .client
        .post(&format!("{base}/push"), json!({ "grant_id": g.body["grant_id"], "epoch": g.body["epoch"], "idempotency_key": "p-1" }))
        .await;
    assert_eq!(pushed.status, 201, "{}", pushed.text);
    let factory_tip = pushed.body["result"]["remote_commit"].as_str().unwrap().to_owned();

    // Nothing new yet.
    let same = alice.client.post(&format!("{base}/sync"), json!({})).await;
    assert_eq!(same.status, 200, "{}", same.text);
    assert_eq!(same.body["external_changes"], false);

    // A modder clones the branch with plain Git, edits outside the factory and pushes back.
    let outside = git_root.join("_outside");
    let clone = |args: &[&str]| {
        let out = std::process::Command::new("git").args(["-c", "core.autocrlf=false"]).args(args).current_dir(&git_root).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    };
    clone(&["clone", "-q", "--branch", &branch, bare.to_str().unwrap(), outside.to_str().unwrap()]);
    std::fs::write(outside.join("notes.txt"), "edited outside the factory\n").unwrap();
    let in_clone = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(["-c", "core.autocrlf=false", "-c", "user.name=Modder", "-c", "user.email=modder@example.org"])
            .args(args)
            .current_dir(&outside)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    in_clone(&["commit", "-q", "-am", "Edit outside"]);
    in_clone(&["push", "-q", "origin", &branch]);
    let outside_tip = in_clone(&["rev-parse", "HEAD"]);
    assert_ne!(outside_tip, factory_tip);

    // Back in the factory: the outside change is detected and taken in.
    let synced = alice.client.post(&format!("{base}/sync"), json!({})).await;
    assert_eq!(synced.status, 200, "{}", synced.text);
    assert_eq!(synced.body["external_changes"], true);
    assert_eq!(synced.body["to"], json!(outside_tip));
    assert_eq!(synced.body["changed_paths"], json!(["notes.txt"]));
    let wt = h.data_dir.path().join("workspaces").join(&alice.id).join(&project).join("tasks").join(&task_id);
    assert_eq!(std::fs::read_to_string(wt.join("notes.txt")).unwrap(), "edited outside the factory\n");
    let events = alice.client.get(&format!("{base}/events?after=0&limit=200")).await;
    assert!(events.body["events"].as_array().unwrap().iter().any(|e| e["type"] == "external_changes"));
}

/// Gate 8 (P4): every import gets a preservation report (one row per file: parsed, kept
/// byte-exact or not understood) and a recorded engine baseline for its commit.
#[tokio::test]
async fn an_upload_gets_a_preservation_report_and_a_baseline() {
    let h = Harness::start(Options {
        anthropic_key: Some("sk-ant-not-used"),
        agent: AgentOptions { worker: None, settings: None, tools: Some(None), skip_recovery: true },
        ..Options::default()
    })
    .await;
    let alice = h.verified_user("alice@example.org").await;
    let project = h.project(&alice, "Report").await;
    let entries = vec![
        ZEntry::file("descriptor.mod", b"name=\"Report\"\nsupported_version=\"1.19.*\"\n".to_vec()),
        ZEntry::file("common/national_focus/ok.txt", b"focus_tree = {\n\tid = ok_tree\n\tfocus = {\n\t\tid = OK_one\n\t\tx = 1\n\t\ty = 0\n\t\tcost = 10\n\t}\n}\n".to_vec()),
        ZEntry::file("common/national_focus/broken.txt", b"focus_tree = {\n\tid = broken\n\tfocus = {\n".to_vec()),
        ZEntry::file("gfx/art.dds", vec![0u8, 159, 146, 150, 1, 2, 3]),
    ];
    let up = alice
        .client
        .upload(&project, &[Part::text("kind", "zip"), Part::text("git_init_consent", "true"), Part::file("r.zip", build_zip(&entries))])
        .await;
    assert_eq!(up.status, 201, "{}", up.text);
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    let report = loop {
        let r = alice.client.get(&format!("/api/v1/projects/{project}/import-report")).await;
        assert_eq!(r.status, 200, "{}", r.text);
        if r.body["state"] == "recorded" {
            break r.body["report"].clone();
        }
        assert!(std::time::Instant::now() < deadline, "no report: {}", r.text);
        tokio::time::sleep(Duration::from_millis(250)).await;
    };
    assert_eq!(report["engine_rev"], factory_engine_check::ENGINE_REV);
    let rows = report["preservation"]["rows"].as_array().unwrap();
    let status = |p: &str| rows.iter().find(|r| r["path"] == p).map(|r| r["status"].clone()).unwrap_or(Value::Null);
    assert_eq!(status("common/national_focus/ok.txt"), "parsed", "{report:#}");
    assert_eq!(status("common/national_focus/broken.txt"), "not_understood");
    assert_eq!(status("gfx/art.dds"), "kept_byte_exact");
    assert_ne!(report["verdict"], "pass", "no game reference: never a pass");
}
