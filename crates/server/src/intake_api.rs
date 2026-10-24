//! Upload, GitHub import and workspace routes of a project.
//!
//! | Method and path | Role needed | Purpose |
//! |---|---|---|
//! | `POST /projects/{id}/uploads` | owner, maintainer | Multipart ZIP or folder upload into a new workspace |
//! | `GET /projects/{id}/github/repositories` | owner, maintainer | Repositories the caller's App installation can see |
//! | `POST /projects/{id}/github/import` | owner, maintainer | Body `{repository, ref?}`: allow one chosen repository and import a branch |
//! | `GET /projects/{id}/github/link` | any member | The repository link, checked against GitHub |
//! | `GET /projects/{id}/workspace` | any member | The C-01 workspace descriptor |
//!
//! Every handler needs a signed-in [`Principal`] and resolves the project through the accounts
//! crate's tenant-scoped repository (see [`crate::access`]). A project the caller cannot see is
//! `404 not_found`, the same answer as a project that does not exist.
//!
//! # Upload request
//!
//! `multipart/form-data` with these fields, in any order:
//!
//! * `kind`: `zip` or `folder`. When absent, a single `.zip` file is a ZIP and anything else a folder.
//! * `files`: one part per file whose `filename` is the path relative to the uploaded folder
//!   (`FormData.append("files", blob, relativePath)`). A ZIP upload is exactly one `files` part.
//! * `git_init_consent`: `true` to let the server create a Git repository with one baseline
//!   commit when the upload has none. Anything else, or absent, leaves the mod without Git.
//! * `name`: optional display name of the upload (the folder name). Never used as a path.
//!
//! The body limit of this route is `FACTORY_UPLOAD_MAX_BYTES` (default 512 MiB); every other
//! route keeps `FACTORY_MAX_BODY_BYTES`. Each part is spooled to disk as it arrives, so memory
//! use does not grow with the upload.

use std::io::Read;
use std::path::{Path as FsPath, PathBuf};

use axum::Json;
use axum::Router;
use axum::extract::multipart::{Field, Multipart, MultipartError};
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use factory_accounts::{ApiError, ApiJson, Principal, RetryClass};
use factory_intake::{
    DeclaredKind, EntryData, FolderEntry, GitInitRequest, GitOutcome, GithubImportRequest,
    InstallationRef, IntakeError, LinkState, RepositoryLink, UploadMeta, UploadOptions,
    parse_full_name,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::access::{IMPORT_PERMISSION, READ_PERMISSION, active_project_for, project_for};
use crate::api_error::{Fail, intake_error, upload_failure};
use crate::config::cors_origins;
use crate::github::{GithubRuntime, installation_reference};
use crate::state::{IntakeState, ServerState};
use crate::workspace_api::{self, WorkspaceSummary, summary};

/// Largest text field (consent, display name) the upload accepts.
const MAX_TEXT_FIELD: usize = 1024;
/// Branch of the baseline commit made with consent.
const BASELINE_BRANCH: &str = "main";
/// Where a signed-in user links a GitHub account (a browser navigation).
const LINK_GITHUB_PATH: &str = "/api/v1/auth/github/start?link=1";

/// The routes of this module, relative to `/api/v1`.
/// Origins that may make state-changing requests: ours and the configured extras.
pub(crate) fn accepted_origins(state: &ServerState) -> Vec<String> {
    let mut accepted = cors_origins(&state.accounts.config().allowed_origins);
    accepted.extend(cors_origins(std::slice::from_ref(
        &state.accounts.config().public_url,
    )));
    accepted
}

pub fn router(state: &ServerState) -> Router {
    let accepted = accepted_origins(state);
    Router::new()
        .route(
            "/projects/{id}/uploads",
            post(upload).layer(DefaultBodyLimit::max(state.intake.upload_max_bytes)),
        )
        .route("/projects/{id}/github/repositories", get(repositories))
        .route("/projects/{id}/github/import", post(import))
        .route("/projects/{id}/github/link", get(link))
        .route("/projects/{id}/workspace", get(workspace_api::get))
        .layer(middleware::from_fn_with_state(accepted, origin_guard))
        .with_state(state.clone())
}

/// The accounts router's cross-site protection for state-changing requests, applied to these
/// routes too: a request that names an origin must name one of ours, and a browser-declared
/// cross-site request without an Origin header is refused. Clients that send neither pass.
pub(crate) async fn origin_guard(
    State(accepted): State<Vec<String>>,
    request: Request,
    next: Next,
) -> Response {
    let unsafe_method = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    );
    if unsafe_method {
        let headers = request.headers();
        let refused = match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            Some(origin) => {
                let normalised = cors_origins(&[origin.to_owned()]);
                normalised.first().is_none_or(|o| !accepted.contains(o))
            }
            None => headers
                .get("sec-fetch-site")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v == "cross-site"),
        };
        if refused {
            return ApiError::new(
                StatusCode::FORBIDDEN,
                "origin_not_allowed",
                "This request came from an origin that is not allowed.",
                RetryClass::Terminal,
            )
            .into_response();
        }
    }
    next.run(request).await
}

