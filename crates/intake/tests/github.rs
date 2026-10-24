//! GitHub import against a local fake GitHub that serves real bare repositories through the
//! real `git http-backend`.
//!
//! Substitution, stated plainly: the REST side is a fake that checks what GitHub checks (RS256
//! App JWT, repository-scoped installation tokens, `Authorization: basic` on every Git
//! request). The Git side is not faked: `git ls-remote`, `fetch` and `checkout` run against a
//! genuine smart-HTTP server, so the extra-header credential path is exercised end to end.
//! Only the host differs from production (`http://127.0.0.1:<port>` instead of
//! `https://github.com`).

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use common::fake_github::*;
use common::*;
use factory_intake::{
    FileTypePolicy, Finding, GithubConfig, GithubEndpoints, GithubImporter, HttpGithubApi,
    ImportRequest, InstallationRef, IntakeError, IntakeLimits, ObservedRef, RepositoryAllowlist,
    RetryClass, SecretString, StaticSecrets, app_jwt,
};
use factory_workspace::{AccountId, ProjectId, StorageRoot};

const REPO: &str = "klimPaskov/hoi4-factory-test-mod";

struct Gh {
    _tmp: tempfile::TempDir,
    storage: StorageRoot,
    fake: FakeGithub,
    importer: GithubImporter,
    remote: RemoteFixture,
    account: AccountId,
    project: ProjectId,
    installation: InstallationRef,
    allowlist: RepositoryAllowlist,
}

async fn setup_with(secrets: StaticSecrets) -> Gh {
    let tmp = tempfile::tempdir().unwrap();
    let git_root = tmp.path().join("remote");
    fs::create_dir_all(&git_root).unwrap();
    let remote = make_remote(&git_root, "klimPaskov", "hoi4-factory-test-mod");

    let fake = FakeGithub::start(
        &git_root,
        vec![
            FakeRepo::new(REPO, 1001),
            FakeRepo::new("klimPaskov/other-repo", 1002),
        ],
    )
    .await;
    let storage = StorageRoot::new(tmp.path().join("data"));
    let endpoints = GithubEndpoints::custom(&fake.api_base(), &fake.clone_base()).unwrap();
    let api = Arc::new(HttpGithubApi::new(&endpoints).unwrap());
    let importer = GithubImporter::new(
        api,
        Arc::new(secrets),
        storage.clone(),
        GithubConfig::new(endpoints),
        IntakeLimits::default(),
        FileTypePolicy::default(),
    );
    Gh {
        _tmp: tmp,
        storage,
        fake,
        importer,
        remote,
        account: AccountId::new("acct-0001").unwrap(),
        project: ProjectId::new("proj-0001").unwrap(),
        installation: InstallationRef::new("installations/test").unwrap(),
        // Entered in the wrong case on purpose: matching ignores case.
        allowlist: RepositoryAllowlist::new(["KLIMPASKOV/HOI4-FACTORY-TEST-MOD"]).unwrap(),
    }
}

fn good_secrets() -> StaticSecrets {
    StaticSecrets::new()
        .with("github_app.id", APP_ID)
        .with("github_app.private_key", &test_key().private_pem)
        .with("installations/test", &INSTALLATION_ID.to_string())
}

async fn setup() -> Gh {
    setup_with(good_secrets()).await
}

impl Gh {
    fn req<'a>(&'a self, repository: &'a str, branch: Option<&'a str>) -> ImportRequest<'a> {
        ImportRequest {
            account: &self.account,
            project: &self.project,
            installation: &self.installation,
            repository,
            branch,
            allowlist: &self.allowlist,
        }
    }

    fn project_dir(&self) -> PathBuf {
        self.storage
            .data_root()
            .join("workspaces")
            .join(self.account.as_str())
            .join(self.project.as_str())
    }

    fn source(&self) -> PathBuf {
        self.project_dir().join("source")
    }

    async fn import_err(&self, repository: &str, branch: Option<&str>) -> IntakeError {
        self.importer
            .import(&self.req(repository, branch))
            .await
            .expect_err("the import should have been refused")
    }
}

/// Every file under `root` must be free of the given secrets.
fn assert_secrets_absent(root: &Path, secrets: &[String]) {
    fn walk(dir: &Path, secrets: &[String]) {
        let Ok(read) = fs::read_dir(dir) else { return };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, secrets);
            } else if let Ok(bytes) = fs::read(&path) {
                for secret in secrets {
                    assert!(
                        !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
                        "a credential was written to {}",
                        path.display()
                    );
                }
            }
        }
    }
    walk(root, secrets);
}

