use axum::{
    Json,
    body::{Body, to_bytes},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("authentication required")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("invalid request: {0}")]
    BadRequest(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<crate::errors::wara::WaraError> for ApiError {
    fn from(error: crate::errors::wara::WaraError) -> Self {
        Self::Internal(error.to_string())
    }
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::BadRequest(_) => "bad_request",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn public_message(&self) -> String {
        match self {
            Self::Internal(_) => "internal server error".to_string(),
            _ => self.to_string(),
        }
    }

    pub fn response_body(&self) -> ErrorResponse {
        ErrorResponse {
            code: self.code().to_string(),
            message: self.public_message(),
            details: None,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status();
        (status, Json(self.response_body())).into_response()
    }
}

impl ErrorResponse {
    pub fn from_status(status: StatusCode, details: Option<String>) -> Self {
        let (code, message) = match status {
            StatusCode::BAD_REQUEST => ("bad_request", "invalid request"),
            StatusCode::UNAUTHORIZED => ("unauthorized", "authentication required"),
            StatusCode::FORBIDDEN => ("forbidden", "forbidden"),
            StatusCode::NOT_FOUND => ("not_found", "route not found"),
            StatusCode::METHOD_NOT_ALLOWED => ("method_not_allowed", "method not allowed"),
            StatusCode::UNPROCESSABLE_ENTITY => ("bad_request", "invalid request"),
            status if status.is_server_error() => ("internal_error", "internal server error"),
            _ => ("request_error", "request failed"),
        };

        Self {
            code: code.to_string(),
            message: message.to_string(),
            details: if status.is_server_error() {
                None
            } else {
                details
            },
        }
    }
}

pub async fn normalize_error_response(request: axum::http::Request<Body>, next: Next) -> Response {
    let response = next.run(request).await;
    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return response;
    }

    let (parts, body) = response.into_parts();
    let bytes = match to_bytes(body, 64 * 1024).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::error!(%error, "failed to buffer error response body");
            return ApiError::Internal("failed to serialize error response".to_string())
                .into_response();
        }
    };

    if serde_json::from_slice::<ErrorResponse>(&bytes).is_ok() {
        return Response::from_parts(parts, Body::from(bytes));
    }

    let details = std::str::from_utf8(&bytes)
        .ok()
        .map(str::trim)
        .filter(|body| !body.is_empty())
        .map(ToOwned::to_owned);
    let body = ErrorResponse::from_status(status, details);
    let mut response = (status, Json(body)).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    response
}
