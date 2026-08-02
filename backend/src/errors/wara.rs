use thiserror::Error;

#[derive(Error, Debug)]
pub enum WaraError {
    #[error("{0} cannot be empty or missing")]
    MissingConfiguration(&'static str),

    #[error("invalid {0} configuration: {1}")]
    InvalidConfiguration(&'static str, String),

    #[error("JWT key signing error: {0}")]
    JwtSigning(String),

    #[error("JWT key verification error: {0}")]
    JwtVerification(String),

    #[error("configuration file error: {0}")]
    ConfigFile(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("cryptography error: {0}")]
    Crypto(String),

    #[error("SSH error: {0}")]
    Ssh(String),

    #[error("telemetry error: {0}")]
    Telemetry(String),

    #[error("deployment execution error: {0}")]
    DeploymentExecution(String),

    #[error("invalid queue: {0}")]
    InvalidQueue(String),

    #[error("application error: {0}")]
    Application(String),
}

impl From<toasty::Error> for WaraError {
    fn from(error: toasty::Error) -> Self {
        Self::Database(error.to_string())
    }
}

impl From<std::io::Error> for WaraError {
    fn from(error: std::io::Error) -> Self {
        Self::ConfigFile(error.to_string())
    }
}

impl From<serde_yaml_neo::Error> for WaraError {
    fn from(error: serde_yaml_neo::Error) -> Self {
        Self::ConfigFile(error.to_string())
    }
}

impl From<base64::DecodeError> for WaraError {
    fn from(error: base64::DecodeError) -> Self {
        Self::Crypto(error.to_string())
    }
}

impl From<std::string::FromUtf8Error> for WaraError {
    fn from(error: std::string::FromUtf8Error) -> Self {
        Self::Crypto(error.to_string())
    }
}

impl From<crate::errors::api::ApiError> for WaraError {
    fn from(error: crate::errors::api::ApiError) -> Self {
        Self::Application(error.to_string())
    }
}
