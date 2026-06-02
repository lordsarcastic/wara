use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, post},
};
use axum_valid::Valid;
use serde::Deserialize;
use utoipa::ToSchema;
use validator::Validate;

use crate::{
    errors::ApiError,
    libs::docker::ProxyKind,
    models::servers::Server,
    services::{
        auth::{CurrentUser, ensure_super_admin},
        servers::{CreateServerInput, ServerCheckResponse, ServerService},
    },
    state::AppState,
};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/servers", get(list_servers).post(create_server))
        .route("/servers/{id}", get(get_server))
        .route("/servers/{id}/check", post(check_server))
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateServerRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(length(min = 1, max = 253))]
    pub host: String,
    #[validate(range(min = 1, max = 65535))]
    pub port: Option<u16>,
    #[validate(length(min = 1, max = 64))]
    pub username: String,
    #[validate(length(min = 1))]
    pub public_key: String,
    #[validate(length(min = 1))]
    pub private_key: String,
    #[validate(length(min = 1))]
    pub private_key_passphrase: Option<String>,
    pub default_proxy: Option<ProxyKind>,
}

#[utoipa::path(get, path = "/api/v1/servers", security(("bearer_auth" = [])), responses((status = 200, body = [Server])))]
pub async fn list_servers(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<Server>>, ApiError> {
    ensure_super_admin(&user)?;
    Ok(Json(
        ServerService::new(state.db, state.config.secret_key)
            .list_servers()
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/servers", security(("bearer_auth" = [])), request_body = CreateServerRequest, responses((status = 200, body = Server)))]
pub async fn create_server(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateServerRequest>>,
) -> Result<Json<Server>, ApiError> {
    ensure_super_admin(&user)?;
    Ok(Json(
        ServerService::new(state.db, state.config.secret_key)
            .create_server(payload.into())
            .await?,
    ))
}

#[utoipa::path(get, path = "/api/v1/servers/{id}", security(("bearer_auth" = [])), params(("id" = Uuid, Path)), responses((status = 200, body = Server), (status = 404, body = crate::errors::ErrorResponse)))]
pub async fn get_server(
    Path(id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Server>, ApiError> {
    ensure_super_admin(&user)?;
    Ok(Json(
        ServerService::new(state.db, state.config.secret_key)
            .get_server(id)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/servers/{id}/check", security(("bearer_auth" = [])), params(("id" = Uuid, Path)), responses((status = 200, body = ServerCheckResponse), (status = 404, body = crate::errors::ErrorResponse)))]
pub async fn check_server(
    Path(id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<ServerCheckResponse>, ApiError> {
    ensure_super_admin(&user)?;
    Ok(Json(
        ServerService::new(state.db, state.config.secret_key)
            .check_server(id)
            .await?,
    ))
}

impl From<CreateServerRequest> for CreateServerInput {
    fn from(value: CreateServerRequest) -> Self {
        Self {
            name: value.name,
            host: value.host,
            port: value.port,
            username: value.username,
            public_key: value.public_key,
            private_key: value.private_key,
            private_key_passphrase: value.private_key_passphrase,
            default_proxy: value.default_proxy,
        }
    }
}
