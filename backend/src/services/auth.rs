use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, request::Parts},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, decode_header, encode,
    get_current_timestamp,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use toasty::Executor;
use toasty::stmt::{List, Query, Update};
use uuid::Uuid;

use crate::{
    errors::ApiError,
    libs::{config::Config, db::Database},
    models::{
        users::{
            Role, User, UserApiTokenRecord, UserInviteRecord, UserRecord, UserRefreshTokenRecord,
            UserStatus, WorkspaceUserRoleRecord,
        },
        workspaces::Workspace,
    },
    state::AppState,
};

#[derive(Debug, Clone)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct LoginOutput {
    pub token: String,
    pub refresh_token: String,
    pub user: User,
}

#[derive(Debug, Clone)]
pub struct InviteUserInput {
    pub inviter: User,
    pub workspace_id: Uuid,
    pub email: String,
    pub name: String,
    pub role: Role,
}

#[derive(Debug, Clone)]
pub struct InviteUserOutput {
    pub user: User,
    pub invite_link: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AcceptInviteInput {
    pub token: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct RefreshSessionInput {
    pub refresh_token: String,
}

#[derive(Debug, Clone)]
pub struct LogoutInput {
    pub refresh_token: String,
}

#[derive(Debug, Clone)]
pub struct CreateApiTokenInput {
    pub user: User,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct CreateApiTokenOutput {
    pub token: String,
    pub api_token: UserApiTokenRecord,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    jti: String,
    ret: String,
    sub: String,
    email: String,
    role: String,
    iss: String,
    aud: String,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct RefreshClaims {
    jti: String,
    sub: String,
    typ: String,
    iss: String,
    aud: String,
    iat: u64,
    exp: u64,
}

struct RefreshTokenIssue {
    token: String,
    record: UserRefreshTokenRecord,
}

#[derive(Clone)]
pub struct AuthService {
    db: Database,
    config: Config,
}

impl AuthService {
    pub fn new(db: Database, config: Config) -> Self {
        Self { db, config }
    }

    pub async fn bootstrap_admin(&self) -> Result<(), ApiError> {
        let mut db = self.db.handle()?;
        let users = Query::<List<UserRecord>>::all()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        if !users.is_empty() {
            return Ok(());
        }

        let password_hash = hash_password(&self.config.bootstrap_admin_password)?;
        toasty::create!(UserRecord {
            id: Uuid::now_v7(),
            email: self.config.bootstrap_admin_email.clone(),
            name: self.config.bootstrap_admin_name.clone(),
            role: Role::SuperAdmin.as_str().to_string(),
            status: UserStatus::Active.as_str().to_string(),
            password_hash,
        })
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(())
    }

    pub async fn login(&self, input: LoginInput) -> Result<LoginOutput, ApiError> {
        let user_record = self.find_user_by_email(&input.email).await?;
        if user_record.status != UserStatus::Active.as_str() {
            return Err(ApiError::Unauthorized);
        }
        if user_record.password_hash.is_empty() {
            return Err(ApiError::Unauthorized);
        }
        verify_password(&input.password, &user_record.password_hash)?;
        let user = self.user_from_record(user_record).await?;
        let mut db = self.db.handle()?;
        let refresh_token = self.issue_refresh_token(&mut db, user.id).await?;
        let token = self.sign_access_token(&user, refresh_token.record.id)?;
        Ok(LoginOutput {
            token,
            refresh_token: refresh_token.token,
            user,
        })
    }

    pub async fn invite_user(&self, input: InviteUserInput) -> Result<InviteUserOutput, ApiError> {
        if input.role == Role::SuperAdmin {
            return Err(ApiError::BadRequest(
                "super_admin is a global bootstrap-only role".to_string(),
            ));
        }
        if !can_administer_workspace(&input.inviter, input.workspace_id) {
            return Err(ApiError::Forbidden);
        }
        if self.find_user_by_email(&input.email).await.is_ok() {
            return Err(ApiError::BadRequest("user already exists".to_string()));
        }
        self.ensure_workspace_exists(input.workspace_id).await?;

        let now = Utc::now();
        let expires_at = now + Duration::seconds(self.config.invite_token_ttl_seconds as i64);
        let raw_token = generate_invite_token();
        let token_hash = hash_invite_token(&raw_token);

        let mut db = self.db.handle()?;
        let mut tx = db.transaction().await.map_err(map_toasty_error)?;
        let user_record = toasty::create!(UserRecord {
            id: Uuid::now_v7(),
            email: input.email,
            name: input.name,
            role: Role::Viewer.as_str().to_string(),
            status: UserStatus::Invited.as_str().to_string(),
            password_hash: String::new(),
        })
        .exec(&mut tx)
        .await
        .map_err(map_toasty_error)?;

        toasty::create!(UserInviteRecord {
            id: Uuid::now_v7(),
            user_id: user_record.id,
            token_hash,
            expires_at: expires_at.to_rfc3339(),
            accepted_at: String::new(),
            created_at: now.to_rfc3339(),
        })
        .exec(&mut tx)
        .await
        .map_err(map_toasty_error)?;

        let workspace_role = toasty::create!(WorkspaceUserRoleRecord {
            id: Uuid::now_v7(),
            user_id: user_record.id,
            workspace_id: input.workspace_id,
            role: input.role.as_str().to_string(),
        })
        .exec(&mut tx)
        .await
        .map_err(map_toasty_error)?;
        tx.commit().await.map_err(map_toasty_error)?;

        let mut user = User::from(user_record);
        user.workspace_roles = vec![workspace_role.into()];
        Ok(InviteUserOutput {
            user,
            invite_link: invite_link(&self.config.app_base_url, &raw_token),
            expires_at,
        })
    }

    pub async fn accept_invite(&self, input: AcceptInviteInput) -> Result<LoginOutput, ApiError> {
        let invite = self
            .find_open_invite(&hash_invite_token(&input.token))
            .await?;
        let expires_at = parse_rfc3339(&invite.expires_at)?;
        if expires_at < Utc::now() {
            return Err(ApiError::Unauthorized);
        }

        let user = self.get_user_record(invite.user_id).await?;
        if user.status == UserStatus::Active.as_str() {
            return Err(ApiError::BadRequest(
                "invite has already been accepted".to_string(),
            ));
        }

        let password_hash = hash_password(&input.password)?;
        let mut db = self.db.handle()?;
        let mut update_user = Update::<List<UserRecord>>::new(Query::<List<UserRecord>>::filter(
            UserRecord::fields().id().eq(invite.user_id),
        ));
        update_user.set(4, UserStatus::Active.as_str());
        update_user.set(5, password_hash);
        // Without this, exec returns a sparse record (only the set columns) that
        // Toasty cannot materialize back into a full UserRecord. The updated user is
        // re-fetched below, so no returning clause is needed.
        update_user.set_returning_none();
        update_user.exec(&mut db).await.map_err(map_toasty_error)?;

        let mut update_invite =
            Update::<List<UserInviteRecord>>::new(Query::<List<UserInviteRecord>>::filter(
                UserInviteRecord::fields().id().eq(invite.id),
            ));
        update_invite.set(4, Utc::now().to_rfc3339());
        update_invite.set_returning_none();
        update_invite
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;

        let user = self.get_user(invite.user_id).await?;
        let refresh_token = self.issue_refresh_token(&mut db, user.id).await?;
        let token = self.sign_access_token(&user, refresh_token.record.id)?;
        Ok(LoginOutput {
            token,
            refresh_token: refresh_token.token,
            user,
        })
    }

    pub async fn refresh_session(
        &self,
        input: RefreshSessionInput,
    ) -> Result<LoginOutput, ApiError> {
        let record = self
            .active_refresh_token_record(&input.refresh_token)
            .await?;
        let user_record = self.get_user_record(record.user_id).await?;
        if user_record.status != UserStatus::Active.as_str() {
            return Err(ApiError::Unauthorized);
        }
        let user = self.user_from_record(user_record).await?;

        let mut db = self.db.handle()?;
        let mut tx = db.transaction().await.map_err(map_toasty_error)?;
        let refresh_token = self.issue_refresh_token(&mut tx, user.id).await?;

        let now = Utc::now();
        let mut update_old = Update::<List<UserRefreshTokenRecord>>::new(Query::<
            List<UserRefreshTokenRecord>,
        >::filter(
            UserRefreshTokenRecord::fields().id().eq(record.id),
        ));
        update_old.set(4, now.to_rfc3339());
        update_old.set_returning_none();
        update_old.exec(&mut tx).await.map_err(map_toasty_error)?;
        tx.commit().await.map_err(map_toasty_error)?;

        let token = self.sign_access_token(&user, refresh_token.record.id)?;
        Ok(LoginOutput {
            token,
            refresh_token: refresh_token.token,
            user,
        })
    }

    pub async fn logout(&self, input: LogoutInput) -> Result<(), ApiError> {
        let record = self
            .active_refresh_token_record(&input.refresh_token)
            .await?;
        let mut db = self.db.handle()?;
        let mut update_token = Update::<List<UserRefreshTokenRecord>>::new(Query::<
            List<UserRefreshTokenRecord>,
        >::filter(
            UserRefreshTokenRecord::fields().id().eq(record.id),
        ));
        update_token.set(4, Utc::now().to_rfc3339());
        update_token.set_returning_none();
        update_token.exec(&mut db).await.map_err(map_toasty_error)?;
        Ok(())
    }

    pub async fn authenticate_bearer(&self, token: &str) -> Result<User, ApiError> {
        match self.verify_access_token(token) {
            Ok(claims) => {
                let user_id = Uuid::parse_str(&claims.sub).map_err(|_| ApiError::Unauthorized)?;
                self.get_user(user_id).await
            }
            Err(ApiError::Unauthorized) => self.authenticate_api_token(token).await,
            Err(error) => Err(error),
        }
    }

    pub async fn list_users(&self) -> Result<Vec<User>, ApiError> {
        let mut db = self.db.handle()?;
        let records = Query::<List<UserRecord>>::all()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        let mut users = Vec::with_capacity(records.len());
        for record in records {
            users.push(self.user_from_record(record).await?);
        }
        Ok(users)
    }

    pub async fn create_api_token(
        &self,
        input: CreateApiTokenInput,
    ) -> Result<CreateApiTokenOutput, ApiError> {
        let raw_token = generate_api_token();
        let token_hash = hash_api_token(&raw_token);
        let token_prefix = token_prefix(&raw_token);
        let now = Utc::now().to_rfc3339();

        let mut db = self.db.handle()?;
        let record = toasty::create!(UserApiTokenRecord {
            id: Uuid::now_v7(),
            user_id: input.user.id,
            name: input.name,
            token_hash,
            token_prefix,
            created_at: now,
            revoked_at: None,
            last_used_at: None,
        })
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;

        Ok(CreateApiTokenOutput {
            token: raw_token,
            api_token: record,
        })
    }

    pub async fn list_api_tokens(&self, user: &User) -> Result<Vec<UserApiTokenRecord>, ApiError> {
        let mut db = self.db.handle()?;
        let mut records = Query::<List<UserApiTokenRecord>>::filter(
            UserApiTokenRecord::fields().user_id().eq(user.id),
        )
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;

        records.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(records)
    }

    pub async fn revoke_api_token(
        &self,
        user: &User,
        token_id: Uuid,
    ) -> Result<UserApiTokenRecord, ApiError> {
        let mut db = self.db.handle()?;
        let record = Query::<List<UserApiTokenRecord>>::filter(
            UserApiTokenRecord::fields().id().eq(token_id),
        )
        .first()
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?
        .ok_or(ApiError::NotFound("api token"))?;
        if record.user_id != user.id {
            return Err(ApiError::NotFound("api token"));
        }

        let revoked_at = record
            .revoked_at
            .clone()
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        let mut update_token =
            Update::<List<UserApiTokenRecord>>::new(Query::<List<UserApiTokenRecord>>::filter(
                UserApiTokenRecord::fields().id().eq(token_id),
            ));
        update_token.set(6, revoked_at);
        update_token.set_returning_none();
        update_token.exec(&mut db).await.map_err(map_toasty_error)?;

        let record = Query::<List<UserApiTokenRecord>>::filter(
            UserApiTokenRecord::fields().id().eq(token_id),
        )
        .first()
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?
        .ok_or(ApiError::NotFound("api token"))?;
        Ok(record)
    }

    async fn issue_refresh_token(
        &self,
        executor: &mut dyn Executor,
        user_id: Uuid,
    ) -> Result<RefreshTokenIssue, ApiError> {
        let id = Uuid::now_v7();
        let now = Utc::now();
        let expires_at = now + Duration::seconds(self.config.refresh_token_ttl_seconds as i64);
        let record = toasty::create!(UserRefreshTokenRecord {
            id,
            user_id,
            created_at: now.to_rfc3339(),
            expires_at: expires_at.to_rfc3339(),
            revoked_at: None,
        })
        .exec(executor)
        .await
        .map_err(map_toasty_error)?;
        let raw_token = self.sign_refresh_token(id, user_id, now, expires_at)?;
        Ok(RefreshTokenIssue {
            token: raw_token,
            record,
        })
    }

    async fn active_refresh_token_record(
        &self,
        token: &str,
    ) -> Result<UserRefreshTokenRecord, ApiError> {
        let claims = self.verify_refresh_token(token)?;
        let token_id = Uuid::parse_str(&claims.jti).map_err(|_| ApiError::Unauthorized)?;
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| ApiError::Unauthorized)?;
        let mut db = self.db.handle()?;
        let record = Query::<List<UserRefreshTokenRecord>>::filter(
            UserRefreshTokenRecord::fields().id().eq(token_id),
        )
        .first()
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?
        .ok_or(ApiError::Unauthorized)?;
        if record.user_id != user_id {
            return Err(ApiError::Unauthorized);
        }

        if record.revoked_at.is_some() {
            return Err(ApiError::Unauthorized);
        }
        if parse_rfc3339(&record.expires_at)? <= Utc::now() {
            return Err(ApiError::Unauthorized);
        }
        Ok(record)
    }

    async fn get_user(&self, id: Uuid) -> Result<User, ApiError> {
        let record = self.get_user_record(id).await?;
        self.user_from_record(record).await
    }

    async fn get_user_record(&self, id: Uuid) -> Result<UserRecord, ApiError> {
        let mut db = self.db.handle()?;
        let record = Query::<List<UserRecord>>::filter(UserRecord::fields().id().eq(id))
            .first()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        record.ok_or(ApiError::Unauthorized)
    }

    async fn ensure_workspace_exists(&self, id: Uuid) -> Result<(), ApiError> {
        let mut db = self.db.handle()?;
        let record = Query::<List<Workspace>>::filter(Workspace::fields().id().eq(id))
            .first()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        record.map(|_| ()).ok_or(ApiError::NotFound("workspace"))
    }

    async fn user_from_record(&self, record: UserRecord) -> Result<User, ApiError> {
        let user_id = record.id;
        let mut user = User::from(record);
        if user.role == Role::SuperAdmin {
            return Ok(user);
        }

        let mut db = self.db.handle()?;
        let roles = Query::<List<WorkspaceUserRoleRecord>>::filter(
            WorkspaceUserRoleRecord::fields().user_id().eq(user_id),
        )
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        user.workspace_roles = roles.into_iter().map(Into::into).collect();
        Ok(user)
    }

    async fn authenticate_api_token(&self, token: &str) -> Result<User, ApiError> {
        if !token.starts_with("wara_") {
            return Err(ApiError::Unauthorized);
        }

        let token_hash = hash_api_token(token);
        let mut db = self.db.handle()?;
        let record = Query::<List<UserApiTokenRecord>>::filter(
            UserApiTokenRecord::fields().token_hash().eq(token_hash),
        )
        .first()
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?
        .ok_or(ApiError::Unauthorized)?;
        if record.revoked_at.is_some() {
            return Err(ApiError::Unauthorized);
        }

        let user_record = self.get_user_record(record.user_id).await?;
        if user_record.status != UserStatus::Active.as_str() {
            return Err(ApiError::Unauthorized);
        }

        let mut update_token =
            Update::<List<UserApiTokenRecord>>::new(Query::<List<UserApiTokenRecord>>::filter(
                UserApiTokenRecord::fields().id().eq(record.id),
            ));
        update_token.set(7, Utc::now().to_rfc3339());
        update_token.set_returning_none();
        update_token.exec(&mut db).await.map_err(map_toasty_error)?;

        self.user_from_record(user_record).await
    }

    async fn find_user_by_email(&self, email: &str) -> Result<UserRecord, ApiError> {
        let mut db = self.db.handle()?;
        let record = Query::<List<UserRecord>>::filter(UserRecord::fields().email().eq(email))
            .first()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        record.ok_or(ApiError::Unauthorized)
    }

    async fn find_open_invite(&self, token_hash: &str) -> Result<UserInviteRecord, ApiError> {
        let mut db = self.db.handle()?;
        let record = Query::<List<UserInviteRecord>>::filter(
            UserInviteRecord::fields().token_hash().eq(token_hash),
        )
        .first()
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        let invite = record.ok_or(ApiError::Unauthorized)?;
        if !invite.accepted_at.is_empty() {
            return Err(ApiError::Unauthorized);
        }
        Ok(invite)
    }

    fn sign_access_token(&self, user: &User, refresh_token_id: Uuid) -> Result<String, ApiError> {
        let now = get_current_timestamp();
        let claims = Claims {
            jti: Uuid::now_v7().to_string(),
            ret: refresh_token_id.to_string(),
            sub: user.id.to_string(),
            email: user.email.clone(),
            role: user.role.as_str().to_string(),
            iss: self.config.jwt_issuer.clone(),
            aud: self.config.jwt_audience.clone(),
            iat: now,
            exp: now + self.config.jwt_access_token_ttl_seconds,
        };
        let key = EncodingKey::from_rsa_pem(self.config.jwt_private_key_pem.as_bytes())
            .map_err(|error| ApiError::Internal(format!("invalid JWT private key: {error}")))?;
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.config.jwt_active_key_id.clone());
        encode(&header, &claims, &key)
            .map_err(|error| ApiError::Internal(format!("failed to sign JWT: {error}")))
    }

    fn sign_refresh_token(
        &self,
        refresh_token_id: Uuid,
        user_id: Uuid,
        issued_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<String, ApiError> {
        let claims = RefreshClaims {
            jti: refresh_token_id.to_string(),
            sub: user_id.to_string(),
            typ: "refresh".to_string(),
            iss: self.config.jwt_issuer.clone(),
            aud: self.config.jwt_audience.clone(),
            iat: issued_at.timestamp() as u64,
            exp: expires_at.timestamp() as u64,
        };
        let key = EncodingKey::from_rsa_pem(self.config.jwt_private_key_pem.as_bytes())
            .map_err(|error| ApiError::Internal(format!("invalid JWT private key: {error}")))?;
        encode(&Header::new(Algorithm::RS256), &claims, &key)
            .map_err(|error| ApiError::Internal(format!("failed to sign refresh JWT: {error}")))
    }

    fn verify_access_token(&self, token: &str) -> Result<Claims, ApiError> {
        let header = decode_header(token).map_err(|_| ApiError::Unauthorized)?;
        let key_id = header.kid.ok_or(ApiError::Unauthorized)?;
        let public_key = self
            .config
            .jwt_public_key_for_id(&key_id)
            .ok_or(ApiError::Unauthorized)?;
        let key = DecodingKey::from_rsa_pem(public_key.as_bytes()).map_err(|error| {
            ApiError::Internal(format!("invalid JWT public key for {key_id}: {error}"))
        })?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.config.jwt_audience.as_str()]);
        validation.set_issuer(&[self.config.jwt_issuer.as_str()]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        decode::<Claims>(token, &key, &validation)
            .map(|data| data.claims)
            .map_err(|_| ApiError::Unauthorized)
    }

    fn verify_refresh_token(&self, token: &str) -> Result<RefreshClaims, ApiError> {
        let key = DecodingKey::from_rsa_pem(self.config.jwt_public_key_pem.as_bytes())
            .map_err(|error| ApiError::Internal(format!("invalid JWT public key: {error}")))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.config.jwt_audience.as_str()]);
        validation.set_issuer(&[self.config.jwt_issuer.as_str()]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub", "jti"]);
        let claims = decode::<RefreshClaims>(token, &key, &validation)
            .map(|data| data.claims)
            .map_err(|_| ApiError::Unauthorized)?;
        if claims.typ != "refresh" {
            return Err(ApiError::Unauthorized);
        }
        Ok(claims)
    }
}

pub fn ensure_workspace_access(user: &User, workspace_id: Uuid) -> Result<(), ApiError> {
    if can_access_workspace(user, workspace_id) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

pub fn ensure_super_admin(user: &User) -> Result<(), ApiError> {
    if user.role == Role::SuperAdmin {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

fn can_access_workspace(user: &User, workspace_id: Uuid) -> bool {
    user.role == Role::SuperAdmin
        || user
            .workspace_roles
            .iter()
            .any(|role| role.workspace_id == workspace_id)
}

fn can_administer_workspace(user: &User, workspace_id: Uuid) -> bool {
    user.role == Role::SuperAdmin
        || user
            .workspace_roles
            .iter()
            .any(|role| role.workspace_id == workspace_id && role.role == Role::Admin)
}

#[derive(Debug, Clone)]
pub struct CurrentUser(pub User);

impl<S> FromRequestParts<S> for CurrentUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = AppState::from_ref(state);
        let token = parts
            .headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;

        let user = AuthService::new(state.db, state.config)
            .authenticate_bearer(token)
            .await?;
        Ok(CurrentUser(user))
    }
}

#[derive(Debug, Clone)]
pub struct AdminUser(pub User);

impl<S> FromRequestParts<S> for AdminUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let CurrentUser(user) = CurrentUser::from_request_parts(parts, state).await?;
        if user.role != Role::SuperAdmin {
            return Err(ApiError::Forbidden);
        }
        Ok(AdminUser(user))
    }
}

pub fn unauthorized_status() -> StatusCode {
    StatusCode::UNAUTHORIZED
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| ApiError::Internal(format!("failed to hash password: {error}")))
}

fn verify_password(password: &str, password_hash: &str) -> Result<(), ApiError> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|_| ApiError::Unauthorized)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError::Unauthorized)
}

fn generate_invite_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn generate_api_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("wara_{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn hash_invite_token(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}

fn hash_api_token(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}

fn token_prefix(token: &str) -> String {
    token.chars().take(12).collect()
}

fn invite_link(app_base_url: &str, token: &str) -> String {
    format!(
        "{}/accept-invite?token={token}",
        app_base_url.trim_end_matches('/')
    )
}

fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>, ApiError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|error| ApiError::Internal(format!("invalid timestamp: {error}")))
}

fn map_toasty_error(error: toasty::Error) -> ApiError {
    ApiError::Internal(format!("database operation failed: {error}"))
}
