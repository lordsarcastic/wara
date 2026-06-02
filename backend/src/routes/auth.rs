use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use axum_valid::Valid;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::{
    errors::ApiError,
    models::users::User,
    services::auth::{AcceptInviteInput, AuthService, CurrentUser, LoginInput},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/invites/accept", post(accept_invite))
        .route("/auth/me", get(me))
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1))]
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub token: String,
    pub user: User,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct AcceptInviteRequest {
    #[validate(length(min = 1))]
    pub token: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses((status = 200, body = LoginResponse), (status = 401, body = crate::errors::ErrorResponse))
)]
pub async fn login(
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<LoginRequest>>,
) -> Result<Json<LoginResponse>, ApiError> {
    let output = AuthService::new(state.db, state.config)
        .login(LoginInput {
            email: payload.email,
            password: payload.password,
        })
        .await?;
    Ok(Json(LoginResponse {
        token: output.token,
        user: output.user,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    security(("bearer_auth" = [])),
    responses((status = 200, body = User), (status = 401, body = crate::errors::ErrorResponse))
)]
pub async fn me(CurrentUser(user): CurrentUser) -> Json<User> {
    Json(user)
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/invites/accept",
    request_body = AcceptInviteRequest,
    responses((status = 200, body = LoginResponse), (status = 401, body = crate::errors::ErrorResponse))
)]
pub async fn accept_invite(
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<AcceptInviteRequest>>,
) -> Result<Json<LoginResponse>, ApiError> {
    let output = AuthService::new(state.db, state.config)
        .accept_invite(AcceptInviteInput {
            token: payload.token,
            password: payload.password,
        })
        .await?;
    Ok(Json(LoginResponse {
        token: output.token,
        user: output.user,
    }))
}
