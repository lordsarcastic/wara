use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub role: Role,
    pub status: UserStatus,
    pub workspace_roles: Vec<WorkspaceRole>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    SuperAdmin,
    Admin,
    Operator,
    Viewer,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, PartialEq, Eq)]
pub struct WorkspaceRole {
    pub workspace_id: Uuid,
    pub role: Role,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Invited,
    Active,
}

impl UserStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Invited => "invited",
            Self::Active => "active",
        }
    }
}

impl From<&str> for UserStatus {
    fn from(value: &str) -> Self {
        match value {
            "active" => Self::Active,
            _ => Self::Invited,
        }
    }
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SuperAdmin => "super_admin",
            Self::Admin => "admin",
            Self::Operator => "operator",
            Self::Viewer => "viewer",
        }
    }
}

impl From<&str> for Role {
    fn from(value: &str) -> Self {
        match value {
            "super_admin" => Self::SuperAdmin,
            "admin" => Self::Admin,
            "operator" => Self::Operator,
            _ => Self::Viewer,
        }
    }
}

#[derive(Debug, Clone, toasty::Model)]
pub struct UserRecord {
    #[key]
    pub id: Uuid,
    #[unique]
    pub email: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, toasty::Model)]
pub struct UserInviteRecord {
    #[key]
    pub id: Uuid,
    #[index]
    pub user_id: Uuid,
    #[unique]
    pub token_hash: String,
    pub expires_at: String,
    pub accepted_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, toasty::Model)]
pub struct WorkspaceUserRoleRecord {
    #[key]
    pub id: Uuid,
    #[index]
    pub user_id: Uuid,
    #[index]
    pub workspace_id: Uuid,
    pub role: String,
}

#[derive(Debug, Clone, toasty::Model)]
pub struct UserApiTokenRecord {
    #[key]
    pub id: Uuid,
    #[index]
    pub user_id: Uuid,
    pub name: String,
    #[unique]
    pub token_hash: String,
    pub token_prefix: String,
    pub created_at: String,
    pub revoked_at: Option<String>,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, toasty::Model)]
pub struct UserRefreshTokenRecord {
    #[key]
    pub id: Uuid,
    #[index]
    pub user_id: Uuid,
    #[unique]
    pub token_hash: String,
    pub token_prefix: String,
    pub created_at: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub replaced_by_token_id: Option<String>,
    pub last_used_at: Option<String>,
}

impl From<UserRecord> for User {
    fn from(record: UserRecord) -> Self {
        Self {
            id: record.id,
            email: record.email,
            name: record.name,
            role: Role::from(record.role.as_str()),
            status: UserStatus::from(record.status.as_str()),
            workspace_roles: Vec::new(),
        }
    }
}

impl From<WorkspaceUserRoleRecord> for WorkspaceRole {
    fn from(record: WorkspaceUserRoleRecord) -> Self {
        Self {
            workspace_id: record.workspace_id,
            role: Role::from(record.role.as_str()),
        }
    }
}
