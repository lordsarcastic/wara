use uuid::Uuid;
use wara_backend::{
    libs::{config::Config, db},
    services::workspaces::WorkspaceService,
};

#[tokio::test]
async fn workspaces_and_default_environment_persist_with_toasty_when_database_is_configured() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping Toasty integration test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };

    let test_database_url = create_isolated_database(&database_url).await;
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.db_push_schema = true;
    let database = db::connect(&config).await.expect("connect test database");
    let service = WorkspaceService::new(database);

    let workspace = service
        .create_workspace(
            format!("audit-{}", uuid::Uuid::now_v7().simple()),
            Some("created by persistence integration test".to_string()),
        )
        .await
        .expect("create workspace");

    let loaded = service
        .get_workspace(workspace.id)
        .await
        .expect("load workspace");
    assert_eq!(loaded.id, workspace.id);
    assert_eq!(loaded.name, workspace.name);

    let environments = service
        .list_environments(workspace.id)
        .await
        .expect("list environments");
    assert_eq!(environments.len(), 1);
    assert_eq!(environments[0].name, "production");

    drop_isolated_database(&test_database_url).await;
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
