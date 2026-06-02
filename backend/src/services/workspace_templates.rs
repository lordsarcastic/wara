use toasty::stmt::{List, Query};
use uuid::Uuid;

use crate::{
    errors::ApiError,
    libs::db::Database,
    models::{
        templates::{WorkspaceTemplate, WorkspaceTemplateRecord},
        workspaces::Workspace,
    },
    services::{templates::SecretCopyMode, workspaces::WorkspaceService},
};

#[derive(Debug, Clone)]
pub struct CreateTemplateInput {
    pub workspace_id: Uuid,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateWorkspacesFromTemplateInput {
    pub template_id: Uuid,
    pub names: Vec<String>,
    pub secret_copy_mode: SecretCopyMode,
}

#[derive(Clone)]
pub struct WorkspaceTemplateService {
    db: Database,
}

impl WorkspaceTemplateService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list_workspace_templates(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceTemplate>, ApiError> {
        let mut db = self.db.handle()?;
        let records = Query::<List<WorkspaceTemplateRecord>>::filter(
            WorkspaceTemplateRecord::fields()
                .source_workspace_id()
                .eq(workspace_id),
        )
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(records.into_iter().map(WorkspaceTemplate::from).collect())
    }

    pub async fn create_template(
        &self,
        input: CreateTemplateInput,
    ) -> Result<WorkspaceTemplate, ApiError> {
        WorkspaceService::new(self.db.clone())
            .get_workspace(input.workspace_id)
            .await?;
        let mut db = self.db.handle()?;
        let record = toasty::create!(WorkspaceTemplateRecord {
            id: Uuid::now_v7(),
            source_workspace_id: input.workspace_id,
            name: input.name,
            description: input.description,
        })
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(WorkspaceTemplate::from(record))
    }

    pub async fn create_workspaces_from_template(
        &self,
        input: CreateWorkspacesFromTemplateInput,
    ) -> Result<Vec<Workspace>, ApiError> {
        let template = self.get_template(input.template_id).await?;
        tracing::info!(template_id = %template.id, secret_copy_mode = ?input.secret_copy_mode, "creating workspaces from template");
        let workspace_service = WorkspaceService::new(self.db.clone());
        let mut workspaces = Vec::with_capacity(input.names.len());
        for name in input.names {
            workspaces.push(
                workspace_service
                    .create_workspace(name, template.description.clone())
                    .await?,
            );
        }
        Ok(workspaces)
    }

    pub async fn get_template(&self, id: Uuid) -> Result<WorkspaceTemplate, ApiError> {
        let mut db = self.db.handle()?;
        let record = Query::<List<WorkspaceTemplateRecord>>::filter(
            WorkspaceTemplateRecord::fields().id().eq(id),
        )
        .first()
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        record
            .map(WorkspaceTemplate::from)
            .ok_or(ApiError::NotFound("template"))
    }
}

fn map_toasty_error(error: toasty::Error) -> ApiError {
    ApiError::Internal(format!("database operation failed: {error}"))
}
