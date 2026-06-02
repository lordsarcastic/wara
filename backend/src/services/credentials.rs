use toasty::stmt::{List, Query};
use uuid::Uuid;

use crate::{
    errors::ApiError,
    libs::{crypto, db::Database},
    models::credentials::{DockerCredentialRecord, EnvVarRecord},
    routes::credentials::{CredentialResponse, EnvVarResponse},
    services::{app_services::AppServiceService, workspaces::WorkspaceService},
};

#[derive(Debug, Clone)]
pub struct CreateCredentialInput {
    pub workspace_id: Uuid,
    pub registry: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct CreateEnvVarInput {
    pub workspace_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub service_id: Option<Uuid>,
    pub key: String,
    pub value: String,
}

#[derive(Clone)]
pub struct CredentialService {
    db: Database,
    secret_key: String,
}

impl CredentialService {
    pub fn new(db: Database, secret_key: String) -> Self {
        Self { db, secret_key }
    }

    pub async fn list_credentials(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<CredentialResponse>, ApiError> {
        let mut db = self.db.handle()?;
        let records = Query::<List<DockerCredentialRecord>>::filter(
            DockerCredentialRecord::fields()
                .workspace_id()
                .eq(workspace_id),
        )
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(records
            .into_iter()
            .map(|record| CredentialResponse {
                id: record.id,
                workspace_id: record.workspace_id,
                registry: record.registry,
                username: record.username,
                password: crypto::redact("secret"),
            })
            .collect())
    }

    pub async fn create_credential(
        &self,
        input: CreateCredentialInput,
    ) -> Result<CredentialResponse, ApiError> {
        WorkspaceService::new(self.db.clone())
            .get_workspace(input.workspace_id)
            .await?;
        let mut db = self.db.handle()?;
        let record = toasty::create!(DockerCredentialRecord {
            id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            registry: input.registry,
            username: input.username,
            encrypted_password: crypto::encrypt_secret(&self.secret_key, &input.password),
        })
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(CredentialResponse {
            id: record.id,
            workspace_id: record.workspace_id,
            registry: record.registry,
            username: record.username,
            password: crypto::redact("secret"),
        })
    }

    pub async fn list_env_vars(&self, workspace_id: Uuid) -> Result<Vec<EnvVarResponse>, ApiError> {
        let mut db = self.db.handle()?;
        let records = Query::<List<EnvVarRecord>>::filter(
            EnvVarRecord::fields().workspace_id().eq(workspace_id),
        )
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(records.into_iter().map(env_var_response).collect())
    }

    pub async fn create_env_var(
        &self,
        input: CreateEnvVarInput,
    ) -> Result<EnvVarResponse, ApiError> {
        self.validate_env_var_scope(&input).await?;
        let mut db = self.db.handle()?;
        let record = toasty::create!(EnvVarRecord {
            id: Uuid::now_v7(),
            workspace_id: input.workspace_id,
            environment_id: input.environment_id,
            service_id: input.service_id,
            key: input.key,
            encrypted_value: crypto::encrypt_secret(&self.secret_key, &input.value),
        })
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(env_var_response(record))
    }

    pub async fn create_env_vars(
        &self,
        inputs: Vec<CreateEnvVarInput>,
    ) -> Result<Vec<EnvVarResponse>, ApiError> {
        for input in &inputs {
            self.validate_env_var_scope(input).await?;
        }

        let mut db = self.db.handle()?;
        let mut tx = db.transaction().await.map_err(map_toasty_error)?;
        let mut created = Vec::with_capacity(inputs.len());
        for input in inputs {
            let record = toasty::create!(EnvVarRecord {
                id: Uuid::now_v7(),
                workspace_id: input.workspace_id,
                environment_id: input.environment_id,
                service_id: input.service_id,
                key: input.key,
                encrypted_value: crypto::encrypt_secret(&self.secret_key, &input.value),
            })
            .exec(&mut tx)
            .await
            .map_err(map_toasty_error)?;
            created.push(env_var_response(record));
        }
        tx.commit().await.map_err(map_toasty_error)?;
        Ok(created)
    }

    async fn validate_env_var_scope(&self, input: &CreateEnvVarInput) -> Result<(), ApiError> {
        WorkspaceService::new(self.db.clone())
            .get_workspace(input.workspace_id)
            .await?;
        if let Some(environment_id) = input.environment_id {
            let environments = WorkspaceService::new(self.db.clone())
                .list_environments(input.workspace_id)
                .await?;
            if !environments
                .iter()
                .any(|environment| environment.id == environment_id)
            {
                return Err(ApiError::NotFound("environment"));
            }
        }
        if let Some(service_id) = input.service_id {
            let service = AppServiceService::new(self.db.clone())
                .get_service(service_id)
                .await?;
            if service.workspace_id != input.workspace_id {
                return Err(ApiError::NotFound("service"));
            }
        }
        Ok(())
    }
}

fn env_var_response(record: EnvVarRecord) -> EnvVarResponse {
    EnvVarResponse {
        id: record.id,
        workspace_id: record.workspace_id,
        environment_id: record.environment_id,
        service_id: record.service_id,
        key: record.key,
        value: crypto::redact("secret"),
    }
}

fn map_toasty_error(error: toasty::Error) -> ApiError {
    ApiError::Internal(format!("database operation failed: {error}"))
}
