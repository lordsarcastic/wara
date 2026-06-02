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
    models::environments::Environment,
    services::{
        auth::{CurrentUser, ensure_workspace_access},
        workspaces::WorkspaceService,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/workspaces/{workspace_id}/environments",
        get(list_environments).post(create_environment),
    )
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateEnvironmentRequest {
    #[validate(length(min = 1, max = 80))]
    pub name: String,
}

#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}/environments", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), responses((status = 200, body = [Environment])))]
pub async fn list_environments(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<Environment>>, ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        WorkspaceService::new(state.db)
            .list_environments(workspace_id)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/environments", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), request_body = CreateEnvironmentRequest, responses((status = 200, body = Environment)))]
pub async fn create_environment(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateEnvironmentRequest>>,
) -> Result<Json<Environment>, ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        WorkspaceService::new(state.db)
            .create_environment(workspace_id, payload.name)
            .await?,
    ))
}
