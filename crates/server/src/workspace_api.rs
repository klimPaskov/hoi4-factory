//! `GET /api/v1/projects/{id}/workspace`: the C-01 `WorkspaceDescriptor`.
//!
//! The descriptor is assembled by `Workspace::describe` from the files and Git state on disk
//! plus the server game reference. It names roots by logical locator (`workspace:<project>/source`,
//! `/game`), never by host path, and carries no credential. The work reads and hashes files, so
//! it runs on the blocking pool.

use axum::Json;
use axum::extract::{Path, State};
use factory_accounts::{ApiError, Principal, Project, Role as ProjectRole};
use factory_game_ref::DEFAULT_MOUNT_LOCATOR;
use factory_workspace::{
    Access, AccountId, Capability, CapabilityStatus, DescriptorInputs, EntryKind, GameProfile,
    MemberRole, ProjectId, Role, RootHandle, RootKind, Workspace, WorkspaceDescriptor,
    WorkspaceError, WorkspaceId,
};
use serde::Serialize;
use serde_json::{Value, json};

use crate::access::{READ_PERMISSION, project_for};
use crate::api_error::Fail;
use crate::state::ServerState;

fn role(role: ProjectRole) -> Role {
    match role {
        ProjectRole::Owner => Role::Owner,
        ProjectRole::Maintainer => Role::Maintainer,
        ProjectRole::Contributor => Role::Contributor,
        ProjectRole::Viewer => Role::Viewer,
    }
}

/// Opens the project's workspace. The workspace belongs to the project owner's account.
pub fn open_workspace(state: &ServerState, project: &Project) -> Result<Workspace, Fail> {
    let account = AccountId::new(project.owner_account_id.to_string());
    let id = ProjectId::new(project.id.to_string());
    let (Ok(account), Ok(id)) = (account, id) else {
        return Err(ApiError::internal().into());
    };
    match state.storage.open_workspace(&account, &id) {
        Ok(workspace) => Ok(workspace),
        Err(WorkspaceError::NotFound) => Err(ApiError::new(
            axum::http::StatusCode::NOT_FOUND,
            "workspace_not_found",
            "This project has no workspace yet. Upload a mod or import a repository first.",
            factory_accounts::RetryClass::RefreshPreconditions,
        )
        .with_id("project_id", project.id)
        .into()),
        Err(err) => {
            tracing::error!(error = %err, "could not open a workspace");
            Err(ApiError::internal().into())
        }
    }
}

/// Descriptor of `workspace` for the caller, built on the blocking pool.
pub async fn describe(
    state: &ServerState,
    workspace: Workspace,
    project: &Project,
    caller: factory_accounts::AccountId,
) -> Result<WorkspaceDescriptor, Fail> {
    let reference = state.game_reference.reference();
    let mut inputs = DescriptorInputs::new(
        WorkspaceId::new(project.id.to_string()).map_err(|_| ApiError::internal())?,
    );
    // The caller's own role. Listing every member is an owner-only call of the accounts API.
    if let Some(caller_role) = project.role
        && let Ok(account) = AccountId::new(caller.to_string())
    {
        inputs.member_roles.push(MemberRole {
            account_id: account,
            role: role(caller_role),
        });
    }
    let available = reference.is_some();
    if available {
        inputs.extra_roots.push(RootHandle {
            kind: RootKind::GameReference,
            locator: DEFAULT_MOUNT_LOCATOR.to_owned(),
            access: Access::ReadOnly,
        });
    }
    inputs.capabilities.push(Capability {
        name: "game_reference".to_owned(),
        status: if available {
            CapabilityStatus::Available
        } else {
            CapabilityStatus::Unavailable
        },
        reason: (!available).then(|| "the server game reference is not mounted".to_owned()),
    });
    inputs.game_profile = GameProfile::new(reference, None, Vec::new());
    tokio::task::spawn_blocking(move || workspace.describe(inputs))
        .await
        .map_err(|_| ApiError::internal())?
        .map_err(|err| {
            tracing::error!(error = %err, "could not describe a workspace");
            Fail::from(ApiError::internal())
        })
}

/// A compact view of a workspace for upload and import responses.
#[derive(Debug, Serialize)]
pub struct WorkspaceSummary {
    pub workspace_id: String,
    pub source_locator: String,
    pub files: u64,
    pub inventory_hash: String,
    pub git: Value,
}

pub fn summary(
    project: &Project,
    files: u64,
    inventory_hash: &str,
    git: Value,
) -> WorkspaceSummary {
    WorkspaceSummary {
        workspace_id: project.id.to_string(),
        source_locator: format!("workspace:{}/source", project.id),
        files,
        inventory_hash: inventory_hash.to_owned(),
        git,
    }
}

/// Where the project's source came from: a live GitHub link, an accepted upload, or neither.
async fn origin_of(state: &ServerState, project: &Project) -> Result<Option<&'static str>, Fail> {
    let mut tx = state
        .accounts
        .scoped(project.owner_account_id)
        .begin()
        .await?;
    let linked: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM repository_links
                        WHERE project_id = $1 AND state IN ('pending', 'active', 'access_revoked'))",
    )
    .bind(project.id.0)
    .fetch_one(&mut *tx)
    .await?;
    let uploaded: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM intake_uploads WHERE project_id = $1 AND status = 'accepted')",
    )
    .bind(project.id.0)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(if linked {
        Some("github")
    } else if uploaded {
        Some("upload")
    } else {
        None
    })
}

/// `GET /projects/{id}/workspace`: `{ "summary": {...}, "descriptor": {...} | null }`.
///
/// The summary is what the browser shows: `has_source`, `origin` (`upload`, `github` or null),
/// `file_count`, `ref` (the branch), `revision` (first 12 characters of HEAD) and `setup_state`
/// (`not_started` until a Setup receipt exists). A project without a workspace answers 200 with
/// `has_source: false` and `descriptor: null`, so a client does not treat "nothing yet" as an error.
pub async fn get(
    State(state): State<ServerState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, Fail> {
    let project = project_for(&state, &principal, &id, READ_PERMISSION).await?;
    let workspace = match open_workspace(&state, &project) {
        Ok(workspace) => workspace,
        Err(fail) if fail.error.code == "workspace_not_found" => {
            return Ok(Json(json!({
                "summary": {
                    "has_source": false,
                    "origin": null,
                    "file_count": 0,
                    "ref": null,
                    "revision": null,
                    "setup_state": "not_started",
                },
                "descriptor": null,
            })));
        }
        Err(other) => return Err(other),
    };
    let descriptor = describe(&state, workspace, &project, principal.account_id()).await?;
    let file_count = descriptor
        .source_snapshot
        .inventory
        .entries
        .iter()
        .filter(|e| e.kind == EntryKind::File)
        .count();
    let origin = origin_of(&state, &project).await?;
    let revision = descriptor
        .git_state
        .head
        .as_ref()
        .map(|h| h.as_str().chars().take(12).collect::<String>());
    let setup_state = descriptor
        .setup_receipt
        .as_ref()
        .map_or("not_started", |r| r.transaction_state.as_str())
        .to_owned();
    Ok(Json(json!({
        "summary": {
            "has_source": file_count > 0,
            "origin": origin,
            "file_count": file_count,
            "ref": descriptor.git_state.branch,
            "revision": revision,
            "setup_state": setup_state,
        },
        "descriptor": descriptor,
    })))
}
