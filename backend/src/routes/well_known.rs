use axum::{Json, Router, extract::State, routing::get};

use crate::{errors::ApiError, models::users::Jwks, services::auth::AuthService, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/.well-known/jwks.json", get(jwks))
}

#[utoipa::path(
    get,
    path = "/.well-known/jwks.json",
    responses((status = 200, body = Jwks), (status = 500, body = crate::errors::ErrorResponse))
)]
pub async fn jwks(State(state): State<AppState>) -> Result<Json<Jwks>, ApiError> {
    let jwks = AuthService::new(state.db, state.config).jwks().await?;
    Ok(Json(jwks))
}
