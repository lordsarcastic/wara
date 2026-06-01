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
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode, get_current_timestamp,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use toasty::stmt::{List, Query, Update};
use uuid::Uuid;

use crate::{
    entities::users::{Role, User, UserInviteRecord, UserRecord, UserStatus},
    errors::ApiError,
    libs::{config::Config, db::Database},
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
    pub user: User,
}

#[derive(Debug, Clone)]
pub struct InviteUserInput {
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

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    email: String,
    role: String,
    iss: String,
    aud: String,
    iat: u64,
    exp: u64,
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
            role: Role::Admin.as_str().to_string(),
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
        let user = User::from(user_record);
        let token = self.sign_access_token(&user)?;
        Ok(LoginOutput { token, user })
    }

    pub async fn invite_user(&self, input: InviteUserInput) -> Result<InviteUserOutput, ApiError> {
        if self.find_user_by_email(&input.email).await.is_ok() {
            return Err(ApiError::BadRequest("user already exists".to_string()));
        }

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
            role: input.role.as_str().to_string(),
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
        tx.commit().await.map_err(map_toasty_error)?;

        Ok(InviteUserOutput {
            user: User::from(user_record),
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
        let token = self.sign_access_token(&user)?;
        Ok(LoginOutput { token, user })
    }

    pub async fn authenticate_bearer(&self, token: &str) -> Result<User, ApiError> {
        let claims = self.verify_access_token(token)?;
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| ApiError::Unauthorized)?;
        self.get_user(user_id).await
    }

    pub async fn list_users(&self) -> Result<Vec<User>, ApiError> {
        let mut db = self.db.handle()?;
        let records = Query::<List<UserRecord>>::all()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        Ok(records.into_iter().map(User::from).collect())
    }

    async fn get_user(&self, id: Uuid) -> Result<User, ApiError> {
        self.get_user_record(id).await.map(User::from)
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

    fn sign_access_token(&self, user: &User) -> Result<String, ApiError> {
        let now = get_current_timestamp();
        let claims = Claims {
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
        encode(&Header::new(Algorithm::RS256), &claims, &key)
            .map_err(|error| ApiError::Internal(format!("failed to sign JWT: {error}")))
    }

    fn verify_access_token(&self, token: &str) -> Result<Claims, ApiError> {
        let key = DecodingKey::from_rsa_pem(self.config.jwt_public_key_pem.as_bytes())
            .map_err(|error| ApiError::Internal(format!("invalid JWT public key: {error}")))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.config.jwt_audience.as_str()]);
        validation.set_issuer(&[self.config.jwt_issuer.as_str()]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        decode::<Claims>(token, &key, &validation)
            .map(|data| data.claims)
            .map_err(|_| ApiError::Unauthorized)
    }
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
        if user.role != Role::Admin {
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

fn hash_invite_token(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
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