// ---------------------------------------------------------------------------------------------
// Uploads

/// One part of the request that was written to the spool.
struct Spooled {
    path: PathBuf,
    /// `filename` of the part.
    client_name: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Zip,
    Folder,
}

/// Everything the upload request carried, with the bytes already on disk.
struct Received {
    /// Removes the spooled files when dropped.
    _dir: tempfile::TempDir,
    archive: Option<Spooled>,
    files: Vec<Spooled>,
    consent: bool,
    name: Option<String>,
}

fn invalid(message: &str) -> Fail {
    ApiError::invalid_request(message).into()
}

fn multipart_error(error: &MultipartError, state: &IntakeState) -> Fail {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return intake_error(&IntakeError::UploadTooLarge {
            limit: state.upload_max_bytes as u64,
        });
    }
    invalid("The upload could not be read as multipart/form-data.")
}

async fn small_text(field: &mut Field<'_>, state: &IntakeState) -> Result<String, Fail> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|e| multipart_error(&e, state))?
    {
        if bytes.len() + chunk.len() > MAX_TEXT_FIELD {
            return Err(invalid("A form field of the upload is too long."));
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes).map_err(|_| invalid("A form field of the upload is not UTF-8."))
}

/// Streams one file part to `target`, counting its bytes against the upload limit.
async fn spool(
    field: &mut Field<'_>,
    target: &FsPath,
    received: &mut u64,
    state: &IntakeState,
) -> Result<(), Fail> {
    let io_error = |err: std::io::Error| {
        tracing::error!(error = %err, "could not spool an upload part");
        Fail::from(ApiError::internal())
    };
    let mut file = tokio::fs::File::create(target).await.map_err(io_error)?;
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|e| multipart_error(&e, state))?
    {
        *received += chunk.len() as u64;
        if *received > state.limits.max_upload_bytes {
            return Err(intake_error(&IntakeError::UploadTooLarge {
                limit: state.limits.max_upload_bytes,
            }));
        }
        file.write_all(&chunk).await.map_err(io_error)?;
    }
    file.flush().await.map_err(io_error)?;
    Ok(())
}

async fn receive(state: &IntakeState, mut multipart: Multipart) -> Result<Received, Fail> {
    let dir = tempfile::Builder::new()
        .prefix("upload-")
        .tempdir_in(&state.incoming_dir)
        .map_err(|err| {
            tracing::error!(error = %err, "could not create the upload spool");
            Fail::from(ApiError::internal())
        })?;
    let spool_dir = dir.path().to_path_buf();
    let mut received = Received {
        _dir: dir,
        archive: None,
        files: Vec::new(),
        consent: false,
        name: None,
    };

    let mut bytes = 0u64;
    let mut index = 0u64;
    let mut kind: Option<Kind> = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| multipart_error(&e, state))?
    {
        let field_name = field.name().unwrap_or("").to_owned();
        match field_name.as_str() {
            "git_init_consent" => {
                let text = small_text(&mut field, state).await?;
                received.consent = matches!(
                    text.trim().to_ascii_lowercase().as_str(),
                    "true" | "1" | "yes" | "on"
                );
            }
            "name" => {
                let text = small_text(&mut field, state).await?;
                let text = text.trim();
                received.name = (!text.is_empty()).then(|| text.to_owned());
            }
            "kind" => {
                let text = small_text(&mut field, state).await?;
                kind = Some(match text.trim() {
                    "zip" => Kind::Zip,
                    "folder" => Kind::Folder,
                    _ => return Err(invalid("kind must be zip or folder.")),
                });
            }
            "files" => {
                let Some(client_name) = field.file_name().map(str::to_owned) else {
                    return Err(invalid("Every file part needs a filename."));
                };
                if received.files.len() as u64 >= state.limits.max_entries {
                    return Err(intake_error(&IntakeError::TooManyEntries {
                        limit: state.limits.max_entries,
                    }));
                }
                // The spool name is generated; the client's name is only data.
                index += 1;
                let path = spool_dir.join(format!("part-{index:08}"));
                spool(&mut field, &path, &mut bytes, state).await?;
                received.files.push(Spooled { path, client_name });
            }
            _ => {
                return Err(invalid(
                    "The upload has a field this endpoint does not know.",
                ));
            }
        }
    }
    if received.files.is_empty() {
        return Err(invalid("The upload has no files."));
    }
    let kind = kind.unwrap_or_else(|| {
        if received.files.len() == 1
            && received.files[0]
                .client_name
                .to_ascii_lowercase()
                .ends_with(".zip")
        {
            Kind::Zip
        } else {
            Kind::Folder
        }
    });
    if kind == Kind::Zip {
        if received.files.len() != 1 {
            return Err(invalid("A ZIP upload carries exactly one file."));
        }
        received.archive = received.files.pop();
    }
    Ok(received)
}

