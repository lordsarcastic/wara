use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, patch, post},
};
use axum_valid::Valid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::{
    errors::api::ApiError,
    models::users::{Role, User},
    services::auth::{AdminUser, AuthService, ChangeUserRoleInput, CurrentUser, InviteUserInput},
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/users", get(list_users).post(invite_user))
        .route("/admin/users/{id}/disable", post(disable_user))
        .route("/admin/users/{id}/reactivate", post(reactivate_user))
        .route("/admin/users/{id}/role", patch(change_user_role))
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct InviteUserRequest {
    pub workspace_id: Uuid,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub role: Role,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InviteUserResponse {
    pub user: User,
    pub invite_link: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema, Validate)]
pub struct ChangeUserRoleRequest {
    pub role: Role,
}

#[utoipa::path(get, path = "/api/v1/admin/users", security(("bearer_auth" = [])), responses((status = 200, body = [User])))]
pub async fn list_users(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<User>>, ApiError> {
    Ok(Json(
        AuthService::new(state.db, state.config)
            .list_users()
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/admin/users", security(("bearer_auth" = [])), request_body = InviteUserRequest, responses((status = 200, body = InviteUserResponse)))]
pub async fn invite_user(
    CurrentUser(inviter): CurrentUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<InviteUserRequest>>,
) -> Result<Json<InviteUserResponse>, ApiError> {
    let output = AuthService::new(state.db, state.config)
        .invite_user(InviteUserInput {
            inviter,
            workspace_id: payload.workspace_id,
            email: payload.email,
            name: payload.name,
            role: payload.role,
        })
        .await?;
    Ok(Json(InviteUserResponse {
        user: output.user,
        invite_link: output.invite_link,
        expires_at: output.expires_at,
    }))
}

#[utoipa::path(post, path = "/api/v1/admin/users/{id}/disable", security(("bearer_auth" = [])), params(("id" = Uuid, Path)), responses((status = 200, body = User)))]
pub async fn disable_user(
    Path(id): Path<Uuid>,
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<User>, ApiError> {
    Ok(Json(
        AuthService::new(state.db, state.config)
            .disable_user(id)
            .await?,
    ))
}

#[utoipa::path(post, path = "/api/v1/admin/users/{id}/reactivate", security(("bearer_auth" = [])), params(("id" = Uuid, Path)), responses((status = 200, body = User)))]
pub async fn reactivate_user(
    Path(id): Path<Uuid>,
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<User>, ApiError> {
    Ok(Json(
        AuthService::new(state.db, state.config)
            .reactivate_user(id)
            .await?,
    ))
}

#[utoipa::path(patch, path = "/api/v1/admin/users/{id}/role", security(("bearer_auth" = [])), params(("id" = Uuid, Path)), request_body = ChangeUserRoleRequest, responses((status = 200, body = User)))]
pub async fn change_user_role(
    Path(id): Path<Uuid>,
    _admin: AdminUser,
    State(state): State<AppState>,
    Valid(Json(payload)): Valid<Json<ChangeUserRoleRequest>>,
) -> Result<Json<User>, ApiError> {
    Ok(Json(
        AuthService::new(state.db, state.config)
            .change_user_role(ChangeUserRoleInput {
                user_id: id,
                role: payload.role,
            })
            .await?,
    ))
}
