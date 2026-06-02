use chrono::Utc;
use toasty::stmt::{List, Query};
use uuid::Uuid;

use crate::{
    errors::ApiError,
    libs::docker::{
        DeployKind, DockerCommandConfig, compose_deploy_commands, dockerfile_deploy_commands,
        image_deploy_commands,
    },
    libs::{config::Config, db::Database},
    models::{
        deployments::{Deployment, DeploymentRecord, DeploymentStatus},
        services::AppService,
    },
    services::app_services::AppServiceService,
};

pub fn planned_commands(service: &AppService, config: &Config) -> Vec<String> {
    let docker_config = DockerCommandConfig::new(
        config.remote_services_root.clone(),
        config.dockerfile_context_dir.clone(),
    );

    match service.deploy_kind {
        DeployKind::DockerImage => image_deploy_commands(
            service.image.as_deref().unwrap_or("missing-image"),
            &service.name,
        ),
        DeployKind::DockerCompose => compose_deploy_commands(&service.name, &docker_config),
        DeployKind::Dockerfile => dockerfile_deploy_commands(&service.name, &docker_config),
    }
}

#[derive(Clone)]
pub struct DeploymentService {
    db: Database,
    config: Config,
}

impl DeploymentService {
    pub fn new(db: Database, config: Config) -> Self {
        Self { db, config }
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
        let commands = planned_commands(&service, &self.config);
        self.create_deployment(service_id, commands.join("\n"))
            .await
    }

    pub async fn restart_service(&self, service_id: Uuid) -> Result<Deployment, ApiError> {
        let service = AppServiceService::new(self.db.clone())
            .get_service(service_id)
            .await?;
        self.create_deployment(service_id, format!("docker restart wara-{}", service.name))
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
        output: String,
    ) -> Result<Deployment, ApiError> {
        let mut db = self.db.handle()?;
        let record = toasty::create!(DeploymentRecord {
            id: Uuid::now_v7(),
            service_id,
            status: DeploymentStatus::Queued.as_str().to_string(),
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

fn map_toasty_error(error: toasty::Error) -> ApiError {
    ApiError::Internal(format!("database operation failed: {error}"))
}
