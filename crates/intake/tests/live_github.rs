//! One live import against GitHub. Ignored by default.
//!
//! Run with
//! `HOI4_FACTORY_GITHUB_APP_JSON=<path to app.json> cargo test -p factory-intake --test live_github -- --ignored`
//!
//! The file holds the HOI4 Factory Dev App credentials (`id`, `pem`) and stays outside the
//! repository (`.work/github-app/app.json`, git-ignored). The test reads it at run time and
//! never copies, prints or stores the key. Everything it does is read-only: `GET
//! /app/installations`, an installation token limited to one repository, `GET` calls, and a
//! Git fetch of `klimPaskov/hoi4-factory-test-mod`. It pushes nothing and writes nothing to
//! GitHub.

use std::sync::Arc;

use factory_intake::{
    FileTypePolicy, GithubConfig, GithubEndpoints, GithubImporter, HttpGithubApi, ImportRequest,
    InstallationRef, IntakeLimits, RepositoryAllowlist, SecretString, StaticSecrets, app_jwt,
};
use factory_workspace::{AccountId, ProjectId, StorageRoot};

const OWNER: &str = "klimPaskov";
const REPO: &str = "klimPaskov/hoi4-factory-test-mod";

#[tokio::test]
#[ignore = "contacts github.com and needs HOI4_FACTORY_GITHUB_APP_JSON"]
async fn live_read_only_import_of_the_canonical_test_mod() {
    let path = std::env::var("HOI4_FACTORY_GITHUB_APP_JSON")
        .expect("set HOI4_FACTORY_GITHUB_APP_JSON to the path of the App credentials file");
    let app: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).expect("read the credentials file")).unwrap();
    let app_id = match &app["id"] {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let pem = app["pem"]
        .as_str()
        .expect("the credentials file has a pem field")
        .to_owned();

    // Find the installation of the test account with the App JWT. This is the only call the
    // library does not make itself.
    let jwt = app_jwt(&app_id, &SecretString::new(pem.clone()), now()).unwrap();
    let installations: serde_json::Value = reqwest::Client::new()
        .get("https://api.github.com/app/installations")
        .bearer_auth(jwt.expose())
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "hoi4-factory-intake-live-test")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let installation = installations
        .as_array()
        .and_then(|all| all.iter().find(|i| i["account"]["login"] == OWNER))
        .expect("the App is installed on the test account");
    assert_eq!(
        installation["repository_selection"], "selected",
        "the test installation must be limited to selected repositories"
    );
    let installation_id = installation["id"].as_u64().unwrap().to_string();

    let tmp = tempfile::tempdir().unwrap();
    let storage = StorageRoot::new(tmp.path().join("data"));
    let endpoints = GithubEndpoints::github_com();
    let secrets = StaticSecrets::new()
        .with("github_app.id", &app_id)
        .with("github_app.private_key", &pem)
        .with("installations/live", &installation_id);
    let importer = GithubImporter::new(
        Arc::new(HttpGithubApi::new(&endpoints).unwrap()),
        Arc::new(secrets),
        storage.clone(),
        GithubConfig::new(endpoints),
        IntakeLimits::default(),
        FileTypePolicy::default(),
    );
    let installation_ref = InstallationRef::new("installations/live").unwrap();
    let allowlist = RepositoryAllowlist::new([REPO]).unwrap();

    let visible = importer
        .list_repositories(&installation_ref, &allowlist)
        .await
        .unwrap();
    // The installation may cover more repositories than the test mod. The project allowlist is
    // what limits the import (S01 finding), so only the test mod is flagged as allowed.
    let allowed: Vec<&str> = visible
        .iter()
        .filter(|v| v.allowed)
        .map(|v| v.repository.full_name.as_str())
        .collect();
    assert_eq!(
        allowed,
        vec![REPO],
        "only the allowlisted repository is importable"
    );

    let account = AccountId::new("live-acct").unwrap();
    let project = ProjectId::new("live-proj").unwrap();
    let outcome = importer
        .import(&ImportRequest {
            account: &account,
            project: &project,
            installation: &installation_ref,
            repository: REPO,
            branch: None,
            allowlist: &allowlist,
        })
        .await
        .unwrap();

    assert_eq!(
        outcome.dirty_paths, 0,
        "the working tree equals the remote ref"
    );
    let main = outcome
        .observed_refs
        .iter()
        .find(|r| r.name == format!("refs/heads/{}", outcome.branch))
        .expect("the imported branch is among the observed refs");
    assert_eq!(main.sha, outcome.head.to_string());
    assert_eq!(
        outcome.remote_url,
        "https://github.com/klimPaskov/hoi4-factory-test-mod.git"
    );
    let source = tmp
        .path()
        .join("data/workspaces/live-acct/live-proj/source");
    assert!(source.join(".git").is_dir());
    let config = std::fs::read_to_string(source.join(".git/config")).unwrap();
    assert!(!config.contains("extraheader") && !config.to_lowercase().contains("authorization"));
    println!(
        "live import ok: branch {} at {}, {} files, {} refs",
        outcome.branch,
        outcome.head,
        outcome.inventory.entries,
        outcome.observed_refs.len()
    );
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
