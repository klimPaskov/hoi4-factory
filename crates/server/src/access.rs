//! Project access for the routes that act on a project's workspace.
//!
//! Every request resolves the project through the accounts crate's tenant-scoped repository, so
//! PostgreSQL row-level security applies, and then checks the caller's role. A project the
//! caller cannot see (another account's private project, a public project the caller is not a
//! member of, an id that does not exist or does not parse) is the same `404 not_found`.

use std::str::FromStr;

use factory_accounts::{ApiError, Permission, Principal, Project, ProjectId, RateLimitConfig, RateLimiter, Rule};

use crate::state::ServerState;

/// What importing or uploading a mod into the project's workspace requires. The permission
/// matrix (SPEC-005) has no dedicated permission for it yet: the owner and maintainers, the
/// roles that may change the project's settings, may bring in a mod. Change this one constant
/// when the matrix gains it.
pub const IMPORT_PERMISSION: Permission = Permission::RenameProject;

/// Per-account request limits of the routes that cost disk, network or model work (PR-098).
/// Generous for a person working through the flow, tight enough to stop a script.
pub const LIMIT_UPLOAD: &str = "route_upload";
pub const LIMIT_GITHUB_IMPORT: &str = "route_github_import";
pub const LIMIT_SETUP_SCAN: &str = "route_setup_scan";
pub const LIMIT_SETUP_ANALYZE: &str = "route_setup_analyze";
pub const LIMIT_SETUP_PLAN: &str = "route_setup_plan";
pub const LIMIT_SETUP_APPLY: &str = "route_setup_apply";

pub fn route_limits() -> RateLimitConfig {
    let rule = |capacity, secs| Rule { capacity, refill_every: std::time::Duration::from_secs(secs) };
    RateLimitConfig {
        rules: [
            (LIMIT_UPLOAD, rule(20, 180)),
            (LIMIT_GITHUB_IMPORT, rule(10, 300)),
            (LIMIT_SETUP_SCAN, rule(20, 120)),
            (LIMIT_SETUP_ANALYZE, rule(10, 300)),
            (LIMIT_SETUP_PLAN, rule(20, 120)),
            (LIMIT_SETUP_APPLY, rule(10, 120)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect(),
    }
}

/// Counts one request of `rule` for the signed-in account, or refuses it with `429 rate_limited`.
pub fn limit(state: &ServerState, rule: &str, principal: &Principal) -> Result<(), ApiError> {
    state
        .route_limits
        .check(rule, &principal.account_id().to_string())
        .map_err(ApiError::rate_limited)
}

/// What reading the workspace requires.
pub const READ_PERMISSION: Permission = Permission::ReadProject;

/// The project `raw_id` for the signed-in caller, when the caller is a member whose role grants
/// `permission`.
pub async fn project_for(
    state: &ServerState,
    principal: &Principal,
    raw_id: &str,
    permission: Permission,
) -> Result<Project, ApiError> {
    let id = ProjectId::from_str(raw_id).map_err(|_| ApiError::not_found())?;
    let project = state
        .accounts
        .scoped(principal.account_id())
        .get_project(id)
        .await?;
    let role = project.role.ok_or_else(ApiError::not_found)?;
    if !role.allows(permission) {
        return Err(
            ApiError::forbidden("Your role on this project does not allow that action.")
                .with_id("project_id", project.id),
        );
    }
    Ok(project)
}

/// As [`project_for`], and the project must not be waiting for deletion.
pub async fn active_project_for(
    state: &ServerState,
    principal: &Principal,
    raw_id: &str,
    permission: Permission,
) -> Result<Project, ApiError> {
    let project = project_for(state, principal, raw_id, permission).await?;
    if project.pending_deletion() {
        return Err(ApiError::conflict(
            "project_pending_deletion",
            "This project is marked for deletion. Restore it first.",
        )
        .with_id("project_id", project.id));
    }
    Ok(project)
}
