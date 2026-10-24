//! Project endpoints for the signed-in account.

use std::str::FromStr;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::extract::ApiJson;
use crate::error::ApiError;
use crate::ids::ProjectId;
use crate::model::{Permission, Visibility};
use crate::service::{AccountsState, Principal};

/// An id that does not parse is indistinguishable from one that does not exist.
fn project_id(raw: &str) -> Result<ProjectId, ApiError> {
    ProjectId::from_str(raw).map_err(|_| ApiError::not_found())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBody {
    name: String,
    visibility: Option<Visibility>,
}

pub async fn create(
    State(state): State<AccountsState>,
    principal: Principal,
    ApiJson(body): ApiJson<CreateBody>,
) -> Result<Response, ApiError> {
    let project = state
        .scoped(principal.account_id())
        .create_project(&body.name, body.visibility.unwrap_or(Visibility::Private))
        .await?;
    Ok((StatusCode::CREATED, Json(json!({ "project": project }))).into_response())
}

#[derive(Deserialize)]
pub struct ListQuery {
    include_deleted: Option<bool>,
}

pub async fn list(
    State(state): State<AccountsState>,
    principal: Principal,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let projects = state
        .scoped(principal.account_id())
        .list_projects(query.include_deleted.unwrap_or(false))
        .await?;
    Ok(Json(json!({ "projects": projects })))
}

pub async fn get(
    State(state): State<AccountsState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let project = state
        .scoped(principal.account_id())
        .get_project(project_id(&id)?)
        .await?;
    Ok(Json(json!({ "project": project })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateBody {
    name: Option<String>,
    visibility: Option<Visibility>,
}

pub async fn update(
    State(state): State<AccountsState>,
    principal: Principal,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<UpdateBody>,
) -> Result<Json<Value>, ApiError> {
    if body.name.is_none() && body.visibility.is_none() {
        return Err(ApiError::invalid_request(
            "Send a name or a visibility to change.",
        ));
    }
    let id = project_id(&id)?;
    let repo = state.scoped(principal.account_id());
    // Check every permission before changing anything, so a refused request changes nothing.
    if body.name.is_some() {
        repo.authorize(id, Permission::RenameProject).await?;
    }
    if body.visibility.is_some() {
        repo.authorize(id, Permission::ChangeVisibility).await?;
    }
    let mut project = None;
    if let Some(name) = &body.name {
        project = Some(repo.rename_project(id, name).await?);
    }
    if let Some(visibility) = body.visibility {
        project = Some(repo.set_visibility(id, visibility).await?);
    }
    Ok(Json(json!({ "project": project })))
}

/// Marks the project for deletion. The response carries `delete_after`, the end of the grace
/// period.
pub async fn mark_for_deletion(
    State(state): State<AccountsState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let project = state
        .scoped(principal.account_id())
        .mark_for_deletion(project_id(&id)?)
        .await?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "project": project }))).into_response())
}

pub async fn restore(
    State(state): State<AccountsState>,
    principal: Principal,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let project = state
        .scoped(principal.account_id())
        .restore_project(project_id(&id)?)
        .await?;
    Ok(Json(json!({ "project": project })))
}
