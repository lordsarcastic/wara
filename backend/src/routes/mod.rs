use axum::{Router, middleware, routing::get};
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    errors::api::{ApiError, normalize_error_response},
    libs::metrics,
    openapi::ApiDoc,
    state::AppState,
};
use utoipa::OpenApi;

pub mod admin;
pub mod auth;
pub mod credentials;
pub mod deployments;
pub mod domains;
pub mod environments;
pub mod servers;
pub mod services;
pub mod telemetry;
pub mod templates;
pub mod well_known;
pub mod workspaces;

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .merge(auth::router())
        .merge(servers::router())
        .merge(workspaces::router())
        .merge(environments::router())
        .merge(services::router())
        .merge(credentials::router())
        .merge(domains::router())
        .merge(deployments::router())
        .merge(templates::router())
        .merge(telemetry::router())
        .merge(admin::router())
        .fallback(api_not_found)
        .layer(middleware::from_fn(normalize_error_response));

    let mut app = Router::new()
        .merge(well_known::router())
        .route("/health", get(health))
        .route("/metrics", get(metrics::metrics_handler))
        .nest("/api/v1", api)
        .route("/api/openapi.json", get(openapi_json))
        .with_state(state.clone());

    if state.config.docs_enabled {
        app = app.merge(SwaggerUi::new("/docs").url("/docs/openapi.json", ApiDoc::openapi()));
    }

    app
}

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Platform health check"))
)]
pub async fn health() -> &'static str {
    "ok"
}

pub async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(ApiDoc::openapi())
}

async fn api_not_found() -> Result<(), ApiError> {
    Err(ApiError::NotFound("route"))
}
