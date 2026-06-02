use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode, get_current_timestamp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;
use wara_backend::{
    libs::{config::Config, db, docker::DeployKind},
    models::users::{
        JwtPublicKeyRecord, Role, UserApiTokenRecord, UserRefreshTokenRecord,
        WorkspaceUserRoleRecord,
    },
    routes,
    services::{
        app_services::{AppServiceService, CreateAppServiceInput},
        auth::AuthService,
        workspaces::WorkspaceService,
    },
    state::AppState,
};

const OLD_JWT_KEY_ID: &str = "01973571-7a80-7000-8000-000000000002";
const UNKNOWN_JWT_KEY_ID: &str = "01973571-7a80-7000-8000-000000000003";
const MISSING_JWT_KEY_ID: &str = "01973571-7a80-7000-8000-000000000004";

#[derive(Debug, Serialize, Deserialize)]
struct TestClaims {
    jti: String,
    ret: String,
    sub: String,
    email: String,
    role: String,
    iss: String,
    aud: String,
    iat: u64,
    exp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct TestRefreshClaims {
    jti: String,
    sub: String,
    typ: String,
    iss: String,
    aud: String,
    iat: u64,
    exp: u64,
}

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
    let app = routes::router(AppState::new(config.clone(), database.clone()));

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
    let header = jsonwebtoken::decode_header(token).expect("decode JWT header");
    let active_key_id = header.kid.expect("active kid");
    assert!(Uuid::parse_str(&active_key_id).is_ok());
    let claims = decode_access_claims(&config, token);
    assert!(Uuid::parse_str(&claims.jti).is_ok());
    assert_eq!(
        claims.sub,
        login_body["user"]["id"].as_str().expect("user id")
    );
    assert_eq!(claims.email, "admin-auth@wara.local");
    assert_eq!(claims.role, "super_admin");
    assert_eq!(claims.iss, config.jwt_issuer);
    assert_eq!(claims.aud, config.jwt_audience);
    assert!(claims.exp > claims.iat);
    assert!(Uuid::parse_str(&claims.ret).is_ok());

    let jwks_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/.well-known/jwks.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(jwks_response.status(), StatusCode::OK);
    let jwks_body = response_json(jwks_response).await;
    let keys = jwks_body["keys"].as_array().expect("jwks keys");
    let active_key = keys
        .iter()
        .find(|key| key["kid"] == active_key_id)
        .expect("active key in jwks");
    assert_eq!(active_key["kty"], "RSA");
    assert_eq!(active_key["use"], "sig");
    assert_eq!(active_key["alg"], "RS256");
    assert!(active_key["n"].as_str().expect("modulus").len() > 100);
    assert_eq!(active_key["e"], "AQAB");

    let mut db_handle = database.handle().expect("database handle");
    toasty::create!(JwtPublicKeyRecord {
        id: Uuid::parse_str(OLD_JWT_KEY_ID).expect("old key id"),
        key_type: "RSA".to_string(),
        key_use: "sig".to_string(),
        algorithm: "RS256".to_string(),
        modulus: active_key["n"].as_str().expect("modulus").to_string(),
        exponent: active_key["e"].as_str().expect("exponent").to_string(),
        is_active: false,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
    .exec(&mut db_handle)
    .await
    .expect("insert inactive old JWT public key");

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
    assert_eq!(me_body["role"], "super_admin");
    let user_id = me_body["id"].as_str().expect("user id");

    let old_key_token = sign_test_jwt(
        &config,
        OLD_JWT_KEY_ID,
        user_id,
        "admin-auth@wara.local",
        "super_admin",
        true,
    );
    let old_key_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {old_key_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_key_response.status(), StatusCode::OK);

    let unknown_key_token = sign_test_jwt(
        &config,
        UNKNOWN_JWT_KEY_ID,
        user_id,
        "admin-auth@wara.local",
        "super_admin",
        true,
    );
    let unknown_key_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {unknown_key_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_key_response.status(), StatusCode::UNAUTHORIZED);

    let missing_key_id_token = sign_test_jwt(
        &config,
        MISSING_JWT_KEY_ID,
        user_id,
        "admin-auth@wara.local",
        "super_admin",
        false,
    );
    let missing_key_id_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {missing_key_id_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_key_id_response.status(), StatusCode::UNAUTHORIZED);

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
async fn refresh_tokens_are_identified_by_jti_rotated_expirable_and_revocable() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping Toasty integration test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };

    let test_database_url = create_isolated_database(&database_url).await;
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.db_push_schema = true;
    config.bootstrap_admin_email = "refresh-admin@wara.local".to_string();
    config.bootstrap_admin_password = "correct-password".to_string();
    config.bootstrap_admin_name = "Refresh Admin".to_string();

    let database = db::connect(&config).await.expect("connect test database");
    AuthService::new(database.clone(), config.clone())
        .bootstrap_admin()
        .await
        .expect("bootstrap admin");
    let app = routes::router(AppState::new(config.clone(), database.clone()));

    let first_login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"refresh-admin@wara.local","password":"correct-password"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_login.status(), StatusCode::OK);
    let first_login_body = response_json(first_login).await;
    let first_access_token = first_login_body["token"].as_str().expect("access token");
    let active_key_id = jsonwebtoken::decode_header(first_access_token)
        .expect("decode JWT header")
        .kid
        .expect("active kid");
    let user_id = Uuid::parse_str(
        first_login_body["user"]["id"]
            .as_str()
            .expect("login user id"),
    )
    .expect("parse user id");
    let expired_refresh_token = first_login_body["refresh_token"]
        .as_str()
        .expect("refresh token")
        .to_string();
    assert_eq!(expired_refresh_token.split('.').count(), 3);

