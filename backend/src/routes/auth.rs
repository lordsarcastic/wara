use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{delete, get, post},
};
use axum_valid::Valid;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use uuid::Uuid;

use crate::{
    errors::ApiError,
    models::users::{User, UserApiTokenRecord},
    services::auth::{
        AcceptInviteInput, AuthService, CreateApiTokenInput, CurrentUser, LoginInput,
    },
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/invites/accept", post(accept_invite))
        .route("/auth/me", get(me))
        .route(
            "/auth/api-tokens",
            get(list_api_tokens).post(create_api_token),
        )
        .route("/auth/api-tokens/{id}", delete(revoke_api_token))
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

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct CreateApiTokenRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiTokenResponse {
    pub id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub created_at: String,
    pub revoked_at: Option<String>,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateApiTokenResponse {
    pub token: String,
    pub api_token: ApiTokenResponse,
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

#[utoipa::path(
    get,
    path = "/api/v1/auth/api-tokens",
    security(("bearer_auth" = [])),
    responses((status = 200, body = [ApiTokenResponse]), (status = 401, body = crate::errors::ErrorResponse))
)]
pub async fn list_api_tokens(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<ApiTokenResponse>>, ApiError> {
    Ok(Json(
        AuthService::new(state.db, state.config)
            .list_api_tokens(&user)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/api-tokens",
    security(("bearer_auth" = [])),
    request_body = CreateApiTokenRequest,
    responses((status = 200, body = CreateApiTokenResponse), (status = 401, body = crate::errors::ErrorResponse))
)]
pub async fn create_api_token(
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<CreateApiTokenRequest>>,
) -> Result<Json<CreateApiTokenResponse>, ApiError> {
    let output = AuthService::new(state.db, state.config)
        .create_api_token(CreateApiTokenInput {
            user,
            name: payload.name,
        })
        .await?;
    Ok(Json(CreateApiTokenResponse {
        token: output.token,
        api_token: output.api_token.into(),
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/auth/api-tokens/{id}",
    security(("bearer_auth" = [])),
    params(("id" = Uuid, Path)),
    responses((status = 200, body = ApiTokenResponse), (status = 401, body = crate::errors::ErrorResponse), (status = 404, body = crate::errors::ErrorResponse))
)]
pub async fn revoke_api_token(
    Path(id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    State(state): State<AppState>,
) -> Result<Json<ApiTokenResponse>, ApiError> {
    Ok(Json(
        AuthService::new(state.db, state.config)
            .revoke_api_token(&user, id)
            .await?
            .into(),
    ))
}

impl From<UserApiTokenRecord> for ApiTokenResponse {
    fn from(token: UserApiTokenRecord) -> Self {
        Self {
            id: token.id,
            name: token.name,
            token_prefix: token.token_prefix,
            created_at: token.created_at,
            revoked_at: token.revoked_at,
            last_used_at: token.last_used_at,
        }
    }
}
