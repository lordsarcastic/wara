use std::{fs, path::PathBuf};

use serde::Deserialize;

use crate::libs::docker::{DEFAULT_DOCKERFILE_CONTEXT_DIR, DEFAULT_REMOTE_SERVICES_ROOT};

pub const DEFAULT_JWT_PRIVATE_KEY_PEM: &str = r#"-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEAs5i+coVRLORZo2cnS8ZgM3NrHhIgpTwUpaPbTLWjORDUqWye
qeVGeoAhZeaFFdRR6FXr+kZfcXrZxioFtW/IQt3dU+OXkYdnvEXy79ORpKSpR6JK
W/gDQ3cuxavgyJ8d1Xd7fiI6OX/iimsQCTDZ7zJQJ50JlKZiFPU+MIHXcF5BoZMX
DWxAAFvG7Upgp2UGX7gt5x/6UqpiHVRgb4IX8qsZNPElFZmtD0FwuminkAVA6Lry
ESCK2G5FMMbtorI55Te2WFlxpVnCYkGf0mkGCANrHvQYI4/gmhRTv53ypyFRWA0s
N4FKbEazCSP1dZFV07GCAAR/hTy+xOudgde9+QIDAQABAoIBAEcQTWtrLS+iO4XY
r0fgevhg1yXS7m/zUggoygGUbb2K11siy7VWL4kRYiW8DTUSCkbwmKszZVi1z64F
urSMQqWSvJ0RFUxUU8u/sd0LzjljnkfmA55YiJINeshktlEsBNYOrSK/0GIoJC+5
JWM9nT50nhrOnJfhLjY0xCLVfbXMLGI9SxGHdukbtlYvInFaegP7gqq5RpERLpNP
ULW0sfn+GEFXBqNCk0a2c9mPtAJR8B46TPCwMuawpOlyQABPHtuqKihZE1hxItl9
fe8DSNLXm5Edy9F9gQ4jHmreIdlE1i929441WyFnwc0mrF6Y1phd1ZkZo0GeoLSi
f2uIU8ECgYEA2BO4scGp/nJTq7WS061/Kxx7Vo5xP9p5QBZ4KVC8WSEScoCCX4Ps
1HSqi8y8XZpIeq+vY1gxneZpq2Umd1X80v0qsMU5giL6cpPEw2dP99M99zoiSp7z
vvQAug6Va2olmG05FiVxOyPfzcfkG6xixQOwXXv/2ABA6R5odRAYvD0CgYEA1MeH
QsFyfHcQNPHtZ+6EK3AzBaTGuWtwl+7LHeq2qgWFrMCF6XDjZLG2Vz7nbR+ngXkH
+OpSR/nXT+SXZ7QOnQRQaNyopUvp69Y5V6ezI4xPy2RP/zNo4/jhhjRTO3eEb08T
q+BCbtsSKY9aDkCzJ3yF15B246q3LoFmzh2PeG0CgYEAyU9GacXmnPri3T0jeDdS
HVZBytiWxkjDYmQMu2FOuTNIvojf7iE5Co9PPUQX0pUlJbh8jO/j+hprJJXuiowA
KopXta1p8Map0wm87dhY9qlGOAlfXWpN6P/nlXB04UhZknNgFjP4FINNxaiP6wBm
XOsc61vVduZ1kzsTUs0WXnkCgYAz5TYoIeY6VQ+u2hJ89r9lmMfY6IdPUdT0OVlw
wn4qmY4wxAPlG5NaS72dKcpn4wCHo20+WGgZBeZtpeMHd/LYeOTjrm2zYwB6dJUn
u88FLIOJp72bEH7Umy7l/H0QU+YI/9BcayXIw8V6PWxJbZ5EUyqRmLpmbIyg2w6n
1q3XQQKBgQDAncwI4nbMmlDsQELHfq3hhrZ+xVahPVopdmrRp91CYNhV6IRs8A7J
mW0330H8k0PSCYGw2sHQNJa551ATSTQMgNmk9rGvLCx+1n9uR+xLn0o3+zLH0qzz
QsOwgROF7MyqBRABgVty5Mr5mgGNV8e+k075Mvu/uV7YoazGYmAvaA==
-----END RSA PRIVATE KEY-----"#;

