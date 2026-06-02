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
    errors::ApiError,
    libs::docker::ProxyKind,
    models::domains::Domain,
    services::{
        app_services::AppServiceService,
        auth::{CurrentUser, ensure_workspace_access},
        domains::{CreateDomainInput, DomainService},
        proxy,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/services/{service_id}/domains",
            get(list_domains).post(create_domain),
        )
        .route("/domains/{id}/proxy-preview", post(preview_proxy))
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateDomainRequest {
    #[validate(length(min = 1, max = 253))]
    pub hostname: String,
    pub proxy: Option<ProxyKind>,
    pub tls_enabled: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProxyPreviewResponse {
    pub config: String,
}

#[utoipa::path(get, path = "/api/v1/services/{service_id}/domains", security(("bearer_auth" = [])), params(("service_id" = Uuid, Path)), responses((status = 200, body = [Domain])))]
pub async fn list_domains(
    Path(service_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<Domain>>, ApiError> {
    let service = AppServiceService::new(state.db.clone())
        .get_service(service_id)
        .await?;
    ensure_workspace_access(&user, service.workspace_id)?;
    Ok(Json(
        DomainService::new(state.db)
            .list_domains(service_id)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/services/{service_id}/domains", security(("bearer_auth" = [])), params(("service_id" = Uuid, Path)), request_body = CreateDomainRequest, responses((status = 200, body = Domain)))]
pub async fn create_domain(
    Path(service_id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateDomainRequest>>,
) -> Result<Json<Domain>, ApiError> {
    let service = AppServiceService::new(state.db.clone())
        .get_service(service_id)
        .await?;
    ensure_workspace_access(&user, service.workspace_id)?;
    Ok(Json(
        DomainService::new(state.db)
            .create_domain(CreateDomainInput {
                service_id,
                hostname: payload.hostname,
                proxy: payload.proxy,
                tls_enabled: payload.tls_enabled,
            })
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/domains/{id}/proxy-preview", security(("bearer_auth" = [])), params(("id" = Uuid, Path)), responses((status = 200, body = ProxyPreviewResponse)))]
pub async fn preview_proxy(
    Path(id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<ProxyPreviewResponse>, ApiError> {
    let domain = DomainService::new(state.db.clone()).get_domain(id).await?;
    let service = AppServiceService::new(state.db)
        .get_service(domain.service_id)
        .await?;
    ensure_workspace_access(&user, service.workspace_id)?;
    Ok(Json(ProxyPreviewResponse {
        config: proxy::generate_config(&domain, "127.0.0.1:8080"),
    }))
}
