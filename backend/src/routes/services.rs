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
    libs::docker::DeployKind,
    models::services::AppService,
    services::{
        app_services::{AppServiceService, CreateAppServiceInput},
        auth::{CurrentUser, ensure_workspace_access},
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/workspaces/{workspace_id}/services",
            get(list_services).post(create_service),
        )
        .route("/services/{id}", get(get_service))
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateServiceRequest {
    pub environment_id: Uuid,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub deploy_kind: DeployKind,
    #[validate(length(min = 1))]
    pub image: Option<String>,
    #[validate(length(min = 1))]
    pub compose_file: Option<String>,
    #[validate(length(min = 1))]
    pub dockerfile: Option<String>,
    #[validate(range(min = 1, max = 65535))]
    pub internal_port: Option<u16>,
}

#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}/services", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), responses((status = 200, body = [AppService])))]
pub async fn list_services(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<AppService>>, ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        AppServiceService::new(state.db)
            .list_services(workspace_id)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/services", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), request_body = CreateServiceRequest, responses((status = 200, body = AppService)))]
pub async fn create_service(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateServiceRequest>>,
) -> Result<Json<AppService>, ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        AppServiceService::new(state.db)
            .create_service(CreateAppServiceInput {
                workspace_id,
                environment_id: payload.environment_id,
                name: payload.name,
                deploy_kind: payload.deploy_kind,
                image: payload.image,
                compose_file: payload.compose_file,
                dockerfile: payload.dockerfile,
                internal_port: payload.internal_port,
            })
            .await?,
    ))
}

#[utoipa::path(get, path = "/api/v1/services/{id}", security(("bearer_auth" = [])), params(("id" = Uuid, Path)), responses((status = 200, body = AppService)))]
pub async fn get_service(
    Path(id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<AppService>, ApiError> {
    let service = AppServiceService::new(state.db).get_service(id).await?;
    ensure_workspace_access(&user, service.workspace_id)?;
    Ok(Json(service))
}