fn git_summary(git: &GitOutcome) -> Value {
    match git {
        GitOutcome::NotInitialised => json!({ "state": "not_initialised" }),
        GitOutcome::Initialised { head, branch, .. } => {
            json!({ "state": "initialised", "branch": branch, "head": head })
        }
        GitOutcome::ExistingRepository { head, branch, .. } => {
            json!({ "state": "existing_repository", "branch": branch, "head": head })
        }
    }
}

async fn upload(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
    multipart: Multipart,
) -> Result<Response, Fail> {
    let project = active_project_for(&state, &principal, &id, IMPORT_PERMISSION).await?;
    let received = receive(&state.intake, multipart).await?;

    let options = UploadOptions {
        init_git: received
            .consent
            .then(|| GitInitRequest::with_consent(BASELINE_BRANCH)),
    };
    let upload_id = Uuid::new_v4();
    let owner = project.owner_account_id.0;
    let service = &state.intake.service;
    let result = if let Some(archive) = &received.archive {
        let file = std::fs::File::open(&archive.path).map_err(|err| {
            tracing::error!(error = %err, "could not reopen a spooled archive");
            Fail::from(ApiError::internal())
        })?;
        let meta = UploadMeta {
            upload_id,
            original_name: Some(
                received
                    .name
                    .clone()
                    .unwrap_or_else(|| archive.client_name.clone()),
            ),
        };
        let reader: Box<dyn Read + Send> = Box::new(std::io::BufReader::new(file));
        service
            .upload_zip(owner, project.id.0, meta, reader, options)
            .await
    } else {
        let entries: Vec<FolderEntry> = received
            .files
            .iter()
            .map(|part| FolderEntry {
                path: part.client_name.clone(),
                data: EntryData::Path(part.path.clone()),
                mode: None,
                kind: DeclaredKind::File,
            })
            .collect();
        let first_directory = received
            .files
            .first()
            .and_then(|p| p.client_name.split('/').next().map(str::to_owned));
        let meta = UploadMeta {
            upload_id,
            original_name: received.name.clone().or(first_directory),
        };
        service
            .upload_folder(owner, project.id.0, meta, entries, options)
            .await
    };
    // The spooled bytes have been read (or refused) by now.
    drop(received);

    let outcome = result
        .map_err(|failure| upload_failure(&failure, upload_id).with_id("project_id", project.id))?;
    let workspace = summary(
        &project,
        outcome.extracted.files,
        &outcome.inventory.content_hash,
        git_summary(&outcome.git),
    );
    Ok((
        StatusCode::CREATED,
        Json(json!({ "upload": outcome, "workspace": workspace })),
    )
        .into_response())
}

// ---------------------------------------------------------------------------------------------
// GitHub

fn install_details(runtime: &GithubRuntime) -> Value {
    json!({ "install_url": runtime.install_url() })
}

fn not_configured() -> Fail {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "github_not_configured",
        "GitHub import is not available on this server.",
        RetryClass::Terminal,
    )
    .into()
}

struct Installation {
    runtime: GithubRuntime,
    id: u64,
    login: String,
    reference: InstallationRef,
}

