use toasty::stmt::{List, Query};
use uuid::Uuid;

use crate::{
    errors::api::ApiError,
    libs::db::Database,
    models::{
        environments::Environment,
        users::{Role, User},
        workspaces::Workspace,
    },
};

#[derive(Clone)]
pub struct WorkspaceService {
    db: Database,
}

impl WorkspaceService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list_workspaces(&self) -> Result<Vec<Workspace>, ApiError> {
        let mut db = self.db.handle()?;
        Query::<List<Workspace>>::all()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)
    }

    pub async fn list_accessible_workspaces(
        &self,
        user: &User,
    ) -> Result<Vec<Workspace>, ApiError> {
        if user.role == Role::SuperAdmin {
            return self.list_workspaces().await;
        }

        let mut workspaces = Vec::new();
        for workspace_role in &user.workspace_roles {
            match self.get_workspace(workspace_role.workspace_id).await {
                Ok(workspace) => workspaces.push(workspace),
                Err(ApiError::NotFound("workspace")) => {}
                Err(error) => return Err(error),
            }
        }
        workspaces.sort_by_key(|workspace| workspace.id);
        workspaces.dedup_by_key(|workspace| workspace.id);
        Ok(workspaces)
    }

    pub async fn create_workspace(
        &self,
        name: String,
        description: Option<String>,
    ) -> Result<Workspace, ApiError> {
        let mut db = self.db.handle()?;
        let mut tx = db.transaction().await.map_err(map_toasty_error)?;

        let workspace = toasty::create!(Workspace {
            id: Uuid::now_v7(),
            name,
            description,
        })
        .exec(&mut tx)
        .await
        .map_err(map_toasty_error)?;

        toasty::create!(Environment {
            id: Uuid::now_v7(),
            workspace_id: workspace.id,
            name: "production",
        })
        .exec(&mut tx)
        .await
        .map_err(map_toasty_error)?;

        tx.commit().await.map_err(map_toasty_error)?;
        Ok(workspace)
    }

    pub async fn get_workspace(&self, id: Uuid) -> Result<Workspace, ApiError> {
        let mut db = self.db.handle()?;
        let workspace = Query::<List<Workspace>>::filter(Workspace::fields().id().eq(id))
            .first()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        workspace.ok_or(ApiError::NotFound("workspace"))
    }

    pub async fn list_environments(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<Environment>, ApiError> {
        let mut db = self.db.handle()?;
        Query::<List<Environment>>::filter(Environment::fields().workspace_id().eq(workspace_id))
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)
    }

    pub async fn create_environment(
        &self,
        workspace_id: Uuid,
        name: String,
    ) -> Result<Environment, ApiError> {
        self.get_workspace(workspace_id).await?;
        let mut db = self.db.handle()?;
        toasty::create!(Environment {
            id: Uuid::now_v7(),
            workspace_id,
            name,
        })
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)
    }
}

fn map_toasty_error(error: toasty::Error) -> ApiError {
    ApiError::Internal(format!("database operation failed: {error}"))
}
