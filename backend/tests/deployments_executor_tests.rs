use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;
use wara_backend::{
    errors::wara::WaraError,
    libs::{
        config::Config,
        db,
        deploy_executor::{CommandOutcome, DeployExecutor, DeployRun},
        docker::{DeployKind, RemoteCommand},
    },
    services::{
        app_services::{AppServiceService, CreateAppServiceInput},
        deployments::DeploymentService,
        workspaces::WorkspaceService,
    },
};

/// A test executor that returns a scripted run and records the commands it was
/// asked to run. Stands in for a real SSH executor.
struct ScriptedExecutor {
    run: DeployRun,
    recorded: Arc<std::sync::Mutex<Vec<String>>>,
}

#[async_trait]
impl DeployExecutor for ScriptedExecutor {
    async fn run(&self, commands: &[RemoteCommand]) -> Result<DeployRun, WaraError> {
        let mut recorded = self.recorded.lock().unwrap();
        *recorded = commands.iter().map(RemoteCommand::render).collect();
        Ok(self.run.clone())
    }
}

async fn seed_service(config: &Config) -> (db::Database, Uuid) {
    let database = db::connect(config).await.expect("connect test database");
    let workspace = WorkspaceService::new(database.clone())
        .create_workspace(
            format!("deploy-exec-{}", Uuid::now_v7().simple()),
            Some("executor test".to_string()),
        )
        .await
        .expect("create workspace");
    let environment = WorkspaceService::new(database.clone())
        .list_environments(workspace.id)
        .await
        .expect("list environments")
        .into_iter()
        .next()
        .expect("default environment");
    let service = AppServiceService::new(database.clone())
        .create_service(CreateAppServiceInput {
            workspace_id: workspace.id,
            environment_id: environment.id,
            name: "web".to_string(),
            deploy_kind: DeployKind::DockerImage,
            image: Some("nginx:latest".to_string()),
            compose_file: None,
            dockerfile: None,
            internal_port: Some(8080),
        })
        .await
        .expect("create app service");
    (database, service.id)
}

#[tokio::test]
async fn trigger_deploy_captures_failure_output_and_marks_failed() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping deploy executor test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };
    let test_database_url = create_isolated_database(&database_url).await;
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.db_push_schema = true;

    let (database, service_id) = seed_service(&config).await;

    let executor = ScriptedExecutor {
        run: DeployRun {
            outcomes: vec![CommandOutcome {
                command: "docker pull 'nginx:latest'".to_string(),
                stdout: String::new(),
                stderr: "Error response from daemon: manifest unknown".to_string(),
                exit_code: 1,
            }],
            executed: true,
            success: false,
        },
        recorded: Arc::new(std::sync::Mutex::new(Vec::new())),
    };

    let deployment = DeploymentService::with_executor(database, config, Arc::new(executor))
        .trigger_deploy(service_id)
        .await
        .expect("trigger deploy");

    // Failure from the executor surfaces as the deployment's status and output.
    assert_eq!(deployment.status.as_str(), "failed");
    assert!(
        deployment
            .output
            .contains("Error response from daemon: manifest unknown"),
        "captured stderr should be in the deployment output: {}",
        deployment.output
    );
    assert!(deployment.output.contains("[exit 1]"));

    drop_isolated_database(&test_database_url).await;
}

#[tokio::test]
async fn trigger_deploy_passes_quoted_commands_to_executor() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping deploy executor test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };
    let test_database_url = create_isolated_database(&database_url).await;
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.db_push_schema = true;

    let (database, service_id) = seed_service(&config).await;

    let executor = ScriptedExecutor {
        run: DeployRun {
            outcomes: Vec::new(),
            executed: true,
            success: true,
        },
        recorded: Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let recorded = executor.recorded.clone();

    let deployment = DeploymentService::with_executor(database, config, Arc::new(executor))
        .trigger_deploy(service_id)
        .await
        .expect("trigger deploy");

    assert_eq!(deployment.status.as_str(), "succeeded");
    let commands = recorded.lock().unwrap().clone();
    // The executor only ever receives rendered, shell-safe commands.
    assert_eq!(commands[0], "docker pull 'nginx:latest'");
    assert!(commands.iter().all(|c| c.contains('\'')));

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
