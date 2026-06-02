use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct DockerCredential {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub registry: String,
    pub username: String,
    #[serde(skip_serializing)]
    #[schema(ignore)]
    pub encrypted_password: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct EnvVar {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub service_id: Option<Uuid>,
    pub key: String,
    #[serde(skip_serializing)]
    #[schema(ignore)]
    pub encrypted_value: String,
}

#[derive(Debug, Clone, toasty::Model)]
pub struct DockerCredentialRecord {
    #[key]
    pub id: Uuid,
    #[index]
    pub workspace_id: Uuid,
    pub registry: String,
    pub username: String,
    pub encrypted_password: String,
}

#[derive(Debug, Clone, toasty::Model)]
pub struct EnvVarRecord {
    #[key]
    pub id: Uuid,
    #[index]
    pub workspace_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub service_id: Option<Uuid>,
    pub key: String,
    pub encrypted_value: String,
}

impl From<DockerCredentialRecord> for DockerCredential {
    fn from(record: DockerCredentialRecord) -> Self {
        Self {
            id: record.id,
            workspace_id: record.workspace_id,
            registry: record.registry,
            username: record.username,
            encrypted_password: record.encrypted_password,
        }
    }
}

impl From<EnvVarRecord> for EnvVar {
    fn from(record: EnvVarRecord) -> Self {
        Self {
            id: record.id,
            workspace_id: record.workspace_id,
            environment_id: record.environment_id,
            service_id: record.service_id,
            key: record.key,
            encrypted_value: record.encrypted_value,
        }
    }
}
