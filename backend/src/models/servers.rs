use crate::libs::docker::ProxyKind;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct Server {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub public_key: String,
    pub private_key_fingerprint: String,
    #[serde(skip_serializing)]
    #[schema(ignore)]
    pub encrypted_private_key: String,
    #[serde(skip_serializing)]
    #[schema(ignore)]
    pub encrypted_private_key_passphrase: Option<String>,
    pub default_proxy: ProxyKind,
    pub docker_status: String,
    pub docker_version: String,
    pub last_check_at: String,
    pub last_check_error: String,
}

#[derive(Debug, Clone, toasty::Model)]
pub struct ServerRecord {
    #[key]
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub public_key: String,
    pub private_key_fingerprint: String,
    pub encrypted_private_key: String,
    pub encrypted_private_key_passphrase: Option<String>,
    pub default_proxy: String,
    pub docker_status: String,
    pub docker_version: String,
    pub last_check_at: String,
    pub last_check_error: String,
}

impl From<ServerRecord> for Server {
    fn from(record: ServerRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            host: record.host,
            port: record.port,
            username: record.username,
            public_key: record.public_key,
            private_key_fingerprint: record.private_key_fingerprint,
            encrypted_private_key: record.encrypted_private_key,
            encrypted_private_key_passphrase: record.encrypted_private_key_passphrase,
            default_proxy: ProxyKind::from(record.default_proxy.as_str()),
            docker_status: record.docker_status,
            docker_version: record.docker_version,
            last_check_at: record.last_check_at,
            last_check_error: record.last_check_error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_serialization_does_not_expose_private_key_material() {
        let server = Server {
            id: Uuid::now_v7(),
            name: "test".to_string(),
            host: "example.com".to_string(),
            port: 22,
            username: "deploy".to_string(),
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITest".to_string(),
            private_key_fingerprint: "SHA256:test".to_string(),
            encrypted_private_key: "encrypted-private-key".to_string(),
            encrypted_private_key_passphrase: Some("encrypted-passphrase".to_string()),
            default_proxy: ProxyKind::Nginx,
            docker_status: "unchecked".to_string(),
            docker_version: String::new(),
            last_check_at: String::new(),
            last_check_error: String::new(),
        };

        let serialized = serde_json::to_string(&server).unwrap();
        assert!(serialized.contains("public_key"));
        assert!(serialized.contains("private_key_fingerprint"));
        assert!(!serialized.contains("encrypted_private_key"));
        assert!(!serialized.contains("encrypted-private-key"));
        assert!(!serialized.contains("encrypted-passphrase"));
    }
}