    let mut db_handle = database.handle().expect("database handle");
    let issued_records = toasty::stmt::Query::<toasty::stmt::List<UserRefreshTokenRecord>>::filter(
        UserRefreshTokenRecord::fields().user_id().eq(user_id),
    )
    .exec(&mut db_handle)
    .await
    .expect("list refresh token records");
    assert_eq!(issued_records.len(), 1);
    let first_claims = decode_access_claims(&config, first_access_token);
    assert!(Uuid::parse_str(&first_claims.jti).is_ok());
    assert_eq!(first_claims.ret, issued_records[0].id.to_string());
    let first_refresh_claims = decode_refresh_claims(&config, &expired_refresh_token);
    assert_eq!(first_refresh_claims.jti, issued_records[0].id.to_string());
    assert_eq!(first_refresh_claims.sub, user_id.to_string());
    assert_eq!(first_refresh_claims.typ, "refresh");
    assert_eq!(first_refresh_claims.iss, config.jwt_issuer);
    assert_eq!(first_refresh_claims.aud, config.jwt_audience);
    assert!(first_refresh_claims.exp > first_refresh_claims.iat);

    let mut expire_token = toasty::stmt::Update::<toasty::stmt::List<UserRefreshTokenRecord>>::new(
        toasty::stmt::Query::<toasty::stmt::List<UserRefreshTokenRecord>>::filter(
            UserRefreshTokenRecord::fields()
                .id()
                .eq(issued_records[0].id),
        ),
    );
    expire_token.set(3, chrono::Utc::now().to_rfc3339());
    expire_token.set_returning_none();
    expire_token
        .exec(&mut db_handle)
        .await
        .expect("expire refresh token");

    let expired_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"refresh_token":"{expired_refresh_token}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(expired_response.status(), StatusCode::UNAUTHORIZED);

