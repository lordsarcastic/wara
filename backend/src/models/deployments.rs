use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Deployment {
    pub id: Uuid,
    pub service_id: Uuid,
    pub status: DeploymentStatus,
    pub workflow_id: String,
    pub output: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
}

impl DeploymentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

impl From<&str> for DeploymentStatus {
    fn from(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            _ => Self::Queued,
        }
    }
}

#[derive(Debug, Clone, toasty::Model)]
pub struct DeploymentRecord {
    #[key]
    pub id: Uuid,
    #[index]
    pub service_id: Uuid,
    pub status: String,
    pub workflow_id: String,
    pub output: String,
    pub created_at: String,
}

impl From<DeploymentRecord> for Deployment {
    fn from(record: DeploymentRecord) -> Self {
        Self {
            id: record.id,
            service_id: record.service_id,
            status: DeploymentStatus::from(record.status.as_str()),
            workflow_id: record.workflow_id,
            output: record.output,
            created_at: record.created_at.parse().unwrap_or_else(|_| Utc::now()),
        }
    }
}
