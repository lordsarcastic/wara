use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;
use wara_backend::{
    libs::{config::Config, db, docker::DeployKind},
    models::users::{Role, WorkspaceUserRoleRecord},
    routes,
    services::{
        app_services::{AppServiceService, CreateAppServiceInput},
        auth::AuthService,
        workspaces::WorkspaceService,
    },
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
    assert_eq!(me_body["role"], "super_admin");

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

async fn login(app: &axum::Router, email: &str, password: &str) -> String {
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
    response_json(response).await["token"]
        .as_str()
        .expect("login token")
        .to_string()
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
