use toasty::stmt::{List, Query};
use uuid::Uuid;

use crate::{
    errors::api::ApiError,
    libs::{db::Database, docker::DeployKind},
    models::services::{AppService, AppServiceRecord},
    services::workspaces::WorkspaceService,
};

#[derive(Debug, Clone)]
pub struct CreateAppServiceInput {
    pub workspace_id: Uuid,
    pub environment_id: Uuid,
    pub name: String,
    pub deploy_kind: DeployKind,
    pub image: Option<String>,
    pub compose_file: Option<String>,
    pub dockerfile: Option<String>,
    pub internal_port: Option<u16>,
}

#[derive(Clone)]
pub struct AppServiceService {
    db: Database,
}

impl AppServiceService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list_services(&self, workspace_id: Uuid) -> Result<Vec<AppService>, ApiError> {
        let mut db = self.db.handle()?;
        let records = Query::<List<AppServiceRecord>>::filter(
            AppServiceRecord::fields().workspace_id().eq(workspace_id),
        )
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(records.into_iter().map(AppService::from).collect())
    }

    pub async fn create_service(
        &self,
        input: CreateAppServiceInput,
    ) -> Result<AppService, ApiError> {
        WorkspaceService::new(self.db.clone())
            .get_workspace(input.workspace_id)
            .await?;
        let environments = WorkspaceService::new(self.db.clone())
            .list_environments(input.workspace_id)
            .await?;
        if !environments
            .iter()
            .any(|environment| environment.id == input.environment_id)
        {
            return Err(ApiError::NotFound("environment"));
        }

        let mut db = self.db.handle()?;
        let record = toasty::create!(AppServiceRecord {
            id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            environment_id: input.environment_id,
            name: input.name,
            deploy_kind: input.deploy_kind.as_str().to_string(),
            image: input.image,
            compose_file: input.compose_file,
            dockerfile: input.dockerfile,
            internal_port: input.internal_port,
        })
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(AppService::from(record))
    }

    pub async fn get_service(&self, id: Uuid) -> Result<AppService, ApiError> {
        let mut db = self.db.handle()?;
        let record =
            Query::<List<AppServiceRecord>>::filter(AppServiceRecord::fields().id().eq(id))
                .first()
                .exec(&mut db)
                .await
                .map_err(map_toasty_error)?;
        record
            .map(AppService::from)
            .ok_or(ApiError::NotFound("service"))
    }
}

fn map_toasty_error(error: toasty::Error) -> ApiError {
    ApiError::Internal(format!("database operation failed: {error}"))
}
