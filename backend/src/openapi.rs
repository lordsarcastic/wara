use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::health,
        crate::routes::auth::login,
        crate::routes::auth::refresh_session,
        crate::routes::auth::logout,
        crate::routes::auth::accept_invite,
        crate::routes::auth::me,
        crate::routes::auth::list_api_tokens,
        crate::routes::auth::create_api_token,
        crate::routes::auth::revoke_api_token,
        crate::routes::servers::list_servers,
        crate::routes::servers::create_server,
        crate::routes::servers::get_server,
        crate::routes::workspaces::list_workspaces,
        crate::routes::workspaces::create_workspace,
        crate::routes::workspaces::get_workspace,
        crate::routes::environments::list_environments,
        crate::routes::environments::create_environment,
        crate::routes::services::list_services,
        crate::routes::services::create_service,
        crate::routes::services::get_service,
        crate::routes::credentials::list_credentials,
        crate::routes::credentials::create_credential,
        crate::routes::credentials::list_env_vars,
        crate::routes::credentials::create_env_var,
        crate::routes::credentials::create_env_vars,
        crate::routes::domains::list_domains,
        crate::routes::domains::create_domain,
        crate::routes::domains::preview_proxy,
        crate::routes::deployments::list_deployments,
        crate::routes::deployments::trigger_deploy,
        crate::routes::deployments::get_deployment,
        crate::routes::deployments::restart_service,
        crate::routes::deployments::service_logs,
        crate::routes::templates::list_workspace_templates,
        crate::routes::templates::create_template,
        crate::routes::templates::create_workspaces_from_template,
        crate::routes::telemetry::get_settings,
        crate::routes::telemetry::update_settings,
        crate::routes::admin::list_users,
        crate::routes::admin::invite_user,
        crate::routes::admin::disable_user,
        crate::routes::admin::reactivate_user,
        crate::routes::admin::change_user_role
    ),
    components(
        schemas(
            crate::errors::ErrorResponse,
            crate::models::users::User,
            crate::models::users::Role,
            crate::models::users::WorkspaceRole,
            crate::models::users::UserStatus,
            crate::models::servers::Server,
            crate::models::workspaces::Workspace,
            crate::models::environments::Environment,
            crate::models::services::AppService,
            crate::models::credentials::DockerCredential,
            crate::models::credentials::EnvVar,
            crate::models::domains::Domain,
            crate::models::deployments::Deployment,
            crate::models::deployments::DeploymentStatus,
            crate::models::templates::WorkspaceTemplate,
            crate::libs::docker::DeployKind,
            crate::libs::docker::ProxyKind,
            crate::routes::auth::LoginRequest,
            crate::routes::auth::LoginResponse,
            crate::routes::auth::RefreshSessionRequest,
            crate::routes::auth::LogoutRequest,
            crate::routes::auth::AcceptInviteRequest,
            crate::routes::auth::CreateApiTokenRequest,
            crate::routes::auth::ApiTokenResponse,
            crate::routes::auth::CreateApiTokenResponse,
            crate::routes::servers::CreateServerRequest,
            crate::routes::workspaces::CreateWorkspaceRequest,
            crate::routes::environments::CreateEnvironmentRequest,
            crate::routes::services::CreateServiceRequest,
            crate::routes::credentials::CreateCredentialRequest,
            crate::routes::credentials::CredentialResponse,
            crate::routes::credentials::CreateEnvVarRequest,
            crate::routes::credentials::CreateEnvVarsRequest,
            crate::routes::credentials::EnvVarResponse,
            crate::routes::domains::CreateDomainRequest,
            crate::routes::domains::ProxyPreviewResponse,
            crate::routes::deployments::LogsResponse,
            crate::routes::templates::CreateTemplateRequest,
            crate::routes::templates::CreateWorkspacesFromTemplateRequest,
            crate::routes::templates::BulkWorkspaceCreateResponse,
            crate::services::templates::SecretCopyMode,
            crate::routes::telemetry::TelemetrySettings,
            crate::routes::admin::InviteUserRequest,
            crate::routes::admin::InviteUserResponse,
            crate::routes::admin::ChangeUserRoleRequest
        )
    ),
    tags(
        (name = "wara", description = "Safe Wara platform APIs")
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

pub struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

        let components = openapi.components.get_or_insert(Default::default());
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("opaque-api-token")
                    .build(),
            ),
        );

        for path_item in openapi.paths.paths.values_mut() {
            for operation in [
                path_item.get.as_mut(),
                path_item.put.as_mut(),
                path_item.post.as_mut(),
                path_item.delete.as_mut(),
                path_item.options.as_mut(),
                path_item.head.as_mut(),
                path_item.patch.as_mut(),
                path_item.trace.as_mut(),
            ]
            .into_iter()
            .flatten()
            {
                add_common_error_responses(operation);
            }
        }
    }
}

fn add_common_error_responses(operation: &mut utoipa::openapi::path::Operation) {
    use utoipa::openapi::{Content, Ref, RefOr, response::ResponseBuilder};

    for (status, description) in [
        ("400", "Invalid request"),
        ("401", "Authentication required"),
        ("403", "Forbidden"),
        ("404", "Resource not found"),
        ("500", "Internal server error"),
    ] {
        operation
            .responses
            .responses
            .entry(status.to_string())
            .or_insert(RefOr::T(
                ResponseBuilder::new()
                    .description(description)
                    .content(
                        "application/json",
                        Content::new(Some(Ref::from_schema_name("ErrorResponse"))),
                    )
                    .build(),
            ));
    }
}
