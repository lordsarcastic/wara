use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use axum_valid::Valid;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    services::{
        auth::{CurrentUser, ensure_workspace_access},
        credentials::{CreateCredentialInput, CreateEnvVarInput, CredentialService},
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/workspaces/{workspace_id}/credentials",
            get(list_credentials).post(create_credential),
        )
        .route(
            "/workspaces/{workspace_id}/env-vars",
            get(list_env_vars).post(create_env_var),
        )
        .route(
            "/workspaces/{workspace_id}/env-vars/bulk",
            post(create_env_vars),
        )
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateCredentialRequest {
    #[validate(length(min = 1, max = 253))]
    pub registry: String,
    #[validate(length(min = 1))]
    pub username: String,
    #[validate(length(min = 1))]
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CredentialResponse {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub registry: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Serialize, ToSchema, Validate)]
pub struct CreateEnvVarRequest {
    pub environment_id: Option<Uuid>,
    pub service_id: Option<Uuid>,
    #[validate(length(min = 1, max = 255))]
    pub key: String,
    #[validate(length(min = 1))]
    pub value: String,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateEnvVarsRequest {
    #[validate(length(min = 1, max = 100), nested)]
    pub env_vars: Vec<CreateEnvVarRequest>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EnvVarResponse {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub service_id: Option<Uuid>,
    pub key: String,
    pub value: String,
}

#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}/credentials", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), responses((status = 200, body = [CredentialResponse])))]
pub async fn list_credentials(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<CredentialResponse>>, crate::errors::api::ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        CredentialService::new(state.db, state.config.secret_key)
            .list_credentials(workspace_id)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/credentials", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), request_body = CreateCredentialRequest, responses((status = 200, body = CredentialResponse)))]
pub async fn create_credential(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateCredentialRequest>>,
) -> Result<Json<CredentialResponse>, crate::errors::api::ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        CredentialService::new(state.db, state.config.secret_key)
            .create_credential(CreateCredentialInput {
                workspace_id,
                registry: payload.registry,
                username: payload.username,
                password: payload.password,
            })
            .await?,
    ))
}

#[utoipa::path(get, path = "/api/v1/workspaces/{workspace_id}/env-vars", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), responses((status = 200, body = [EnvVarResponse])))]
pub async fn list_env_vars(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<EnvVarResponse>>, crate::errors::api::ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        CredentialService::new(state.db, state.config.secret_key)
            .list_env_vars(workspace_id)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/env-vars", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), request_body = CreateEnvVarRequest, responses((status = 200, body = EnvVarResponse)))]
pub async fn create_env_var(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateEnvVarRequest>>,
) -> Result<Json<EnvVarResponse>, crate::errors::api::ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        CredentialService::new(state.db, state.config.secret_key)
            .create_env_var(CreateEnvVarInput {
                workspace_id,
                environment_id: payload.environment_id,
                service_id: payload.service_id,
                key: payload.key,
                value: payload.value,
            })
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/workspaces/{workspace_id}/env-vars/bulk", security(("bearer_auth" = [])), params(("workspace_id" = Uuid, Path)), request_body = CreateEnvVarsRequest, responses((status = 200, body = [EnvVarResponse])))]
pub async fn create_env_vars(
    Path(workspace_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateEnvVarsRequest>>,
) -> Result<Json<Vec<EnvVarResponse>>, crate::errors::api::ApiError> {
    ensure_workspace_access(&user, workspace_id)?;
    Ok(Json(
        CredentialService::new(state.db, state.config.secret_key)
            .create_env_vars(
                payload
                    .env_vars
                    .into_iter()
                    .map(|env_var| CreateEnvVarInput {
                        workspace_id,
                        environment_id: env_var.environment_id,
                        service_id: env_var.service_id,
                        key: env_var.key,
                        value: env_var.value,
                    })
                    .collect(),
            )
            .await?,
    ))
}
