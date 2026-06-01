use uuid::Uuid;
use wara_backend::{
    libs::{
        config::Config,
        db,
        docker::{DeployKind, ProxyKind},
    },
    services::{
        app_services::{AppServiceService, CreateAppServiceInput},
        credentials::{CreateCredentialInput, CreateEnvVarInput, CredentialService},
        deployments::DeploymentService,
        domains::{CreateDomainInput, DomainService},
        project_templates::{
            CreateProjectsFromTemplateInput, CreateTemplateInput, ProjectTemplateService,
        },
        projects::ProjectService,
        templates::SecretCopyMode,
    },
};

#[tokio::test]
async fn product_resources_persist_with_toasty() {
    let Some(database_url) = Config::from_env().test_database_url else {
        eprintln!("skipping Toasty integration test; set WARA_TEST_DATABASE_URL to run it");
        return;
    };

    let test_database_url = create_isolated_database(&database_url).await;
    let mut config = Config::from_env();
    config.database_url = test_database_url.clone();
    config.secret_key = "test-secret-key".to_string();
    config.remote_services_root = "/srv/wara/apps".to_string();
    config.dockerfile_context_dir = "source".to_string();
    config.db_push_schema = true;

    let database = db::connect(&config).await.expect("connect test database");
    let project_service = ProjectService::new(database.clone());
    let app_service = AppServiceService::new(database.clone());
    let credential_service = CredentialService::new(database.clone(), config.secret_key.clone());
    let domain_service = DomainService::new(database.clone());
    let deployment_service = DeploymentService::new(database.clone(), config.clone());
    let template_service = ProjectTemplateService::new(database);

    let project = project_service
        .create_project(
            format!("resource-audit-{}", Uuid::now_v7().simple()),
            Some("resource persistence test".to_string()),
        )
        .await
        .expect("create project");
    let environment = project_service
        .list_environments(project.id)
        .await
        .expect("list environments")
        .into_iter()
        .next()
        .expect("default environment");

    let service = app_service
        .create_service(CreateAppServiceInput {
            project_id: project.id,
            environment_id: environment.id,
            name: "web".to_string(),
            deploy_kind: DeployKind::Dockerfile,
            image: None,
            compose_file: None,
            dockerfile: Some("FROM nginx:alpine".to_string()),
            internal_port: Some(8080),
        })
        .await
        .expect("create app service");
    assert_eq!(service.project_id, project.id);

    let credential = credential_service
        .create_credential(CreateCredentialInput {
            project_id: project.id,
            registry: "ghcr.io".to_string(),
            username: "deploy".to_string(),
            password: "registry-password".to_string(),
        })
        .await
        .expect("create credential");
    assert_eq!(credential.password, "********");

    let env_var = credential_service
        .create_env_var(CreateEnvVarInput {
            project_id: project.id,
            environment_id: Some(environment.id),
            service_id: Some(service.id),
            key: "DATABASE_URL".to_string(),
            value: "postgres://secret".to_string(),
        })
        .await
        .expect("create env var");
    assert_eq!(env_var.value, "********");
    assert!(
        !serde_json::to_string(&env_var)
            .expect("serialize env var")
            .contains("postgres://secret")
    );

    let domain = domain_service
        .create_domain(CreateDomainInput {
            service_id: service.id,
            hostname: "app.example.com".to_string(),
            proxy: Some(ProxyKind::Traefik),
            tls_enabled: Some(true),
        })
        .await
        .expect("create domain");
    assert_eq!(domain.proxy.as_str(), "traefik");

    let deployment = deployment_service
        .trigger_deploy(service.id)
        .await
        .expect("trigger deploy");
    assert!(deployment.output.contains("/srv/wara/apps/web/source"));
    assert_eq!(
        deployment_service
            .list_deployments(service.id)
            .await
            .expect("list deployments")
            .len(),
        1
    );

    let template = template_service
        .create_template(CreateTemplateInput {
            project_id: project.id,
            name: "web-template".to_string(),
            description: Some("duplicated from template".to_string()),
        })
        .await
        .expect("create template");
    let duplicated = template_service
        .create_projects_from_template(CreateProjectsFromTemplateInput {
            template_id: template.id,
            names: vec!["copy-one".to_string(), "copy-two".to_string()],
            secret_copy_mode: SecretCopyMode::Empty,
        })
        .await
        .expect("duplicate projects from template");
    assert_eq!(duplicated.len(), 2);

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