    let malformed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"refresh_token":"not-a-refresh-token"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malformed_response.status(), StatusCode::UNAUTHORIZED);

    let unknown_refresh_token = sign_unknown_refresh_token(&config, user_id, &active_key_id);
    let unknown_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"refresh_token":"{unknown_refresh_token}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_response.status(), StatusCode::UNAUTHORIZED);

    let tampered_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"refresh_token":"{expired_refresh_token}tampered"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tampered_response.status(), StatusCode::UNAUTHORIZED);

    let active_login_body =
        login_response(&app, "refresh-admin@wara.local", "correct-password").await;
    let active_access_token = active_login_body["token"]
        .as_str()
        .expect("active access token");
    let active_refresh_token = active_login_body["refresh_token"]
        .as_str()
        .expect("active refresh token")
        .to_string();

    let refresh_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"refresh_token":"{active_refresh_token}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let refresh_body = response_json(refresh_response).await;
    let rotated_access_token = refresh_body["token"]
        .as_str()
        .expect("rotated access token");
    let rotated_refresh_token = refresh_body["refresh_token"]
        .as_str()
        .expect("rotated refresh token")
        .to_string();
    assert_eq!(rotated_access_token.split('.').count(), 3);
    assert_eq!(rotated_refresh_token.split('.').count(), 3);
    assert_ne!(active_refresh_token, rotated_refresh_token);
    assert!(
        !refresh_body.to_string().contains(&active_refresh_token),
        "refresh response must not return the old plaintext refresh token"
    );

    let records = toasty::stmt::Query::<toasty::stmt::List<UserRefreshTokenRecord>>::filter(
        UserRefreshTokenRecord::fields().user_id().eq(user_id),
    )
    .exec(&mut db_handle)
    .await
    .expect("list rotated refresh token records");
    let active_claims = decode_access_claims(&config, active_access_token);
    let revoked_old = records
        .iter()
        .find(|record| record.id.to_string() == active_claims.ret)
        .expect("old refresh token record");
    assert!(revoked_old.revoked_at.is_some());
    assert_eq!(active_claims.ret, revoked_old.id.to_string());
    let active_refresh_claims = decode_refresh_claims(&config, &active_refresh_token);
    assert_eq!(active_refresh_claims.jti, active_claims.ret);
    let rotated_claims = decode_access_claims(&config, rotated_access_token);
    let active_new = records
        .iter()
        .find(|record| record.id.to_string() == rotated_claims.ret)
        .expect("rotated refresh token record");
    assert!(active_new.revoked_at.is_none());
    let rotated_refresh_claims = decode_refresh_claims(&config, &rotated_refresh_token);
    assert_eq!(rotated_refresh_claims.jti, rotated_claims.ret);
    assert_eq!(rotated_refresh_claims.sub, user_id.to_string());
    assert!(Uuid::parse_str(&rotated_claims.jti).is_ok());
    assert_ne!(rotated_claims.jti, active_claims.jti);
    assert_eq!(rotated_claims.ret, active_new.id.to_string());

    let reused_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"refresh_token":"{active_refresh_token}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reused_response.status(), StatusCode::UNAUTHORIZED);

    let logout_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"refresh_token":"{rotated_refresh_token}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

    let revoked_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"refresh_token":"{rotated_refresh_token}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked_response.status(), StatusCode::UNAUTHORIZED);

    drop_isolated_database(&test_database_url).await;
}

