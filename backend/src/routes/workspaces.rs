use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use axum_valid::Valid;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    errors::ApiError,
    models::workspaces::Workspace,
    services::{
        auth::{CurrentUser, ensure_super_admin, ensure_workspace_access},
        workspaces::WorkspaceService,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/workspaces", get(list_workspaces).post(create_workspace))
        .route("/workspaces/{id}", get(get_workspace))
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateWorkspaceRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(length(max = 500))]
    pub description: Option<String>,
}

#[utoipa::path(get, path = "/api/v1/workspaces", security(("bearer_auth" = [])), responses((status = 200, body = [Workspace])))]
pub async fn list_workspaces(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<Workspace>>, ApiError> {
    Ok(Json(
        WorkspaceService::new(state.db)
            .list_accessible_workspaces(&user)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/workspaces", security(("bearer_auth" = [])), request_body = CreateWorkspaceRequest, responses((status = 200, body = Workspace)))]
pub async fn create_workspace(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateWorkspaceRequest>>,
) -> Result<Json<Workspace>, ApiError> {
    ensure_super_admin(&user)?;
    Ok(Json(
        WorkspaceService::new(state.db)
            .create_workspace(payload.name, payload.description)
            .await?,
    ))
}

#[utoipa::path(get, path = "/api/v1/workspaces/{id}", security(("bearer_auth" = [])), params(("id" = Uuid, Path)), responses((status = 200, body = Workspace)))]
pub async fn get_workspace(
    Path(id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Workspace>, ApiError> {
    ensure_workspace_access(&user, id)?;
    Ok(Json(
        WorkspaceService::new(state.db).get_workspace(id).await?,
    ))
}