pub const DEFAULT_JWT_PUBLIC_KEY_PEM: &str = r#"-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAs5i+coVRLORZo2cnS8Zg
M3NrHhIgpTwUpaPbTLWjORDUqWyeqeVGeoAhZeaFFdRR6FXr+kZfcXrZxioFtW/I
Qt3dU+OXkYdnvEXy79ORpKSpR6JKW/gDQ3cuxavgyJ8d1Xd7fiI6OX/iimsQCTDZ
7zJQJ50JlKZiFPU+MIHXcF5BoZMXDWxAAFvG7Upgp2UGX7gt5x/6UqpiHVRgb4IX
8qsZNPElFZmtD0FwuminkAVA6LryESCK2G5FMMbtorI55Te2WFlxpVnCYkGf0mkG
CANrHvQYI4/gmhRTv53ypyFRWA0sN4FKbEazCSP1dZFV07GCAAR/hTy+xOudgde9
+QIDAQAB
-----END PUBLIC KEY-----"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEnvironment {
    Development,
    Test,
    Staging,
    Production,
}

impl AppEnvironment {
    pub fn from_value(value: &str) -> Self {
        match value {
            "test" => Self::Test,
            "staging" => Self::Staging,
            "prod" | "production" => Self::Production,
            _ => Self::Development,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub app_env: AppEnvironment,
    pub bind_addr: String,
    pub config_file: Option<PathBuf>,
    pub postgres_user: String,
    pub postgres_password: String,
    pub postgres_db: String,
    pub postgres_port: u16,
    pub database_url: String,
    pub secret_key: String,
    pub db_push_schema: bool,
    pub docs_enabled: bool,
    pub remote_services_root: String,
    pub dockerfile_context_dir: String,
    pub jwt_private_key_pem: String,
    pub jwt_public_key_pem: String,
    pub jwt_issuer: String,
    pub jwt_audience: String,
    pub jwt_access_token_ttl_seconds: u64,
    pub app_base_url: String,
    pub invite_token_ttl_seconds: u64,
    pub bootstrap_admin_email: String,
    pub bootstrap_admin_password: String,
    pub bootstrap_admin_name: String,
    pub telemetry_enabled: bool,
    pub otel_service_name: String,
    pub otel_exporter_otlp_endpoint: Option<String>,
    pub backend_port: u16,
    pub frontend_port: u16,
    pub temporal_address: String,
    pub temporal_namespace: String,
    pub temporal_namespaces: String,
    pub temporal_version: String,
    pub temporal_ui_version: String,
    pub temporal_postgresql_version: String,
    pub temporal_postgres_user: String,
    pub temporal_postgres_password: String,
    pub temporal_postgres_port: u16,
    pub temporal_frontend_port: u16,
    pub temporal_ui_port: u16,
    pub temporal_ui_cors_origins: String,
    pub prometheus_port: u16,
    pub jaeger_ui_port: u16,
    pub jaeger_grpc_port: u16,
    pub jaeger_http_port: u16,
    pub elastic_username: String,
    pub elastic_password: String,
    pub elastic_port: u16,
    pub grafana_port: u16,
    pub grafana_admin_user: String,
    pub grafana_admin_password: String,
    pub test_database_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self::from_sources(ConfigFile::load_default().unwrap_or_default())
    }

    fn from_sources(file: ConfigFile) -> Self {
        let app_env = AppEnvironment::from_value(&setting(
            "APP_ENV",
            file.app_env,
            "development".to_string(),
        ));
        Self {
            app_env,
            config_file: config_path(),
            bind_addr: setting("BIND_ADDR", file.bind_addr, "0.0.0.0:8080".to_string()),
            postgres_user: setting("POSTGRES_USER", file.postgres_user, "wara".to_string()),
            postgres_password: setting(
                "POSTGRES_PASSWORD",
                file.postgres_password,
                "wara".to_string(),
            ),
            postgres_db: setting("POSTGRES_DB", file.postgres_db, "wara".to_string()),
            postgres_port: u16_setting("POSTGRES_PORT", file.postgres_port, 5432),
            database_url: setting(
                "DATABASE_URL",
                file.database_url,
                "postgres://wara:wara@localhost:5432/wara".to_string(),
            ),
            secret_key: setting(
                "WARA_SECRET_KEY",
                file.secret_key,
                "development-secret-change-me".to_string(),
            ),
            db_push_schema: bool_setting("WARA_DB_PUSH_SCHEMA", file.db_push_schema, false),
            docs_enabled: bool_setting("WARA_DOCS_ENABLED", file.docs_enabled, true),
            remote_services_root: setting(
                "WARA_REMOTE_SERVICES_ROOT",
                file.remote_services_root,
                DEFAULT_REMOTE_SERVICES_ROOT.to_string(),
            ),
            dockerfile_context_dir: setting(
                "WARA_DOCKERFILE_CONTEXT_DIR",
                file.dockerfile_context_dir,
                DEFAULT_DOCKERFILE_CONTEXT_DIR.to_string(),
            ),
            jwt_private_key_pem: setting(
                "WARA_JWT_PRIVATE_KEY_PEM",
                file.jwt_private_key_pem,
                DEFAULT_JWT_PRIVATE_KEY_PEM.to_string(),
            ),
            jwt_public_key_pem: setting(
                "WARA_JWT_PUBLIC_KEY_PEM",
                file.jwt_public_key_pem,
                DEFAULT_JWT_PUBLIC_KEY_PEM.to_string(),
            ),
            jwt_issuer: setting("WARA_JWT_ISSUER", file.jwt_issuer, "wara".to_string()),
            jwt_audience: setting(
                "WARA_JWT_AUDIENCE",
                file.jwt_audience,
                "wara-api".to_string(),
            ),
            jwt_access_token_ttl_seconds: u64_setting(
                "WARA_JWT_ACCESS_TOKEN_TTL_SECONDS",
                file.jwt_access_token_ttl_seconds,
                900,
            ),
            app_base_url: setting(
                "WARA_APP_BASE_URL",
                file.app_base_url,
                "http://localhost:4200".to_string(),
            ),
            invite_token_ttl_seconds: u64_setting(
                "WARA_INVITE_TOKEN_TTL_SECONDS",
                file.invite_token_ttl_seconds,
                604800,
            ),
            bootstrap_admin_email: setting(
                "WARA_BOOTSTRAP_ADMIN_EMAIL",
                file.bootstrap_admin_email,
                "admin@wara.local".to_string(),
            ),
            bootstrap_admin_password: setting(
                "WARA_BOOTSTRAP_ADMIN_PASSWORD",
                file.bootstrap_admin_password,
                "change-me".to_string(),
            ),
            bootstrap_admin_name: setting(
                "WARA_BOOTSTRAP_ADMIN_NAME",
                file.bootstrap_admin_name,
                "Wara Admin".to_string(),
            ),
            telemetry_enabled: bool_setting(
                "WARA_TELEMETRY_ENABLED",
                file.telemetry_enabled,
                false,
            ),
            otel_service_name: setting(
                "OTEL_SERVICE_NAME",
                file.otel_service_name,
                "wara-backend".to_string(),
            ),
            otel_exporter_otlp_endpoint: optional_setting(
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                file.otel_exporter_otlp_endpoint,
            ),
            backend_port: u16_setting("BACKEND_PORT", file.backend_port, 8080),
            frontend_port: u16_setting("FRONTEND_PORT", file.frontend_port, 4200),
            temporal_address: setting(
                "TEMPORAL_ADDRESS",
                file.temporal_address,
                "localhost:7233".to_string(),
            ),
            temporal_namespace: setting(
                "TEMPORAL_NAMESPACE",
                file.temporal_namespace,
                "default".to_string(),
            ),
            temporal_namespaces: setting(
                "TEMPORAL_NAMESPACES",
                file.temporal_namespaces,
                "default".to_string(),
            ),
            temporal_version: setting(
                "TEMPORAL_VERSION",
                file.temporal_version,
                "1.30.1".to_string(),
            ),
            temporal_ui_version: setting(
                "TEMPORAL_UI_VERSION",
                file.temporal_ui_version,
                "2.34.0".to_string(),
            ),
            temporal_postgresql_version: setting(
                "TEMPORAL_POSTGRESQL_VERSION",
                file.temporal_postgresql_version,
                "16".to_string(),
            ),
            temporal_postgres_user: setting(
                "TEMPORAL_POSTGRES_USER",
                file.temporal_postgres_user,
                "temporal".to_string(),
            ),
            temporal_postgres_password: setting(
                "TEMPORAL_POSTGRES_PASSWORD",
                file.temporal_postgres_password,
                "temporal".to_string(),
            ),
            temporal_postgres_port: u16_setting(
                "TEMPORAL_POSTGRES_PORT",
                file.temporal_postgres_port,
                5432,
            ),
            temporal_frontend_port: u16_setting(
                "TEMPORAL_FRONTEND_PORT",
                file.temporal_frontend_port,
                7233,
            ),
            temporal_ui_port: u16_setting("TEMPORAL_UI_PORT", file.temporal_ui_port, 8081),
            temporal_ui_cors_origins: setting(
                "TEMPORAL_UI_CORS_ORIGINS",
                file.temporal_ui_cors_origins,
                "http://localhost:4200,http://localhost:8080".to_string(),
            ),
            prometheus_port: u16_setting("PROMETHEUS_PORT", file.prometheus_port, 9090),
            jaeger_ui_port: u16_setting("JAEGER_UI_PORT", file.jaeger_ui_port, 16686),
            jaeger_grpc_port: u16_setting("JAEGER_GRPC_PORT", file.jaeger_grpc_port, 4317),
            jaeger_http_port: u16_setting("JAEGER_HTTP_PORT", file.jaeger_http_port, 4318),
            elastic_username: setting(
                "ELASTIC_USERNAME",
                file.elastic_username,
                "elastic".to_string(),
            ),
            elastic_password: setting(
                "ELASTIC_PASSWORD",
                file.elastic_password,
                "changeme".to_string(),
            ),
            elastic_port: u16_setting("ELASTIC_PORT", file.elastic_port, 9200),
            grafana_port: u16_setting("GRAFANA_PORT", file.grafana_port, 3000),
            grafana_admin_user: setting(
                "GRAFANA_ADMIN_USER",
                file.grafana_admin_user,
                "admin".to_string(),
            ),
            grafana_admin_password: setting(
                "GRAFANA_ADMIN_PASSWORD",
                file.grafana_admin_password,
                "admin".to_string(),
            ),
            test_database_url: optional_setting("WARA_TEST_DATABASE_URL", file.test_database_url),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    app_env: Option<String>,
    bind_addr: Option<String>,
    postgres_user: Option<String>,
    postgres_password: Option<String>,
    postgres_db: Option<String>,
    postgres_port: Option<u16>,
    database_url: Option<String>,
    secret_key: Option<String>,
    db_push_schema: Option<bool>,
    docs_enabled: Option<bool>,
    remote_services_root: Option<String>,
    dockerfile_context_dir: Option<String>,
    jwt_private_key_pem: Option<String>,
    jwt_public_key_pem: Option<String>,
    jwt_issuer: Option<String>,
    jwt_audience: Option<String>,
    jwt_access_token_ttl_seconds: Option<u64>,
    app_base_url: Option<String>,
    invite_token_ttl_seconds: Option<u64>,
    bootstrap_admin_email: Option<String>,
    bootstrap_admin_password: Option<String>,
    bootstrap_admin_name: Option<String>,
    telemetry_enabled: Option<bool>,
    otel_service_name: Option<String>,
    otel_exporter_otlp_endpoint: Option<String>,
    backend_port: Option<u16>,
    frontend_port: Option<u16>,
    temporal_address: Option<String>,
    temporal_namespace: Option<String>,
    temporal_namespaces: Option<String>,
    temporal_version: Option<String>,
    temporal_ui_version: Option<String>,
    temporal_postgresql_version: Option<String>,
    temporal_postgres_user: Option<String>,
    temporal_postgres_password: Option<String>,
    temporal_postgres_port: Option<u16>,
    temporal_frontend_port: Option<u16>,
    temporal_ui_port: Option<u16>,
    temporal_ui_cors_origins: Option<String>,
    prometheus_port: Option<u16>,
    jaeger_ui_port: Option<u16>,
    jaeger_grpc_port: Option<u16>,
    jaeger_http_port: Option<u16>,
    elastic_username: Option<String>,
    elastic_password: Option<String>,
    elastic_port: Option<u16>,
    grafana_port: Option<u16>,
    grafana_admin_user: Option<String>,
    grafana_admin_password: Option<String>,
    test_database_url: Option<String>,
}

impl ConfigFile {
    fn load_default() -> anyhow::Result<Self> {
        let Some(path) = config_path() else {
            return Ok(Self::default());
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(&path)?;
        let file = serde_yaml_neo::from_str(&contents)?;
        Ok(file)
    }
}

fn config_path() -> Option<PathBuf> {
    std::env::var_os("WARA_CONFIG_FILE")
        .map(PathBuf::from)
        .or_else(default_config_path)
}

fn default_config_path() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".wara").join("config.yml"))
}

fn setting(env_name: &str, file_value: Option<String>, default: String) -> String {
    std::env::var(env_name)
        .ok()
        .or(file_value)
        .unwrap_or(default)
}

fn optional_setting(env_name: &str, file_value: Option<String>) -> Option<String> {
    std::env::var(env_name).ok().or(file_value)
}

fn bool_setting(env_name: &str, file_value: Option<bool>, default: bool) -> bool {
    std::env::var(env_name)
        .ok()
        .and_then(|value| parse_bool(&value))
        .or(file_value)
        .unwrap_or(default)
}

fn u16_setting(env_name: &str, file_value: Option<u16>, default: u16) -> u16 {
    std::env::var(env_name)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .or(file_value)
        .unwrap_or(default)
}

fn u64_setting(env_name: &str, file_value: Option<u64>, default: u64) -> u64 {
    std::env::var(env_name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .or(file_value)
        .unwrap_or(default)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_file_values_fill_defaults() {
        // The loader gives environment variables precedence over file values, so the
        // variables this test asserts are cleared first to keep it deterministic
        // regardless of the ambient environment (CI, for example, sets DATABASE_URL).
        let vars = [
            "BIND_ADDR",
            "DATABASE_URL",
            "WARA_DB_PUSH_SCHEMA",
            "WARA_TELEMETRY_ENABLED",
            "WARA_REMOTE_SERVICES_ROOT",
            "WARA_DOCKERFILE_CONTEXT_DIR",
            "TEMPORAL_NAMESPACE",
            "OTEL_SERVICE_NAME",
        ];
        let saved: Vec<(&str, Option<String>)> = vars
            .iter()
            .map(|&key| (key, std::env::var(key).ok()))
            .collect();
        unsafe {
            for &key in &vars {
                std::env::remove_var(key);
            }
        }

        let config = Config::from_sources(ConfigFile {
            bind_addr: Some("127.0.0.1:9000".to_string()),
            database_url: Some("postgres://file".to_string()),
            telemetry_enabled: Some(true),
            remote_services_root: Some("/srv/wara/apps".to_string()),
            dockerfile_context_dir: Some("src".to_string()),
            temporal_namespace: Some("wara".to_string()),
            ..ConfigFile::default()
        });

        // Restore before asserting so a failed assertion cannot leak environment state.
        unsafe {
            for (key, value) in saved {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }

        assert_eq!(config.bind_addr, "127.0.0.1:9000");
        assert_eq!(config.database_url, "postgres://file");
        assert!(config.telemetry_enabled);
        assert_eq!(config.remote_services_root, "/srv/wara/apps");
        assert_eq!(config.dockerfile_context_dir, "src");
        assert_eq!(config.temporal_namespace, "wara");
        assert_eq!(config.otel_service_name, "wara-backend");
        assert!(!config.db_push_schema);
    }

    #[test]
    fn parses_bool_values() {
        assert_eq!(parse_bool("true"), Some(true));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("off"), Some(false));
        assert_eq!(parse_bool("nope"), None);
    }

    #[test]
    fn default_path_uses_home_wara_config() {
        let original = std::env::var_os("HOME");
        unsafe {
            std::env::set_var("HOME", "/tmp/wara-home");
        }

        assert_eq!(
            default_config_path().unwrap(),
            PathBuf::from("/tmp/wara-home/.wara/config.yml")
        );

        unsafe {
            match original {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
        }
    }
}