#[tokio::test]
async fn api_tokens_are_shown_once_hashed_revocable_and_authorized_like_users() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping Toasty integration test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };

    let test_database_url = create_isolated_database(&database_url).await;
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.db_push_schema = true;
    config.bootstrap_admin_email = "api-token-admin@wara.local".to_string();
    config.bootstrap_admin_password = "correct-password".to_string();
    config.bootstrap_admin_name = "API Token Admin".to_string();
    config.app_base_url = "http://localhost:4200".to_string();

    let database = db::connect(&config).await.expect("connect test database");
    AuthService::new(database.clone(), config.clone())
        .bootstrap_admin()
        .await
        .expect("bootstrap admin");
    let workspace_service = WorkspaceService::new(database.clone());
    let workspace = workspace_service
        .create_workspace("API token workspace".to_string(), None)
        .await
        .expect("create workspace");
    let other_workspace = workspace_service
        .create_workspace("Other API token workspace".to_string(), None)
        .await
        .expect("create other workspace");
    let app = routes::router(AppState::new(config, database.clone()));

    let root_token = login(&app, "api-token-admin@wara.local", "correct-password").await;
    let viewer_jwt = invite_accept_and_token(
        &app,
        &root_token,
        workspace.id,
        "api-token-viewer@wara.local",
        Role::Viewer,
    )
    .await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/api-tokens")
                .header("authorization", format!("Bearer {viewer_jwt}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Local automation"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = response_json(create_response).await;
    let api_token = create_body["token"]
        .as_str()
        .expect("plaintext api token")
        .to_string();
    assert!(api_token.starts_with("wara_"));
    assert_eq!(create_body["api_token"]["name"], "Local automation");
    assert_eq!(
        create_body["api_token"]["token_prefix"],
        api_token.chars().take(12).collect::<String>()
    );
    assert!(create_body["api_token"].get("token").is_none());
    let token_id = Uuid::parse_str(
        create_body["api_token"]["id"]
            .as_str()
            .expect("api token id"),
    )
    .expect("parse api token id");

    let mut db_handle = database.handle().expect("database handle");
    let records = toasty::stmt::Query::<toasty::stmt::List<UserApiTokenRecord>>::filter(
        UserApiTokenRecord::fields().id().eq(token_id),
    )
    .exec(&mut db_handle)
    .await
    .expect("list api token records");
    assert_eq!(records.len(), 1);
    assert_ne!(records[0].token_hash, api_token);
    assert!(
        !records[0].token_hash.contains(&api_token),
        "plaintext token must not be stored"
    );

    let me_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {api_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_response.status(), StatusCode::OK);
    let me_body = response_json(me_response).await;
    assert_eq!(me_body["email"], "api-token-viewer@wara.local");

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/api-tokens")
                .header("authorization", format!("Bearer {api_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = response_json(list_response).await;
    assert_eq!(list_body.as_array().expect("token list").len(), 1);
    assert_eq!(list_body[0]["id"], token_id.to_string());
    assert_eq!(list_body[0]["name"], "Local automation");
    assert!(
        !list_body.to_string().contains(&api_token),
        "list response must not include plaintext token"
    );

    let forbidden_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/workspaces/{}", other_workspace.id))
                .header("authorization", format!("Bearer {api_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN);

    let malformed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", "Bearer not-an-api-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malformed_response.status(), StatusCode::UNAUTHORIZED);

    let unknown_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", "Bearer wara_unknown-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_response.status(), StatusCode::UNAUTHORIZED);

    let revoke_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/auth/api-tokens/{token_id}"))
                .header("authorization", format!("Bearer {api_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoke_response.status(), StatusCode::OK);
    let revoke_body = response_json(revoke_response).await;
    assert!(revoke_body["revoked_at"].as_str().is_some());
    assert!(
        !revoke_body.to_string().contains(&api_token),
        "revoke response must not include plaintext token"
    );

    let revoked_response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("authorization", format!("Bearer {api_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked_response.status(), StatusCode::UNAUTHORIZED);

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
    let workspace = WorkspaceService::new(database.clone())
        .create_workspace("Invited workspace".to_string(), None)
        .await
        .expect("create workspace");
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

    let missing_workspace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/users")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"workspace_id":"{}","email":"missing-workspace@wara.local","name":"Missing Workspace","role":"operator"}}"#,
                    Uuid::now_v7()
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_workspace_response.status(), StatusCode::NOT_FOUND);

    let invite_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/users")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"workspace_id":"{}","email":"operator@wara.local","name":"Operator","role":"operator"}}"#,
                    workspace.id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invite_response.status(), StatusCode::OK);
    let invite_body = response_json(invite_response).await;
    assert_eq!(invite_body["user"]["status"], "invited");
    assert_eq!(invite_body["user"]["role"], "viewer");
    assert_eq!(
        invite_body["user"]["workspace_roles"][0]["workspace_id"],
        workspace.id.to_string()
    );
    assert_eq!(
        invite_body["user"]["workspace_roles"][0]["role"],
        "operator"
    );
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
    assert_eq!(
        accept_body["user"]["workspace_roles"][0]["workspace_id"],
        workspace.id.to_string()
    );
    assert_eq!(
        accept_body["user"]["workspace_roles"][0]["role"],
        "operator"
    );
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

#[tokio::test]
async fn workspace_admin_can_invite_only_to_their_workspace() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping Toasty integration test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };

    let test_database_url = create_isolated_database(&database_url).await;
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.db_push_schema = true;
    config.bootstrap_admin_email = "workspace-admin-root@wara.local".to_string();
    config.bootstrap_admin_password = "correct-password".to_string();
    config.bootstrap_admin_name = "Workspace Admin Root".to_string();

    let database = db::connect(&config).await.expect("connect test database");
    AuthService::new(database.clone(), config.clone())
        .bootstrap_admin()
        .await
        .expect("bootstrap admin");
    let workspace_service = WorkspaceService::new(database.clone());
    let workspace = workspace_service
        .create_workspace("Scoped workspace".to_string(), None)
        .await
        .expect("create workspace");
    let other_workspace = workspace_service
        .create_workspace("Other workspace".to_string(), None)
        .await
        .expect("create other workspace");
    let environment = workspace_service
        .list_environments(workspace.id)
        .await
        .expect("list environments")
        .into_iter()
        .next()
        .expect("workspace environment");
    let other_environment = workspace_service
        .list_environments(other_workspace.id)
        .await
        .expect("list other environments")
        .into_iter()
        .next()
        .expect("other workspace environment");
    let app_service = AppServiceService::new(database.clone());
    let service = app_service
        .create_service(CreateAppServiceInput {
            workspace_id: workspace.id,
            environment_id: environment.id,
            name: "web".to_string(),
            deploy_kind: DeployKind::DockerImage,
            image: Some("ghcr.io/acme/web:latest".to_string()),
            compose_file: None,
            dockerfile: None,
            internal_port: Some(8080),
        })
        .await
        .expect("create service");
    let other_service = app_service
        .create_service(CreateAppServiceInput {
            workspace_id: other_workspace.id,
            environment_id: other_environment.id,
            name: "api".to_string(),
            deploy_kind: DeployKind::DockerImage,
            image: Some("ghcr.io/acme/api:latest".to_string()),
            compose_file: None,
            dockerfile: None,
            internal_port: Some(8081),
        })
        .await
        .expect("create other service");
    let app = routes::router(AppState::new(config, database.clone()));

    let root_token = login(&app, "workspace-admin-root@wara.local", "correct-password").await;
    let workspace_admin_token = invite_accept_and_token(
        &app,
        &root_token,
        workspace.id,
        "workspace-admin@wara.local",
        Role::Admin,
    )
    .await;

    let allowed_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/users")
                .header("authorization", format!("Bearer {workspace_admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"workspace_id":"{}","email":"scoped-operator@wara.local","name":"Scoped Operator","role":"operator"}}"#,
                    workspace.id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed_response.status(), StatusCode::OK);

    let same_workspace_service_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/services/{}", service.id))
                .header("authorization", format!("Bearer {workspace_admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(same_workspace_service_response.status(), StatusCode::OK);

    let bulk_env_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workspaces/{}/env-vars/bulk", workspace.id))
                .header("authorization", format!("Bearer {workspace_admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{
                        "env_vars": [
                            {{"environment_id":"{}","key":"DATABASE_URL","value":"postgres://secret"}},
                            {{"service_id":"{}","key":"REDIS_URL","value":"redis://secret"}}
                        ]
                    }}"#,
                    environment.id, service.id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bulk_env_response.status(), StatusCode::OK);
    let bulk_env_body = response_json(bulk_env_response).await;
    let bulk_env_vars = bulk_env_body.as_array().expect("bulk env var response");
    assert_eq!(bulk_env_vars.len(), 2);
    assert_eq!(bulk_env_vars[0]["workspace_id"], workspace.id.to_string());
    assert_eq!(bulk_env_vars[0]["value"], "********");
    assert_eq!(bulk_env_vars[1]["service_id"], service.id.to_string());
    assert_eq!(bulk_env_vars[1]["value"], "********");

    let cross_workspace_bulk_env_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workspaces/{}/env-vars/bulk", workspace.id))
                .header("authorization", format!("Bearer {workspace_admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{
                        "env_vars": [
                            {{"service_id":"{}","key":"OTHER_SERVICE","value":"secret"}}
                        ]
                    }}"#,
                    other_service.id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        cross_workspace_bulk_env_response.status(),
        StatusCode::NOT_FOUND
    );

    let other_workspace_service_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/services/{}", other_service.id))
                .header("authorization", format!("Bearer {workspace_admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        other_workspace_service_response.status(),
        StatusCode::FORBIDDEN
    );

    let cross_workspace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/users")
                .header("authorization", format!("Bearer {workspace_admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"workspace_id":"{}","email":"cross-workspace@wara.local","name":"Cross Workspace","role":"operator"}}"#,
                    other_workspace.id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cross_workspace_response.status(), StatusCode::FORBIDDEN);

    let viewer_token = invite_accept_and_token(
        &app,
        &root_token,
        workspace.id,
        "viewer@wara.local",
        Role::Viewer,
    )
    .await;
    let viewer_workspaces_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/workspaces")
                .header("authorization", format!("Bearer {viewer_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(viewer_workspaces_response.status(), StatusCode::OK);
    let viewer_workspaces = response_json(viewer_workspaces_response).await;
    assert_eq!(
        viewer_workspaces
            .as_array()
            .expect("workspaces array")
            .len(),
        1
    );
    assert_eq!(viewer_workspaces[0]["id"], workspace.id.to_string());

    let viewer_other_workspace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/workspaces/{}", other_workspace.id))
                .header("authorization", format!("Bearer {viewer_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        viewer_other_workspace_response.status(),
        StatusCode::FORBIDDEN
    );

    let viewer_create_workspace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workspaces")
                .header("authorization", format!("Bearer {viewer_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Viewer Workspace"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        viewer_create_workspace_response.status(),
        StatusCode::FORBIDDEN
    );

    let workspace_admin_servers_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/servers")
                .header("authorization", format!("Bearer {workspace_admin_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        workspace_admin_servers_response.status(),
        StatusCode::FORBIDDEN
    );
    let forbidden_body = response_json(workspace_admin_servers_response).await;
    assert_eq!(forbidden_body["code"], "forbidden");
    assert_eq!(forbidden_body["message"], "forbidden");

    let viewer_invite_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/users")
                .header("authorization", format!("Bearer {viewer_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"workspace_id":"{}","email":"viewer-created@wara.local","name":"Viewer Created","role":"operator"}}"#,
                    workspace.id
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(viewer_invite_response.status(), StatusCode::FORBIDDEN);

    let mut db = database.handle().expect("database handle");
    let scoped_roles = toasty::stmt::Query::<toasty::stmt::List<WorkspaceUserRoleRecord>>::filter(
        WorkspaceUserRoleRecord::fields()
            .workspace_id()
            .eq(workspace.id),
    )
    .exec(&mut db)
    .await
    .expect("list workspace roles");
    assert!(scoped_roles.len() >= 3);

    drop_isolated_database(&test_database_url).await;
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).expect("parse response JSON")
}

fn decode_access_claims(config: &Config, token: &str) -> TestClaims {
    let key = DecodingKey::from_rsa_pem(config.jwt_public_key.as_bytes())
        .expect("test public key should parse");
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[config.jwt_audience.as_str()]);
    validation.set_issuer(&[config.jwt_issuer.as_str()]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    decode::<TestClaims>(token, &key, &validation)
        .expect("access token should decode")
        .claims
}

fn decode_refresh_claims(config: &Config, token: &str) -> TestRefreshClaims {
    let key = DecodingKey::from_rsa_pem(config.jwt_public_key.as_bytes())
        .expect("test public key should parse");
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[config.jwt_audience.as_str()]);
    validation.set_issuer(&[config.jwt_issuer.as_str()]);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub", "jti"]);
    decode::<TestRefreshClaims>(token, &key, &validation)
        .expect("refresh token should decode")
        .claims
}

fn sign_unknown_refresh_token(config: &Config, user_id: Uuid, key_id: &str) -> String {
    let now = get_current_timestamp();
    let claims = TestRefreshClaims {
        jti: Uuid::now_v7().to_string(),
        sub: user_id.to_string(),
        typ: "refresh".to_string(),
        iss: config.jwt_issuer.clone(),
        aud: config.jwt_audience.clone(),
        iat: now,
        exp: now + config.refresh_token_ttl_seconds,
    };
    let key = EncodingKey::from_rsa_pem(config.jwt_private_key_pem.as_bytes())
        .expect("test private key should parse");
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(key_id.to_string());
    encode(&header, &claims, &key).expect("sign refresh token")
}

fn sign_test_jwt(
    config: &Config,
    key_id: &str,
    user_id: &str,
    email: &str,
    role: &str,
    include_key_id: bool,
) -> String {
    let now = get_current_timestamp();
    let claims = TestClaims {
        jti: Uuid::now_v7().to_string(),
        ret: Uuid::now_v7().to_string(),
        sub: user_id.to_string(),
        email: email.to_string(),
        role: role.to_string(),
        iss: config.jwt_issuer.clone(),
        aud: config.jwt_audience.clone(),
        iat: now,
        exp: now + config.jwt_access_token_ttl_seconds,
    };
    let mut header = Header::new(Algorithm::RS256);
    if include_key_id {
        header.kid = Some(key_id.to_string());
    }
    let key = EncodingKey::from_rsa_pem(config.jwt_private_key_pem.as_bytes())
        .expect("test private key should parse");
    encode(&header, &claims, &key).expect("sign test JWT")
}

async fn login(app: &axum::Router, email: &str, password: &str) -> String {
    login_response(app, email, password).await["token"]
        .as_str()
        .expect("login token")
        .to_string()
}

async fn login_response(app: &axum::Router, email: &str, password: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"email":"{email}","password":"{password}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}

async fn invite_accept_and_token(
    app: &axum::Router,
    inviter_token: &str,
    workspace_id: Uuid,
    email: &str,
    role: Role,
) -> String {
    let invite_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/users")
                .header("authorization", format!("Bearer {inviter_token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"workspace_id":"{workspace_id}","email":"{email}","name":"Scoped User","role":"{}"}}"#,
                    role.as_str()
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invite_response.status(), StatusCode::OK);
    let invite_body = response_json(invite_response).await;
    let token = invite_body["invite_link"]
        .as_str()
        .expect("invite link")
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
    response_json(accept_response).await["token"]
        .as_str()
        .expect("accepted user token")
        .to_string()
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
