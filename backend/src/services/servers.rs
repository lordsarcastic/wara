use chrono::Utc;
use serde::Serialize;
use toasty::stmt::{List, Query, Update};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    errors::ApiError,
    libs::{
        crypto,
        db::Database,
        docker::ProxyKind,
        ssh::{self, SshTarget},
    },
    models::servers::{Server, ServerRecord},
};

#[derive(Debug, Clone)]
pub struct CreateServerInput {
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub public_key: String,
    pub private_key: String,
    pub private_key_passphrase: Option<String>,
    pub default_proxy: Option<ProxyKind>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ServerCheckResponse {
    pub server_id: Uuid,
    pub ssh_status: String,
    pub docker_status: String,
    pub docker_version: Option<String>,
    pub checked_at: String,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct ServerService {
    db: Database,
    secret_key: String,
}

impl ServerService {
    pub fn new(db: Database, secret_key: String) -> Self {
        Self { db, secret_key }
    }

    pub async fn list_servers(&self) -> Result<Vec<Server>, ApiError> {
        let mut db = self.db.handle()?;
        let records = Query::<List<ServerRecord>>::all()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        Ok(records.into_iter().map(Server::from).collect())
    }

    pub async fn create_server(&self, payload: CreateServerInput) -> Result<Server, ApiError> {
        validate_ssh_key_pair(&payload)?;

        let mut db = self.db.handle()?;
        let record = toasty::create!(ServerRecord {
            id: Uuid::now_v7(),
            name: payload.name,
            host: payload.host,
            port: payload.port.unwrap_or(22),
            username: payload.username,
            public_key: payload.public_key.clone(),
            private_key_fingerprint: ssh::fingerprint_public_key(&payload.public_key),
            encrypted_private_key: crypto::encrypt_secret(&self.secret_key, &payload.private_key),
            encrypted_private_key_passphrase: payload
                .private_key_passphrase
                .as_deref()
                .map(|value| crypto::encrypt_secret(&self.secret_key, value)),
            default_proxy: payload
                .default_proxy
                .unwrap_or(ProxyKind::Nginx)
                .as_str()
                .to_string(),
            docker_status: "unchecked".to_string(),
            docker_version: String::new(),
            last_check_at: String::new(),
            last_check_error: String::new(),
        })
        .exec(&mut db)
        .await
        .map_err(map_toasty_error)?;
        Ok(Server::from(record))
    }

    pub async fn get_server(&self, id: Uuid) -> Result<Server, ApiError> {
        let mut db = self.db.handle()?;
        let record = Query::<List<ServerRecord>>::filter(ServerRecord::fields().id().eq(id))
            .first()
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        record.map(Server::from).ok_or(ApiError::NotFound("server"))
    }

    pub async fn check_server(&self, id: Uuid) -> Result<ServerCheckResponse, ApiError> {
        let server = self.get_server(id).await?;
        let target = self.ssh_target(&server)?;
        let checked_at = Utc::now().to_rfc3339();
        let commands = ssh::server_check_commands();

        let (docker_status, docker_version, last_check_error) =
            match ssh::run_controlled_commands(&target, &commands).await {
                Ok(output) => match ssh::parse_server_check_output(&output) {
                    Ok(version) => ("available".to_string(), version, String::new()),
                    Err(error) => (
                        "unavailable".to_string(),
                        String::new(),
                        ssh::redact_error(&error.to_string(), &target),
                    ),
                },
                Err(error) => (
                    "unavailable".to_string(),
                    String::new(),
                    ssh::redact_error(&error.to_string(), &target),
                ),
            };

        self.update_check_metadata(
            id,
            &docker_status,
            &docker_version,
            &checked_at,
            &last_check_error,
        )
        .await?;

        Ok(ServerCheckResponse {
            server_id: id,
            ssh_status: if docker_status == "available" {
                "connected".to_string()
            } else {
                "failed".to_string()
            },
            docker_status,
            docker_version: non_empty(docker_version),
            checked_at,
            error: non_empty(last_check_error),
        })
    }

    pub async fn check_all_servers(&self) -> Result<Vec<ServerCheckResponse>, ApiError> {
        let servers = self.list_servers().await?;
        let mut checks = Vec::with_capacity(servers.len());
        for server in servers {
            match self.check_server(server.id).await {
                Ok(check) => checks.push(check),
                Err(error) => {
                    tracing::warn!(
                        server_id = %server.id,
                        error = %error,
                        "scheduled server connectivity check failed"
                    );
                }
            }
        }
        Ok(checks)
    }

    pub fn ssh_target(&self, server: &Server) -> Result<SshTarget, ApiError> {
        Ok(SshTarget {
            host: server.host.clone(),
            port: server.port,
            username: server.username.clone(),
            public_key: server.public_key.clone(),
            private_key: crypto::decrypt_secret(&self.secret_key, &server.encrypted_private_key)
                .map_err(|error| {
                    ApiError::Internal(format!("failed to decrypt server private key: {error}"))
                })?,
            private_key_passphrase: server
                .encrypted_private_key_passphrase
                .as_deref()
                .map(|value| {
                    crypto::decrypt_secret(&self.secret_key, value).map_err(|error| {
                        ApiError::Internal(format!(
                            "failed to decrypt server private key passphrase: {error}"
                        ))
                    })
                })
                .transpose()?,
        })
    }

    async fn update_check_metadata(
        &self,
        id: Uuid,
        docker_status: &str,
        docker_version: &str,
        checked_at: &str,
        last_check_error: &str,
    ) -> Result<(), ApiError> {
        let mut db = self.db.handle()?;
        let mut update_server = Update::<List<ServerRecord>>::new(
            Query::<List<ServerRecord>>::filter(ServerRecord::fields().id().eq(id)),
        );
        update_server.set(10, docker_status);
        update_server.set(11, docker_version);
        update_server.set(12, checked_at);
        update_server.set(13, last_check_error);
        update_server.set_returning_none();
        update_server
            .exec(&mut db)
            .await
            .map_err(map_toasty_error)?;
        Ok(())
    }
}

fn validate_ssh_key_pair(payload: &CreateServerInput) -> Result<(), ApiError> {
    if !payload.public_key.trim_start().starts_with("ssh-") {
        return Err(ApiError::BadRequest(
            "public_key must be an OpenSSH public key".to_string(),
        ));
    }
    if !payload.private_key.contains("PRIVATE KEY") {
        return Err(ApiError::BadRequest(
            "private_key must be a PEM/OpenSSH private key".to_string(),
        ));
    }
    Ok(())
}

fn map_toasty_error(error: toasty::Error) -> ApiError {
    ApiError::Internal(format!("database operation failed: {error}"))
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}