// ---------------------------------------------------------------------------------------------

#[tokio::test]
async fn import_clones_the_branch_byte_for_byte_with_a_scoped_token() {
    let gh = setup().await;
    let outcome = gh.importer.import(&gh.req(REPO, None)).await.unwrap();

    // The working tree is the remote ref byte for byte, whatever .gitattributes asks for.
    assert_eq!(outcome.head.to_string(), gh.remote.main_tip);
    assert_eq!(outcome.branch, "main");
    assert_eq!(outcome.dirty_paths, 0);
    let source = gh.source();
    assert_eq!(
        fs::read(source.join("notes.txt")).unwrap(),
        NOTES_TXT,
        "eol=crlf must not rewrite the tree"
    );
    assert_eq!(
        fs::read(source.join("data/legacy.csv")).unwrap(),
        LEGACY_CSV
    );
    assert_eq!(fs::read(source.join("interface/bom.gui")).unwrap(), BOM_GUI);
    assert_eq!(fs::read(source.join("gfx/art.dds")).unwrap(), binary_blob());
    assert_eq!(git_text(&source, &["status", "--porcelain"]), "");
    assert_eq!(
        git_text(&source, &["rev-parse", "HEAD"]),
        gh.remote.main_tip
    );
    assert_eq!(
        git_text(&source, &["rev-list", "--count", "HEAD"]),
        "2",
        "history is kept"
    );
    assert_eq!(
        git_text(&source, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "main"
    );
    assert_eq!(outcome.inventory.entries, 6);

    // Remote facts for the C-10 link.
    assert_eq!(outcome.repository.full_name, REPO);
    assert_eq!(outcome.repository.node_id, "R_node1001");
    assert_eq!(outcome.repository.default_branch, "main");
    assert_eq!(
        outcome.remote_url,
        "https://github.com/klimPaskov/hoi4-factory-test-mod.git"
    );
    assert_eq!(
        git_text(
            &source,
            &["config", "--local", "--get", "remote.origin.url"]
        ),
        outcome.remote_url
    );
    assert_eq!(
        git_text(
            &source,
            &["config", "--local", "--get", "hoi4factory.defaultbranch"]
        ),
        "main"
    );
    let refs: Vec<(String, String)> = outcome
        .observed_refs
        .iter()
        .map(|r| (r.name.clone(), r.sha.clone()))
        .collect();
    for (name, sha) in [
        ("refs/heads/main", &gh.remote.main_tip),
        ("refs/heads/feature", &gh.remote.feature_tip),
        ("refs/tags/v1", &gh.remote.tag_object),
    ] {
        assert!(
            refs.contains(&(name.to_owned(), sha.clone())),
            "missing {name} in {refs:?}"
        );
    }
    assert!(
        outcome
            .findings
            .contains(&Finding::GitattributesConversion {
                path: ".gitattributes".into(),
                line: 1,
                attribute: "eol=crlf".into(),
            })
    );

    // The token was minted for exactly this repository, by a valid RS256 App JWT.
    gh.fake.with(|s| {
        assert_eq!(
            s.token_requests,
            vec![serde_json::json!({"repositories": ["hoi4-factory-test-mod"]})]
        );
        assert_eq!(s.minted, 1);
        assert!(
            s.jwt_ok >= 2 && s.jwt_bad == 0,
            "jwt ok {} bad {}",
            s.jwt_ok,
            s.jwt_bad
        );
        // Every Git request carried the extra header for that one token, and succeeded.
        let token = s.tokens.keys().next().unwrap().clone();
        let expected = format!(
            "basic {}",
            STANDARD.encode(format!("x-access-token:{token}"))
        );
        assert!(!s.git_calls.is_empty());
        for call in &s.git_calls {
            assert_eq!(
                call.authorization.as_deref(),
                Some(expected.as_str()),
                "{}",
                call.path
            );
            assert_eq!(call.status, 200, "{}", call.path);
            // Import never pushes.
            assert!(!call.path.contains("receive-pack") && !call.query.contains("receive-pack"));
        }
    });

    // The credential exists nowhere on disk: not in Git config, packed refs, reflogs.
    let (tokens, header_values): (Vec<String>, Vec<String>) = gh.fake.with(|s| {
        let tokens: Vec<String> = s.tokens.keys().cloned().collect();
        let headers = tokens
            .iter()
            .map(|t| STANDARD.encode(format!("x-access-token:{t}")))
            .collect();
        (tokens, headers)
    });
    let mut secrets = tokens;
    secrets.extend(header_values);
    secrets.push(test_key().private_pem.lines().nth(1).unwrap().to_owned());
    assert_secrets_absent(gh.storage.data_root(), &secrets);
    let config = fs::read_to_string(source.join(".git/config")).unwrap();
    assert!(!config.contains("extraheader") && !config.to_lowercase().contains("authorization"));
}

#[tokio::test]
async fn a_non_default_branch_can_be_imported() {
    let gh = setup().await;
    let outcome = gh
        .importer
        .import(&gh.req(REPO, Some("feature")))
        .await
        .unwrap();
    assert_eq!(outcome.head.to_string(), gh.remote.feature_tip);
    assert_eq!(outcome.branch, "feature");
    assert_eq!(outcome.repository.default_branch, "main");
    let source = gh.source();
    assert_eq!(
        git_text(&source, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "feature"
    );
    assert!(source.join("feature.txt").exists());
    assert_eq!(
        git_text(
            &source,
            &["config", "--local", "--get", "hoi4factory.defaultbranch"]
        ),
        "main"
    );
}

#[tokio::test]
async fn an_all_repositories_installation_is_refused_before_a_token_exists() {
    let gh = setup().await;
    gh.fake.with(|s| s.selection = "all".into());
    let err = gh.import_err(REPO, None).await;
    assert_eq!(err.code(), "installation_not_selected");
    assert_eq!(err.retry_class(), RetryClass::RequiresAuthorization);
    gh.fake.with(|s| {
        assert!(
            s.token_requests.is_empty(),
            "no token may be minted for an all-repositories installation"
        );
        assert!(s.git_calls.is_empty());
    });
    assert!(!gh.project_dir().exists());
    let err = gh
        .importer
        .list_repositories(&gh.installation, &gh.allowlist)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "installation_not_selected");
}

#[tokio::test]
async fn a_repository_off_the_project_allowlist_is_refused_without_any_network_call() {
    let gh = setup().await;
    let err = gh.import_err("klimPaskov/other-repo", None).await;
    assert_eq!(err.code(), "repository_not_allowed");
    gh.fake.with(|s| {
        assert_eq!(
            s.api_calls, 0,
            "the allowlist is checked before GitHub is contacted"
        );
        assert!(s.git_calls.is_empty());
    });

    let empty = RepositoryAllowlist::default();
    let request = ImportRequest {
        allowlist: &empty,
        ..gh.req(REPO, None)
    };
    assert_eq!(
        gh.importer.import(&request).await.unwrap_err().code(),
        "repository_not_allowed"
    );
    gh.fake.with(|s| assert_eq!(s.api_calls, 0));
    assert!(!gh.project_dir().exists());
}

#[tokio::test]
async fn a_repository_the_installation_does_not_cover_is_refused() {
    let mut gh = setup().await;
    gh.allowlist = RepositoryAllowlist::new(["klimPaskov/outside-repo"]).unwrap();
    let err = gh.import_err("klimPaskov/outside-repo", None).await;
    assert_eq!(err.code(), "repository_not_in_installation");
    assert!(!gh.project_dir().exists());
}

#[tokio::test]
async fn bad_names_are_refused_before_any_network_call() {
    let gh = setup().await;
    for repo in [
        "not-a-full-name",
        "a/b/c",
        "../x",
        "owner/..",
        "owner name/repo",
        "/repo",
    ] {
        let err = gh.import_err(repo, None).await;
        assert_eq!(err.code(), "invalid_request", "{repo}");
    }
    for branch in ["../x", "a..b", "-x", "a b", "x.lock", ""] {
        let err = gh.import_err(REPO, Some(branch)).await;
        assert_eq!(err.code(), "branch_name_unsupported", "{branch:?}");
    }
    gh.fake.with(|s| assert_eq!(s.api_calls, 0));
}

#[tokio::test]
async fn revocation_is_reported_for_401_and_404_at_every_step() {
    // Installation gone.
    let gh = setup().await;
    gh.fake.with(|s| s.revoked = true);
    let err = gh.import_err(REPO, None).await;
    assert!(matches!(err, IntakeError::AccessRevoked), "{err:?}");
    assert_eq!(err.code(), "access_revoked");
    assert!(!gh.project_dir().exists());

    // Explicit statuses on the installation endpoint, and on the repository endpoint.
    for status in [401u16, 404] {
        let gh = setup().await;
        gh.fake
            .with(|s| s.installation_status_override = Some(status));
        assert_eq!(
            gh.import_err(REPO, None).await.code(),
            "access_revoked",
            "installation {status}"
        );
        let gh = setup().await;
        gh.fake.with(|s| s.repo_status_override = Some(status));
        assert_eq!(
            gh.import_err(REPO, None).await.code(),
            "access_revoked",
            "repository {status}"
        );
        assert!(!gh.project_dir().exists());
    }
}

#[tokio::test]
async fn a_revoked_installation_leaves_an_existing_workspace_untouched() {
    let gh = setup().await;
    let workspace = gh
        .storage
        .create_workspace(&gh.account, &gh.project)
        .unwrap();
    write_file(&workspace.source_dir(), "keep.txt", b"user work\r\n");
    gh.fake.with(|s| s.revoked = true);

    let err = gh.import_err(REPO, None).await;
    assert_eq!(err.code(), "access_revoked");
    assert_eq!(
        fs::read(gh.source().join("keep.txt")).unwrap(),
        b"user work\r\n"
    );
}

#[tokio::test]
async fn a_credential_git_rejects_is_revocation_and_the_partial_workspace_is_discarded() {
    let gh = setup().await;
    // ls-remote (two requests) succeeds; the fetch into the new workspace is refused.
    gh.fake.with(|s| s.git_reject_after = Some(2));
    let err = gh.import_err(REPO, None).await;
    assert_eq!(err.code(), "access_revoked", "{err:?}");
    assert!(!gh.project_dir().exists(), "a partial clone is discarded");
    let shown = format!("{err} {err:?}");
    let tokens: Vec<String> = gh.fake.with(|s| s.tokens.keys().cloned().collect());
    for token in tokens {
        assert!(!shown.contains(&token), "the token leaked into an error");
    }
}

#[tokio::test]
async fn suspended_missing_branch_and_oversized_repositories_are_refused() {
    let gh = setup().await;
    gh.fake.with(|s| s.suspended = true);
    assert_eq!(
        gh.import_err(REPO, None).await.code(),
        "installation_suspended"
    );

    let gh = setup().await;
    let err = gh.import_err(REPO, Some("no-such-branch")).await;
    assert_eq!(err.code(), "branch_not_found");
    assert!(!gh.project_dir().exists());

    let gh = setup().await;
    gh.fake.with(|s| s.repos[0].size_kib = 5_000_000);
    let err = gh.import_err(REPO, None).await;
    assert_eq!(err.code(), "repository_too_large");
    assert!(!gh.project_dir().exists());
}

#[tokio::test]
async fn rate_limits_and_outages_are_distinct_from_revocation() {
    let gh = setup().await;
    gh.fake.with(|s| s.rate_limit_installation = true);
    let err = gh.import_err(REPO, None).await;
    match &err {
        IntakeError::GithubRateLimited { retry_after } => {
            assert_eq!(*retry_after, Some(Duration::from_secs(7)))
        }
        other => panic!("expected a rate limit, got {other:?}"),
    }
    assert_eq!(err.retry_class(), RetryClass::SafeSameInputs);

    let gh = setup().await;
    gh.fake.with(|s| s.installation_status_override = Some(502));
    let err = gh.import_err(REPO, None).await;
    assert_eq!(err.code(), "github_unavailable");
}

#[tokio::test]
async fn listing_shows_the_visible_repositories_with_the_allowlist_flag() {
    let gh = setup().await;
    let listed = gh
        .importer
        .list_repositories(&gh.installation, &gh.allowlist)
        .await
        .unwrap();
    let summary: Vec<(String, bool)> = listed
        .iter()
        .map(|v| (v.repository.full_name.clone(), v.allowed))
        .collect();
    assert_eq!(
        summary,
        vec![
            (REPO.to_owned(), true),
            ("klimPaskov/other-repo".to_owned(), false)
        ]
    );
    // Listing needs a token for the whole installation, never a Git operation.
    gh.fake.with(|s| {
        assert_eq!(s.token_requests, vec![serde_json::json!({})]);
        assert!(s.git_calls.is_empty());
    });
    assert!(!gh.project_dir().exists());
}

#[tokio::test]
async fn missing_or_broken_credentials_are_reported_without_leaking_them() {
    let missing = StaticSecrets::new().with("installations/test", &INSTALLATION_ID.to_string());
    let gh = setup_with(missing).await;
    assert_eq!(
        gh.import_err(REPO, None).await.code(),
        "github_credentials_unavailable"
    );

    let broken = good_secrets().with(
        "github_app.private_key",
        &format!(
            "-----BEGIN {0}-----\nQUJD\n-----END {0}-----\n",
            "PRIVATE KEY"
        ),
    );
    let gh = setup_with(broken).await;
    let err = gh.import_err(REPO, None).await;
    assert_eq!(err.code(), "github_credentials_unavailable");
    assert!(!err.to_string().contains("QUJD"));

    let not_an_id = good_secrets().with("installations/test", "not-a-number");
    let gh = setup_with(not_an_id).await;
    assert_eq!(
        gh.import_err(REPO, None).await.code(),
        "github_credentials_unavailable"
    );
    gh.fake.with(|s| assert_eq!(s.api_calls, 0));
}

#[tokio::test]
async fn prepared_imports_do_not_print_the_token() {
    let gh = setup().await;
    let prepared = gh
        .importer
        .prepare_import(&gh.req(REPO, None))
        .await
        .unwrap();
    let shown = format!("{prepared:?}");
    assert!(!shown.contains("ghs_"), "{shown}");
    assert!(shown.contains("hoi4-factory-test-mod"));
    assert_eq!(prepared.installation_id, INSTALLATION_ID);
    assert_eq!(prepared.branch, "main");
}

#[test]
fn endpoints_refuse_cleartext_to_other_machines() {
    assert!(GithubEndpoints::custom("http://example.com", "https://example.com").is_err());
    assert!(GithubEndpoints::custom("https://api.example.com", "http://example.com").is_err());
    assert!(
        GithubEndpoints::custom("https://user:pw@api.example.com", "https://example.com").is_err()
    );
    assert!(
        GithubEndpoints::custom("https://api.example.com/?x=1", "https://example.com").is_err()
    );
    assert!(GithubEndpoints::custom("not a url", "https://example.com").is_err());
    assert!(GithubEndpoints::custom("https://api.example.com", "https://example.com").is_ok());
    assert!(GithubEndpoints::custom("http://127.0.0.1:9", "http://localhost:9/git").is_ok());
}

#[test]
fn installation_references_are_key_names_never_credentials() {
    for bad in [
        "",
        "ghs_abcdef",
        "ghp_abcdef",
        "github_pat_abc",
        &format!("-----BEGIN {}-----", "PRIVATE KEY"),
        "has space",
        "semi;colon",
        &"x".repeat(201),
    ] {
        assert!(InstallationRef::new(bad).is_err(), "{bad:?}");
    }
    for good in ["installations/42", "github.installation.42", "gh:inst-42_a"] {
        assert!(InstallationRef::new(good).is_ok(), "{good:?}");
    }
}

#[test]
fn the_app_jwt_is_rs256_with_a_short_lifetime_for_either_key_format() {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
    let now = 1_900_000_000i64;
    for pem in [&test_key().private_pem, &test_key().pkcs1_pem] {
        let jwt = app_jwt("1234567", &SecretString::new(pem.clone()), now).unwrap();
        assert_eq!(decode_header(jwt.expose()).unwrap().alg, Algorithm::RS256);
        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = false;
        validation.set_issuer(&["1234567"]);
        validation.set_required_spec_claims(&["iss", "exp"]);
        let key = DecodingKey::from_rsa_pem(test_key().public_pem.as_bytes()).unwrap();
        let claims = decode::<serde_json::Value>(jwt.expose(), &key, &validation)
            .unwrap()
            .claims;
        assert_eq!(claims["iat"], now - 60);
        assert_eq!(
            claims["exp"],
            now + 540,
            "GitHub allows at most ten minutes"
        );
    }
    let err = app_jwt("1", &SecretString::new("not a pem"), now).unwrap_err();
    assert_eq!(err.code(), "github_credentials_unavailable");
    assert_eq!(
        format!("{:?}", SecretString::new("hunter2")),
        "SecretString([redacted])"
    );
    assert_eq!(SecretString::new("hunter2").to_string(), "[redacted]");
}

#[test]
fn observed_refs_serialise_as_plain_name_and_sha() {
    let r = ObservedRef {
        name: "refs/heads/main".into(),
        sha: "a".repeat(40),
    };
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"name": "refs/heads/main", "sha": "a".repeat(40)})
    );
}