/// The App installation of the signed-in user's linked GitHub account. Refuses with
/// `github_not_linked`, `github_app_not_installed`, `installation_suspended` or
/// `installation_not_selected`, each with the link to install the App where that helps.
async fn installation_of(state: &ServerState, principal: &Principal) -> Result<Installation, Fail> {
    let runtime = state.intake.github.clone().ok_or_else(not_configured)?;
    let github_user_id: Option<i64> =
        sqlx::query_scalar("SELECT github_user_id FROM accounts WHERE id = $1")
            .bind(principal.account_id().0)
            .fetch_one(state.db.pool())
            .await?;
    let Some(github_user_id) = github_user_id.and_then(|id| u64::try_from(id).ok()) else {
        return Err(Fail::from(ApiError::new(
            StatusCode::CONFLICT,
            "github_not_linked",
            "Link your GitHub account first, then install the HOI4 Factory GitHub App on the repositories you want to import.",
            RetryClass::RequiresAuthorization,
        ))
        .with_details(json!({
            "install_url": runtime.install_url(),
            "link_url": LINK_GITHUB_PATH,
        })));
    };
    let found = runtime
        .installation_of(github_user_id)
        .await
        .map_err(|e| intake_error(&e))?;
    let Some(found) = found else {
        return Err(Fail::from(ApiError::new(
            StatusCode::CONFLICT,
            "github_app_not_installed",
            "Install the HOI4 Factory GitHub App on your GitHub account, choosing only the repositories you want to import.",
            RetryClass::RequiresAuthorization,
        ))
        .with_details(install_details(&runtime)));
    };
    if found.suspended_at.is_some() {
        return Err(intake_error(&IntakeError::InstallationSuspended)
            .with_details(install_details(&runtime)));
    }
    if found.repository_selection != "selected" {
        return Err(intake_error(&IntakeError::InstallationNotSelected)
            .with_details(install_details(&runtime)));
    }
    let reference =
        InstallationRef::new(installation_reference(found.id)).map_err(|e| intake_error(&e))?;
    Ok(Installation {
        runtime,
        id: found.id,
        login: found.account.login,
        reference,
    })
}

#[derive(Serialize)]
struct RepositoryView {
    full_name: String,
    id: u64,
    default_branch: String,
    private: bool,
    archived: bool,
    size_kib: u64,
    /// True when the project's allowlist already names this repository.
    allowed: bool,
}

