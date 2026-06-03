use std::sync::Arc;

use chrono::Utc;
use toasty::stmt::{List, Query};
use uuid::Uuid;

use crate::{
    errors::ApiError,
    libs::deploy_executor::{DeployExecutor, DeployRun, PreviewExecutor},
    libs::docker::{
        DeployKind, DockerCommandConfig, RemoteCommand, compose_deploy_commands,
        dockerfile_deploy_commands, image_deploy_commands, restart_command,
        validate_image_reference, validate_service_name,
    },
    libs::{config::Config, db::Database},
    models::{
        deployments::{Deployment, DeploymentRecord, DeploymentStatus},
        services::AppService,
    },
    services::app_services::AppServiceService,
};

/// Build the typed, injection-safe command sequence for a service's deploy. User
/// inputs are validated up front and only ever flow into commands as quoted args.
pub fn planned_commands(
    service: &AppService,
    config: &Config,
) -> Result<Vec<RemoteCommand>, ApiError> {
    validate_service_name(&service.name).map_err(ApiError::BadRequest)?;

    let docker_config = DockerCommandConfig::new(
        config.remote_services_root.clone(),
        config.dockerfile_context_dir.clone(),
    );

    let commands = match service.deploy_kind {
        DeployKind::DockerImage => {
            let image = service.image.as_deref().unwrap_or("missing-image");
            validate_image_reference(image).map_err(ApiError::BadRequest)?;
            image_deploy_commands(image, &service.name)
        }
        DeployKind::DockerCompose => compose_deploy_commands(&service.name, &docker_config),
        DeployKind::Dockerfile => dockerfile_deploy_commands(&service.name, &docker_config),
    };
    Ok(commands)
}

#[derive(Clone)]
pub struct DeploymentService {
    db: Database,
    config: Config,
    executor: Arc<dyn DeployExecutor>,
}

impl DeploymentService {
    pub fn new(db: Database, config: Config) -> Self {
        Self {
            db,
            config,
            executor: Arc::new(PreviewExecutor),
        }
    }

    /// Construct with a specific executor (used by tests to inject a mock).
    pub fn with_executor(db: Database, config: Config, executor: Arc<dyn DeployExecutor>) -> Self {
        Self {
            db,
            config,
            executor,
        }
    }

    pub async fn list_deployments(&self, service_id: Uuid) -> Result<Vec<Deployment>, ApiError> {
        let mut db = self.db.handle()?;
        let records = Query::<List<DeploymentRecord>>::filter(
            DeploymentRecord::fields().service_id().eq(service_id),
        )
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(records.into_iter().map(Deployment::from).collect())
    }

    pub async fn get_deployment(&self, id: Uuid) -> Result<Deployment, ApiError> {
        let mut db = self.db.handle()?;
        let record =
            Query::<List<DeploymentRecord>>::filter(DeploymentRecord::fields().id().eq(id))
                .first()
                .exec(&mut db)
                .await
                .map_err(map_toasty_error)?;
        record
            .map(Deployment::from)
            .ok_or(ApiError::NotFound("deployment"))
    }

    pub async fn trigger_deploy(&self, service_id: Uuid) -> Result<Deployment, ApiError> {
        let service = AppServiceService::new(self.db.clone())
            .get_service(service_id)
            .await?;
        let commands = planned_commands(&service, &self.config)?;
        self.run_and_record(service_id, &commands).await
    }

    pub async fn restart_service(&self, service_id: Uuid) -> Result<Deployment, ApiError> {
        let service = AppServiceService::new(self.db.clone())
            .get_service(service_id)
            .await?;
        validate_service_name(&service.name).map_err(ApiError::BadRequest)?;
        let commands = vec![restart_command(&service.name)];
        self.run_and_record(service_id, &commands).await
    }

    /// Run a service's commands through the executor and persist the captured
    /// run as a deployment record.
    async fn run_and_record(
        &self,
        service_id: Uuid,
        commands: &[RemoteCommand],
    ) -> Result<Deployment, ApiError> {
        let run = self
            .executor
            .run(commands)
            .await
            .map_err(|error| ApiError::Internal(format!("deploy execution failed: {error}")))?;
        let status = deploy_status(&run);
        // No secrets flow into commands yet; the secret list grows when credential
        // and env-var binding land. Output is already injection-safe.
        self.create_deployment(service_id, status, run.to_output(&[]))
            .await
    }

    pub async fn service_exists(&self, service_id: Uuid) -> Result<(), ApiError> {
        AppServiceService::new(self.db.clone())
            .get_service(service_id)
            .await?;
        Ok(())
    }

    async fn create_deployment(
        &self,
        service_id: Uuid,
        status: DeploymentStatus,
        output: String,
    ) -> Result<Deployment, ApiError> {
        let mut db = self.db.handle()?;
        let record = toasty::create!(DeploymentRecord {
            id: Uuid::now_v7(),
            service_id,
            status: status.as_str().to_string(),
            workflow_id: format!("workflow-{}", Uuid::now_v7().simple()),
            output,
            created_at: Utc::now().to_rfc3339(),
        })
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(Deployment::from(record))
    }
}

/// Map an executor run to a deployment status. A preview (not executed) stays
/// `Queued` (awaiting the real SSH transport); a real run reports success/failure.
fn deploy_status(run: &DeployRun) -> DeploymentStatus {
    if !run.executed {
        DeploymentStatus::Queued
    } else if run.success {
        DeploymentStatus::Succeeded
    } else {
        DeploymentStatus::Failed
    }
}

fn map_toasty_error(error: toasty::Error) -> ApiError {
    ApiError::Internal(format!("database operation failed: {error}"))
}
