use thiserror::Error;

#[derive(Error, Debug)]
pub enum WaraError {
    #[error("{0} cannot be empty or missing")]
    MissingConfiguration(&'static str),

    #[error("Invalid {0} configuration: {1}")]
    InvalidConfiguration(&'static str, String),

    #[error("JWT key signing error: {0}")]
    JWTSigning(String)
}