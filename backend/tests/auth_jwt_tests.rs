use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;
use wara_backend::{
    libs::{config::Config, db},
    routes,
    services::auth::AuthService,
    state::AppState,
};

#[tokio::test]
async fn login_issues_asymmetric_jwt_and_me_verifies_it() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping Toasty integration test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };

    let test_database_url = create_isolated_database(&database_url).await;
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.db_push_schema = true;
    config.bootstrap_admin_email = "admin-auth@wara.local".to_string();
    config.bootstrap_admin_password = "correct-password".to_string();
    config.bootstrap_admin_name = "Auth Admin".to_string();

    let database = db::connect(&config).await.expect("connect test database");
    AuthService::new(database.clone(), config.clone())
        .bootstrap_admin()
        .await
        .expect("bootstrap admin");
    let app = routes::router(AppState::new(config, database));

    let login_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"admin-auth@wara.local","password":"correct-password"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_response.status(), StatusCode::OK);
    let login_body = response_json(login_response).await;
    let token = login_body["token"].as_str().expect("jwt token");
    assert_eq!(token.split('.').count(), 3);

    let me_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_response.status(), StatusCode::OK);
    let me_body = response_json(me_response).await;
    assert_eq!(me_body["email"], "admin-auth@wara.local");

    let tampered_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {token}tampered"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tampered_response.status(), StatusCode::UNAUTHORIZED);

    drop_isolated_database(&test_database_url).await;
}

#[tokio::test]
async fn admin_can_invite_user_and_user_accepts_once() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping Toasty integration test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };

    let test_database_url = create_isolated_database(&database_url).await;
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.db_push_schema = true;
    config.bootstrap_admin_email = "invite-admin@wara.local".to_string();
    config.bootstrap_admin_password = "correct-password".to_string();
    config.bootstrap_admin_name = "Invite Admin".to_string();
    config.app_base_url = "http://localhost:4200".to_string();

    let database = db::connect(&config).await.expect("connect test database");
    AuthService::new(database.clone(), config.clone())
        .bootstrap_admin()
        .await
        .expect("bootstrap admin");
    let app = routes::router(AppState::new(config, database));

    let admin_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"invite-admin@wara.local","password":"correct-password"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let admin_token = response_json(admin_login).await["token"]
        .as_str()
        .expect("admin token")
        .to_string();

    let invite_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/users")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"operator@wara.local","name":"Operator","role":"operator"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invite_response.status(), StatusCode::OK);
    let invite_body = response_json(invite_response).await;
    assert_eq!(invite_body["user"]["status"], "invited");
    let invite_link = invite_body["invite_link"].as_str().expect("invite link");
    assert!(invite_link.starts_with("http://localhost:4200/accept-invite?token="));
    let token = invite_link
        .split("token=")
        .nth(1)
        .expect("invite token")
        .to_string();

    let accept_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/invites/accept")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"token":"{token}","password":"new-password"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(accept_response.status(), StatusCode::OK);
    let accept_body = response_json(accept_response).await;
    assert_eq!(accept_body["user"]["status"], "active");
    assert_eq!(accept_body["user"]["email"], "operator@wara.local");
    assert!(accept_body["token"].as_str().unwrap().split('.').count() == 3);

    let reuse_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/invites/accept")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"token":"{token}","password":"another-password"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reuse_response.status(), StatusCode::UNAUTHORIZED);

    drop_isolated_database(&test_database_url).await;
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("parse response JSON")
}

async fn create_isolated_database(base_url: &str) -> String {
    let db_name = format!("wara_test_{}", Uuid::now_v7().simple());
    let admin_url = replace_database_name(base_url, "postgres");
    let (client, connection) = tokio_postgres::connect(&admin_url, tokio_postgres::NoTls)
        .await
        .expect("connect postgres admin database");
    tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("postgres admin connection error: {error}");
        }
    });
    client
        .execute(&format!(r#"CREATE DATABASE "{}""#, db_name), &[])
        .await
        .expect("create isolated test database");
    replace_database_name(base_url, &db_name)
}

async fn drop_isolated_database(database_url: &str) {
    let db_name = database_name(database_url);
    let admin_url = replace_database_name(database_url, "postgres");
    let (client, connection) = tokio_postgres::connect(&admin_url, tokio_postgres::NoTls)
        .await
        .expect("connect postgres admin database");
    tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!("postgres admin connection error: {error}");
        }
    });
    client
        .execute(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1",
            &[&db_name],
        )
        .await
        .expect("terminate test database connections");
    client
        .execute(&format!(r#"DROP DATABASE IF EXISTS "{}""#, db_name), &[])
        .await
        .expect("drop isolated test database");
}

fn replace_database_name(database_url: &str, db_name: &str) -> String {
    let slash = database_url
        .rfind('/')
        .expect("database URL must contain a path");
    let query = database_url[slash + 1..]
        .find('?')
        .map(|index| slash + 1 + index);
    match query {
        Some(query) => format!(
            "{}{}{}",
            &database_url[..slash + 1],
            db_name,
            &database_url[query..]
        ),
        None => format!("{}{}", &database_url[..slash + 1], db_name),
    }
}

fn database_name(database_url: &str) -> String {
    let slash = database_url
        .rfind('/')
        .expect("database URL must contain a path");
    let value = &database_url[slash + 1..];
    value.split('?').next().unwrap_or(value).to_string()
}
