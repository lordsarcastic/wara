use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
    response::Response,
};
use serde_json::Value;
use tower::ServiceExt;
use utoipa::OpenApi;
use wara_backend::{
    libs::{config::Config, db::Database},
    openapi::ApiDoc,
    routes,
    state::AppState,
};

fn app() -> axum::Router {
    let state = AppState::new(Config::from_env(), Database::unavailable_for_tests());
    routes::router(state)
}

#[tokio::test]
async fn health_is_public() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn protected_routes_require_auth() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/workspaces")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_error(response, "unauthorized", "authentication required").await;
}

#[tokio::test]
async fn openapi_is_exposed() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn invalid_json_payloads_are_rejected_by_validation() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"email":"not-an-email","password":""}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = assert_error(response, "bad_request", "invalid request").await;
    assert!(
        body.get("details").and_then(Value::as_str).is_some(),
        "validation response should include rejection details"
    );
}

#[tokio::test]
async fn malformed_json_payloads_use_error_envelope() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"email":"admin@wara.local""#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_error(response, "bad_request", "invalid request").await;
}

#[tokio::test]
async fn unmatched_api_routes_use_error_envelope() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/not-a-real-route")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_error(response, "not_found", "route not found").await;
}

#[tokio::test]
async fn internal_errors_are_redacted_in_error_envelope() {
    let mut config = Config::from_env();
    config.jwt_public_key_pem = "not a valid public key".to_string();
    let state = AppState::new(config, Database::unavailable_for_tests());
    let response = routes::router(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", "Bearer malformed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = assert_error(response, "internal_error", "internal server error").await;
    assert!(
        !body.to_string().contains("public key"),
        "internal details must not leak: {body}"
    );
}

#[tokio::test]
async fn product_resource_path_ids_must_be_uuids() {
    let cases = [
        (Method::GET, "/api/v1/workspaces/not-a-uuid"),
        (Method::GET, "/api/v1/servers/not-a-uuid"),
        (Method::GET, "/api/v1/workspaces/not-a-uuid/environments"),
        (Method::POST, "/api/v1/workspaces/not-a-uuid/environments"),
        (Method::GET, "/api/v1/workspaces/not-a-uuid/services"),
        (Method::POST, "/api/v1/workspaces/not-a-uuid/services"),
        (Method::GET, "/api/v1/services/not-a-uuid"),
        (Method::GET, "/api/v1/workspaces/not-a-uuid/credentials"),
        (Method::POST, "/api/v1/workspaces/not-a-uuid/credentials"),
        (Method::GET, "/api/v1/workspaces/not-a-uuid/env-vars"),
        (Method::POST, "/api/v1/workspaces/not-a-uuid/env-vars"),
        (Method::GET, "/api/v1/services/not-a-uuid/domains"),
        (Method::POST, "/api/v1/services/not-a-uuid/domains"),
        (Method::POST, "/api/v1/domains/not-a-uuid/proxy-preview"),
        (Method::GET, "/api/v1/services/not-a-uuid/deployments"),
        (Method::POST, "/api/v1/services/not-a-uuid/deployments"),
        (Method::GET, "/api/v1/deployments/not-a-uuid"),
        (Method::POST, "/api/v1/services/not-a-uuid/restart"),
        (Method::GET, "/api/v1/services/not-a-uuid/logs"),
        (Method::GET, "/api/v1/workspaces/not-a-uuid/templates"),
        (Method::POST, "/api/v1/workspaces/not-a-uuid/templates"),
        (Method::POST, "/api/v1/templates/not-a-uuid/workspaces"),
    ];

    for (method, uri) in cases {
        let response = app()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        assert_error(response, "bad_request", "invalid request").await;
    }
}

#[test]
fn openapi_path_id_parameters_are_documented_as_uuids() {
    let openapi = serde_json::to_value(ApiDoc::openapi()).expect("serialize OpenAPI document");
    let paths = openapi
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths object");

    for (path, operations) in paths {
        let operations = operations.as_object().expect("path item object");
        for (method, operation) in operations {
            let Some(parameters) = operation.get("parameters").and_then(Value::as_array) else {
                continue;
            };

            for parameter in parameters {
                let name = parameter.get("name").and_then(Value::as_str);
                let location = parameter.get("in").and_then(Value::as_str);
                if location == Some("path")
                    && matches!(name, Some("id" | "workspace_id" | "service_id"))
                {
                    let schema = parameter
                        .get("schema")
                        .expect("path parameter must have a schema");
                    assert_eq!(
                        schema.get("type").and_then(Value::as_str),
                        Some("string"),
                        "{method} {path} parameter {name:?} should be a string UUID"
                    );
                    assert_eq!(
                        schema.get("format").and_then(Value::as_str),
                        Some("uuid"),
                        "{method} {path} parameter {name:?} should use UUID format"
                    );
                }
            }
        }
    }
}

#[test]
fn openapi_documents_common_error_responses() {
    let openapi = serde_json::to_value(ApiDoc::openapi()).expect("serialize OpenAPI document");
    let paths = openapi
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths object");

    for (path, operations) in paths {
        let operations = operations.as_object().expect("path item object");
        for (method, operation) in operations {
            let responses = operation
                .get("responses")
                .and_then(Value::as_object)
                .expect("operation responses object");
            for status in ["400", "401", "403", "404", "500"] {
                let response = responses
                    .get(status)
                    .unwrap_or_else(|| panic!("{method} {path} missing {status} response"));
                let schema_ref = response
                    .pointer("/content/application~1json/schema/$ref")
                    .and_then(Value::as_str);
                assert_eq!(
                    schema_ref,
                    Some("#/components/schemas/ErrorResponse"),
                    "{method} {path} {status} should document ErrorResponse"
                );
            }
        }
    }
}

async fn assert_error(response: Response, code: &str, message: &str) -> Value {
    let bytes = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read response body");
    let body = serde_json::from_slice::<Value>(&bytes).expect("error response should be JSON");
    assert_eq!(body["code"], code);
    assert_eq!(body["message"], message);
    body
}
