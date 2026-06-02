#[derive(Debug, Clone)]
pub struct SshTarget {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub public_key: String,
    pub private_key: String,
    pub private_key_passphrase: Option<String>,
}

pub const SSH_CONNECTIVITY_CHECK_COMMAND: &str = "printf 'wara-ssh-ok\\n'";
pub const DOCKER_VERSION_CHECK_COMMAND: &str = "docker version --format '{{.Server.Version}}'";

pub async fn run_controlled_commands(
    target: &SshTarget,
    commands: &[String],
) -> anyhow::Result<String> {
    validate_allowed_commands(commands)?;
    tracing::info!(
        host = %target.host,
        port = target.port,
        username = %target.username,
        public_key_fingerprint = %fingerprint_public_key(&target.public_key),
        command_count = commands.len(),
        "planned controlled SSH command execution"
    );
    Ok(mock_controlled_output(commands))
}

pub fn server_check_commands() -> Vec<String> {
    vec![
        SSH_CONNECTIVITY_CHECK_COMMAND.to_string(),
        DOCKER_VERSION_CHECK_COMMAND.to_string(),
    ]
}

pub fn validate_allowed_commands(commands: &[String]) -> anyhow::Result<()> {
    for command in commands {
        if command != SSH_CONNECTIVITY_CHECK_COMMAND && command != DOCKER_VERSION_CHECK_COMMAND {
            anyhow::bail!("SSH command is not allowlisted");
        }
    }
    Ok(())
}

pub fn parse_server_check_output(output: &str) -> anyhow::Result<String> {
    let lines: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.first() != Some(&"wara-ssh-ok") {
        anyhow::bail!("SSH connectivity probe did not return expected marker");
    }
    let Some(version) = lines.get(1) else {
        anyhow::bail!("Docker version output is missing");
    };
    Ok((*version).to_string())
}

pub fn redact_error(error: &str, target: &SshTarget) -> String {
    let mut redacted = error
        .replace(&target.private_key, "[redacted-private-key]")
        .replace(&target.public_key, "[redacted-public-key]");
    if let Some(passphrase) = &target.private_key_passphrase {
        redacted = redacted.replace(passphrase, "[redacted-private-key-passphrase]");
    }
    redacted.chars().take(500).collect()
}

pub fn fingerprint_public_key(public_key: &str) -> String {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(public_key.trim().as_bytes());
    format!("SHA256:{}", STANDARD.encode(digest))
}

fn mock_controlled_output(commands: &[String]) -> String {
    commands
        .iter()
        .map(|command| match command.as_str() {
            SSH_CONNECTIVITY_CHECK_COMMAND => "wara-ssh-ok",
            DOCKER_VERSION_CHECK_COMMAND => "25.0.0",
            _ => "",
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_check_commands_are_allowlisted() {
        let commands = server_check_commands();

        validate_allowed_commands(&commands).expect("server check commands should be allowed");
        assert_eq!(
            commands,
            vec![
                SSH_CONNECTIVITY_CHECK_COMMAND.to_string(),
                DOCKER_VERSION_CHECK_COMMAND.to_string(),
            ]
        );
        assert!(
            validate_allowed_commands(&["rm -rf /".to_string()]).is_err(),
            "unexpected commands must be rejected"
        );
    }

    #[test]
    fn parses_server_check_output() {
        assert_eq!(
            parse_server_check_output("wara-ssh-ok\n25.0.0\n").expect("parse output"),
            "25.0.0"
        );
        assert!(parse_server_check_output("wrong\n25.0.0\n").is_err());
        assert!(parse_server_check_output("wara-ssh-ok\n").is_err());
    }

    #[test]
    fn redacts_key_material_from_errors() {
        let target = SshTarget {
            host: "203.0.113.10".to_string(),
            port: 22,
            username: "deploy".to_string(),
            public_key: "ssh-ed25519 public".to_string(),
            private_key: "-----BEGIN PRIVATE KEY-----secret".to_string(),
            private_key_passphrase: Some("passphrase-secret".to_string()),
        };
        let error = format!(
            "failed with {} {} {}",
            target.private_key,
            target.public_key,
            target.private_key_passphrase.as_deref().unwrap()
        );

        let redacted = redact_error(&error, &target);

        assert!(!redacted.contains(&target.private_key));
        assert!(!redacted.contains(&target.public_key));
        assert!(!redacted.contains("passphrase-secret"));
        assert!(redacted.contains("[redacted-private-key]"));
    }
}
