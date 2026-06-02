use std::process::Command;

use uuid::Uuid;
use wara_backend::{
    libs::{config::Config, db},
    services::{
        auth::{AuthService, CreateApiTokenInput},
        workspaces::WorkspaceService,
    },
};

/// Run the real `wara-migrate` binary against `database_url`. Cargo exposes the
/// built binary path via `CARGO_BIN_EXE_*`; the working directory is the backend
/// crate root so `Toasty.toml` and `toasty/` resolve.
fn run_migrate(database_url: &str, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_wara-migrate"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(args)
        .env("DATABASE_URL", database_url)
        .env("WARA_TELEMETRY_ENABLED", "false")
        .output()
        .expect("run wara-migrate")
}

/// Applying migrations to an empty database via the single migration binary must
/// produce a fully working schema (no `push_schema`), and re-running must be a
/// no-op.
#[tokio::test]
async fn migration_apply_initializes_a_fresh_database() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping migration integration test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };

    let test_database_url = create_isolated_database(&database_url).await;

    // First apply creates the schema.
    let output = run_migrate(&test_database_url, &["migration", "apply"]);
    assert!(
        output.status.success(),
        "first apply failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // The migrated schema must be usable with no push_schema in play.
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.db_push_schema = false;
    let bootstrap_email = config.bootstrap_admin_email.clone();
    let bootstrap_password = config.bootstrap_admin_password.clone();
    let database = db::connect(&config)
        .await
        .expect("connect to migrated database");
    let workspace_service = WorkspaceService::new(database.clone());
    let workspace = workspace_service
        .create_workspace(
            format!("migration-check-{}", Uuid::now_v7().simple()),
            Some("created against a migrated schema".to_string()),
        )
        .await
        .expect("create workspace on migrated schema");
    let environments = workspace_service
        .list_environments(workspace.id)
        .await
        .expect("list environments on migrated schema");
    assert!(
        !environments.is_empty(),
        "workspace creation should seed a default environment"
    );
    let auth_service = AuthService::new(database, config);
    auth_service
        .bootstrap_admin()
        .await
        .expect("bootstrap admin on migrated schema");
    let login = auth_service
        .login(wara_backend::services::auth::LoginInput {
            email: bootstrap_email,
            password: bootstrap_password,
        })
        .await
        .expect("login on migrated schema");
    let created_token = auth_service
        .create_api_token(CreateApiTokenInput {
            user: login.user.clone(),
            name: "migration token".to_string(),
        })
        .await
        .expect("create api token on migrated schema");
    assert!(created_token.token.starts_with("wara_"));
    let api_tokens = auth_service
        .list_api_tokens(&login.user)
        .await
        .expect("list api tokens on migrated schema");
    assert_eq!(api_tokens.len(), 1);

    // Re-applying is idempotent.
    let output = run_migrate(&test_database_url, &["migration", "apply"]);
    assert!(output.status.success(), "re-apply failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("up to date") || stdout.contains("No pending"),
        "second apply should be a no-op: {stdout}"
    );

    drop_isolated_database(&test_database_url).await;
}

async fn create_isolated_database(base_url: &str) -> String {
    let db_name = format!("wara_mig_test_{}", Uuid::now_v7().simple());
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
