use crate::libs::config::Config;
use crate::{
    errors::ApiError,
    models::{
        credentials::{DockerCredentialRecord, EnvVarRecord},
        deployments::DeploymentRecord,
        domains::DomainRecord,
        environments::Environment,
        servers::ServerRecord,
        services::AppServiceRecord,
        templates::WorkspaceTemplateRecord,
        users::{UserApiTokenRecord, UserInviteRecord, UserRecord, WorkspaceUserRoleRecord},
        workspaces::Workspace,
    },
};

#[derive(Clone)]
pub struct Database {
    pub url: String,
    toasty: Option<toasty::Db>,
}

/// Build a connected Toasty `Db` with every product model registered.
///
/// Shared by [`connect`] and the `wara-migrate` binary so the model set has a
/// single source of truth. This does not create or migrate the schema; schema is
/// managed by the `wara-migrate` migration commands (or, for local iteration, by
/// `WARA_DB_PUSH_SCHEMA`).
pub async fn build_toasty(config: &Config) -> anyhow::Result<toasty::Db> {
    let db = toasty::Db::builder()
        .models(toasty::models!(
            Workspace,
            Environment,
            ServerRecord,
            AppServiceRecord,
            DockerCredentialRecord,
            EnvVarRecord,
            DomainRecord,
            DeploymentRecord,
            WorkspaceTemplateRecord,
            UserRecord,
            UserInviteRecord,
            UserApiTokenRecord,
            WorkspaceUserRoleRecord
        ))
        .connect(&config.database_url)
        .await?;
    Ok(db)
}

pub async fn connect(config: &Config) -> anyhow::Result<Database> {
    let db = build_toasty(config).await?;

    // Schema is applied out of band by `wara-migrate migration apply`.
    // `WARA_DB_PUSH_SCHEMA` (default false) remains a local-only escape hatch that
    // lets Toasty regenerate the schema directly while iterating on models.
    if config.db_push_schema {
        db.push_schema().await?;
    }

    tracing::info!("Toasty PostgreSQL database initialized");
    Ok(Database {
        url: config.database_url.clone(),
        toasty: Some(db),
    })
}

impl Database {
    pub fn unavailable_for_tests() -> Self {
        Self {
            url: "postgres://test-unavailable".to_string(),
            toasty: None,
        }
    }

    pub fn handle(&self) -> Result<toasty::Db, ApiError> {
        self.toasty
            .clone()
            .ok_or_else(|| ApiError::Internal("database handle is unavailable".to_string()))
    }
}
