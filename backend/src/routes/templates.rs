use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use axum_valid::Valid;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    errors::ApiError,
    models::{templates::WorkspaceTemplate, workspaces::Workspace},
    services::{
        auth::{CurrentUser, ensure_super_admin, ensure_workspace_access},
        templates::SecretCopyMode,
        workspace_templates::{
            CreateTemplateInput, CreateWorkspacesFromTemplateInput, WorkspaceTemplateService,
        },
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/workspaces/{workspace_id}/templates",
            get(list_workspace_templates).post(create_template),
        )
        .route(
            "/templates/{id}/workspaces",
            axum::routing::post(create_workspaces_from_template),
        )
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateTemplateRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(length(max = 500))]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateWorkspacesFromTemplateRequest {
    #[validate(length(min = 1, max = 100))]
    pub names: Vec<String>,
    pub secret_copy_mode: SecretCopyMode,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkWorkspaceCreateResponse {
    pub workspaces: Vec<Workspace>,
}

#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}/templates", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), responses((status = 200, body = [WorkspaceTemplate])))]
pub async fn list_workspace_templates(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<WorkspaceTemplate>>, ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        WorkspaceTemplateService::new(state.db)
            .list_workspace_templates(workspace_id)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/templates", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), request_body = CreateTemplateRequest, responses((status = 200, body = WorkspaceTemplate)))]
pub async fn create_template(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateTemplateRequest>>,
) -> Result<Json<WorkspaceTemplate>, ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        WorkspaceTemplateService::new(state.db)
            .create_template(CreateTemplateInput {
                workspace_id,
                name: payload.name,
                description: payload.description,
            })
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/templates/{id}/workspaces", security(("bearer_auth" = [])), params(("id" = Uuid, Path)), request_body = CreateWorkspacesFromTemplateRequest, responses((status = 200, body = BulkWorkspaceCreateResponse)))]
pub async fn create_workspaces_from_template(
    Path(id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateWorkspacesFromTemplateRequest>>,
) -> Result<Json<BulkWorkspaceCreateResponse>, ApiError> {
    ensure_super_admin(&user)?;
    let template = WorkspaceTemplateService::new(state.db.clone())
        .get_template(id)
        .await?;
    ensure_workspace_access(&user, template.source_workspace_id)?;
    Ok(Json(BulkWorkspaceCreateResponse {
        workspaces: WorkspaceTemplateService::new(state.db)
            .create_workspaces_from_template(CreateWorkspacesFromTemplateInput {
                template_id: id,
                names: payload.names,
                secret_copy_mode: payload.secret_copy_mode,
            })
            .await?,
    }))
}