async fn repositories(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let project = active_project_for(&state, &principal, &id, IMPORT_PERMISSION).await?;
    let installation = installation_of(&state, &principal).await?;
    let listed = state
        .intake
        .service
        .list_github_repositories(
            project.owner_account_id.0,
            project.id.0,
            &installation.reference,
        )
        .await
        .map_err(|e| intake_error(&e))?;
    let repositories: Vec<RepositoryView> = listed
        .into_iter()
        .map(|visible| RepositoryView {
            full_name: visible.repository.full_name,
            id: visible.repository.id,
            default_branch: visible.repository.default_branch,
            private: visible.repository.private,
            archived: visible.repository.archived,
            size_kib: visible.repository.size,
            allowed: visible.allowed,
        })
        .collect();
    Ok(Json(json!({
        "repositories": repositories,
        "installation": {
            "id": installation.id,
            "account_login": installation.login,
            "repository_selection": "selected",
        },
        "install_url": installation.runtime.install_url(),
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportBody {
    /// `owner/name`.
    #[serde(alias = "repository_full_name")]
    repository: String,
    /// Branch to import; absent or empty means the default branch.
    #[serde(default, rename = "ref", alias = "branch")]
    branch: Option<String>,
}

/// A repository link as the browser sees it. The installation reference and the owner account
/// stay on the server.
#[derive(Serialize)]
struct LinkView {
    id: Uuid,
    workspace_id: String,
    provider: String,
    repository_full_name: String,
    default_branch: Option<String>,
    imported_branch: Option<String>,
    imported_commit: Option<String>,
    observed_refs: Vec<factory_intake::ObservedRef>,
    observed_at: Option<chrono::DateTime<chrono::Utc>>,
    state: LinkState,
    state_reason: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<RepositoryLink> for LinkView {
    fn from(link: RepositoryLink) -> Self {
        LinkView {
            id: link.id,
            workspace_id: link.workspace_id,
            provider: link.provider,
            repository_full_name: link.repository_full_name,
            default_branch: link.default_branch,
            imported_branch: link.imported_branch,
            imported_commit: link.imported_commit,
            observed_refs: link.observed_refs,
            observed_at: link.observed_at,
            state: link.state,
            state_reason: link.state_reason,
            created_at: link.created_at,
            updated_at: link.updated_at,
        }
    }
}

async fn import(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ImportBody>,
) -> Result<Response, Fail> {
    let project = active_project_for(&state, &principal, &id, IMPORT_PERMISSION).await?;
    let repository = body.repository.trim().to_owned();
    if parse_full_name(&repository).is_none() {
        return Err(invalid("repository must be owner/name."));
    }
    let branch = body
        .branch
        .map(|b| b.trim().to_owned())
        .filter(|b| !b.is_empty());
    let owner = project.owner_account_id.0;
    let service = &state.intake.service;

    // The installation first: an unlinked account, a missing installation or one that is not
    // limited to selected repositories refuses before anything about the project changes.
    let installation = installation_of(&state, &principal).await?;

    // One workspace per project. Say so before touching the allowlist.
    let existing = service
        .store()
        .links_for_project(owner, project.id.0)
        .await
        .map_err(|e| intake_error(&e))?;
    if existing
        .iter()
        .any(|l| matches!(l.state, LinkState::Pending | LinkState::Active))
    {
        return Err(Fail::from(
            ApiError::conflict(
                "project_already_linked",
                "This project is already linked to a GitHub repository.",
            )
            .with_id("project_id", project.id),
        ));
    }
    if workspace_api::open_workspace(&state, &project).is_ok() {
        return Err(intake_error(&IntakeError::Workspace(
            factory_workspace::WorkspaceError::AlreadyExists,
        ))
        .with_id("project_id", project.id));
    }

    // Choosing the repository in this request is the user's explicit act of adding it to the
    // project's allowlist. An entry this request added does not outlive its failure.
    let already_allowed = service
        .store()
        .allowlist(owner, project.id.0)
        .await
        .map_err(|e| intake_error(&e))?
        .contains(&repository);
    service
        .store()
        .allow_repository(owner, project.id.0, &repository)
        .await
        .map_err(|e| intake_error(&e))?;

    let request = GithubImportRequest {
        account: owner,
        project: project.id.0,
        link_id: Uuid::new_v4(),
        installation: installation.reference.clone(),
        repository: repository.clone(),
        branch,
    };
    let (link, outcome) = match service.import_github(request).await {
        Ok(done) => done,
        Err(error) => {
            if !already_allowed
                && let Err(db) = service
                    .store()
                    .disallow_repository(owner, project.id.0, &repository)
                    .await
            {
                tracing::error!(error = %db, "could not remove an allowlist entry after a failed import");
            }
            return Err(intake_error(&error).with_id("project_id", project.id));
        }
    };
    let workspace: WorkspaceSummary = summary(
        &project,
        outcome.inventory.entries,
        &outcome.inventory.content_hash,
        json!({
            "state": "existing_repository",
            "branch": outcome.branch,
            "head": outcome.head,
        }),
    );
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "link": LinkView::from(link),
            "import": {
                "repository": outcome.repository.full_name,
                "branch": outcome.branch,
                "head": outcome.head,
                "default_branch": outcome.repository.default_branch,
                "source_locator": outcome.source_locator,
                "findings": outcome.findings,
                "inventory": outcome.inventory,
                "dirty_paths": outcome.dirty_paths,
            },
            "workspace": workspace,
        })),
    )
        .into_response())
}

/// The project's repository link, re-checked against GitHub when it is active. The link row is
/// returned either way; `live_check` says what the check could establish: `ok`, `revoked` (the
/// stored state was changed to `access_revoked`), `unavailable` (GitHub or the credentials could
/// not be reached, the stored state is unchanged) or `not_applicable` (the link is not active).
async fn link(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let owner = project.owner_account_id.0;
    let service = &state.intake.service;
    let mut links = service
        .store()
        .links_for_project(owner, project.id.0)
        .await
        .map_err(|e| intake_error(&e))?;
    // Newest first. A link that is still live wins over a failed or disconnected one.
    let Some(position) = links
        .iter()
        .position(|l| matches!(l.state, LinkState::Pending | LinkState::Active))
        .or((!links.is_empty()).then_some(0))
    else {
        return Ok(Json(
            json!({ "link": null, "live_check": "not_applicable" }),
        ));
    };
    let mut current = links.swap_remove(position);
    let mut live_check = "not_applicable";
    if current.state == LinkState::Active {
        match service.check_link(owner, current.id).await {
            Ok(checked) => {
                live_check = if checked.state == LinkState::Active {
                    "ok"
                } else {
                    "revoked"
                };
                current = checked;
            }
            Err(error) => {
                tracing::warn!(
                    code = error.code(),
                    "repository link check did not complete"
                );
                live_check = "unavailable";
            }
        }
    }
    Ok(Json(json!({
        "link": LinkView::from(current),
        "live_check": live_check,
        "install_url": state.intake.github.as_ref().map(GithubRuntime::install_url),
    })))
}
