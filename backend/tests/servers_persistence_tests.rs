use uuid::Uuid;
use wara_backend::{
    libs::{config::Config, db, docker::ProxyKind},
    services::servers::{CreateServerInput, ServerService},
};

#[tokio::test]
async fn servers_persist_with_encrypted_ssh_key_material() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping Toasty integration test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };

    let test_database_url = create_isolated_database(&database_url).await;
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.secret_key = "test-secret-key".to_string();
    config.db_push_schema = true;

    let database = db::connect(&config).await.expect("connect test database");
    let service = ServerService::new(database, config.secret_key.clone());
    let private_key =
        "-----BEGIN OPENSSH PRIVATE KEY-----\ntest-key\n-----END OPENSSH PRIVATE KEY-----";

    let created = service
        .create_server(CreateServerInput {
            name: "test-host".to_string(),
            host: "203.0.113.10".to_string(),
            port: Some(2222),
            username: "deploy".to_string(),
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAITest".to_string(),
            private_key: private_key.to_string(),
            private_key_passphrase: Some("key-passphrase".to_string()),
            default_proxy: Some(ProxyKind::Traefik),
        })
        .await
        .expect("create server");

    let loaded = service
        .get_server(created.id)
        .await
        .expect("load server by id");
    assert_eq!(loaded.name, "test-host");
    assert_eq!(loaded.host, "203.0.113.10");
    assert_eq!(loaded.port, 2222);
    assert_eq!(loaded.default_proxy.as_str(), "traefik");
    assert_eq!(loaded.docker_status, "unchecked");
    assert!(loaded.docker_version.is_empty());
    assert!(loaded.last_check_at.is_empty());
    assert!(loaded.last_check_error.is_empty());
    assert_ne!(loaded.encrypted_private_key, private_key);

    let ssh_target = service.ssh_target(&loaded).expect("build ssh target");
    assert_eq!(ssh_target.private_key, private_key);
    assert_eq!(
        ssh_target.private_key_passphrase.as_deref(),
        Some("key-passphrase")
    );

    let servers = service.list_servers().await.expect("list servers");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].id, loaded.id);

    let serialized = serde_json::to_string(&loaded).expect("serialize server response");
    assert!(serialized.contains("private_key_fingerprint"));
    assert!(!serialized.contains("encrypted_private_key"));
    assert!(!serialized.contains(private_key));
    assert!(!serialized.contains("key-passphrase"));

    let check = service
        .check_server(loaded.id)
        .await
        .expect("check server connectivity");
    assert_eq!(check.server_id, loaded.id);
    assert_eq!(check.ssh_status, "connected");
    assert_eq!(check.docker_status, "available");
    assert_eq!(check.docker_version.as_deref(), Some("25.0.0"));
    assert!(check.error.is_none());
    let checked = service
        .get_server(loaded.id)
        .await
        .expect("load checked server");
    assert_eq!(checked.docker_status, "available");
    assert_eq!(checked.docker_version, "25.0.0");
    assert!(!checked.last_check_at.is_empty());
    assert!(checked.last_check_error.is_empty());
    let check_json = serde_json::to_string(&check).expect("serialize check response");
    assert!(!check_json.contains(private_key));
    assert!(!check_json.contains("key-passphrase"));

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
