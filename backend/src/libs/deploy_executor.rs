//! Controlled execution of deployment commands.
//!
//! A [`DeployExecutor`] runs a sequence of typed [`RemoteCommand`]s and captures
//! the outcome of each. The trait is the seam that keeps the rest of the system
//! free of any "run an arbitrary command" capability: callers can only hand over
//! command objects built by the typed builders in [`crate::libs::docker`].
//!
//! This module ships [`PreviewExecutor`], which renders commands without
//! connecting anywhere (deployment output stays honest and injection-safe until
//! the real SSH transport lands). A real `russh`-backed executor and the
//! environment-to-server link are the immediate follow-up.

use async_trait::async_trait;

use crate::libs::docker::RemoteCommand;

/// The captured result of a single executed (or previewed) command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    /// The rendered, shell-safe command that was run.
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl CommandOutcome {
    pub fn succeeded(&self) -> bool {
        self.exit_code == 0
    }
}

/// The result of running all of a deployment's commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployRun {
    pub outcomes: Vec<CommandOutcome>,
    /// Whether the commands were actually executed on a host (`false` for a preview).
    pub executed: bool,
    /// Whether every executed command succeeded.
    pub success: bool,
}

impl DeployRun {
    /// Render a human-readable transcript suitable for `DeploymentRecord.output`.
    /// Pass any known secret values in `secrets` to scrub them from captured
    /// output before storage.
    pub fn to_output(&self, secrets: &[&str]) -> String {
        let mut lines = Vec::new();
        for outcome in &self.outcomes {
            lines.push(format!("$ {}", outcome.command));
            let stdout = redact_secrets(outcome.stdout.trim_end(), secrets);
            if !stdout.is_empty() {
                lines.push(stdout);
            }
            let stderr = redact_secrets(outcome.stderr.trim_end(), secrets);
            if !stderr.is_empty() {
                lines.push(stderr);
            }
            if self.executed {
                lines.push(format!("[exit {}]", outcome.exit_code));
            }
        }
        if !self.executed {
            lines.push("(preview — commands not executed)".to_string());
        }
        lines.join("\n")
    }
}

/// Replace every occurrence of each secret in `text` with a redaction marker, so
/// captured command output never leaks secret material into deployment records.
pub fn redact_secrets(text: &str, secrets: &[&str]) -> String {
    let mut redacted = text.to_string();
    for secret in secrets {
        if !secret.is_empty() {
            redacted = redacted.replace(secret, "[redacted]");
        }
    }
    redacted
}

/// Runs deployment commands and captures their output. Implementations decide
/// the transport (a no-op preview here; a real SSH connection in the follow-up).
#[async_trait]
pub trait DeployExecutor: Send + Sync {
    async fn run(&self, commands: &[RemoteCommand]) -> anyhow::Result<DeployRun>;
}

/// The default executor: renders commands but does not connect or execute. It
/// produces an honest, injection-safe transcript so deployment output is useful
/// before the real SSH transport exists.
#[derive(Debug, Clone, Default)]
pub struct PreviewExecutor;

#[async_trait]
impl DeployExecutor for PreviewExecutor {
    async fn run(&self, commands: &[RemoteCommand]) -> anyhow::Result<DeployRun> {
        let outcomes = commands
            .iter()
            .map(|command| CommandOutcome {
                command: command.render(),
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            })
            .collect();
        Ok(DeployRun {
            outcomes,
            executed: false,
            success: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A test double that returns scripted outcomes and records what it was asked
    /// to run. Stands in for a real SSH executor so capture behavior can be tested
    /// without a host (per issue #11's testing guidance).
    struct RecordingExecutor {
        run: DeployRun,
    }

    #[async_trait]
    impl DeployExecutor for RecordingExecutor {
        async fn run(&self, _commands: &[RemoteCommand]) -> anyhow::Result<DeployRun> {
            Ok(self.run.clone())
        }
    }

    fn outcome(command: &str, stdout: &str, stderr: &str, exit_code: i32) -> CommandOutcome {
        CommandOutcome {
            command: command.to_string(),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            exit_code,
        }
    }

    #[tokio::test]
    async fn preview_executor_renders_without_executing() {
        let commands = vec![RemoteCommand::new().lit("docker pull").arg("nginx:latest")];
        let run = PreviewExecutor.run(&commands).await.unwrap();
        assert!(!run.executed);
        assert!(run.success);
        assert_eq!(run.outcomes.len(), 1);
        assert_eq!(run.outcomes[0].command, "docker pull 'nginx:latest'");
        let output = run.to_output(&[]);
        assert!(output.contains("$ docker pull 'nginx:latest'"));
        assert!(output.contains("(preview — commands not executed)"));
    }

    #[tokio::test]
    async fn captures_success_output() {
        let executor = RecordingExecutor {
            run: DeployRun {
                outcomes: vec![outcome("docker pull 'nginx'", "Pulled nginx", "", 0)],
                executed: true,
                success: true,
            },
        };
        let run = executor.run(&[]).await.unwrap();
        let output = run.to_output(&[]);
        assert!(output.contains("$ docker pull 'nginx'"));
        assert!(output.contains("Pulled nginx"));
        assert!(output.contains("[exit 0]"));
    }

    #[tokio::test]
    async fn captures_failure_output() {
        let executor = RecordingExecutor {
            run: DeployRun {
                outcomes: vec![outcome(
                    "docker run -d --name 'wara-api' 'nginx'",
                    "",
                    "docker: error: name already in use",
                    125,
                )],
                executed: true,
                success: false,
            },
        };
        let run = executor.run(&[]).await.unwrap();
        assert!(!run.success);
        assert!(!run.outcomes[0].succeeded());
        let output = run.to_output(&[]);
        assert!(output.contains("docker: error: name already in use"));
        assert!(output.contains("[exit 125]"));
    }

    #[test]
    fn redacts_secret_values_from_output() {
        let secret = "super-secret-registry-token";
        let run = DeployRun {
            outcomes: vec![outcome(
                "docker login -u user -p [arg]",
                &format!("logged in with {secret}"),
                "",
                0,
            )],
            executed: true,
            success: true,
        };
        let output = run.to_output(&[secret]);
        assert!(!output.contains(secret));
        assert!(output.contains("[redacted]"));
    }

    #[test]
    fn redact_secrets_handles_empty_and_multiple() {
        assert_eq!(redact_secrets("abc", &[]), "abc");
        assert_eq!(redact_secrets("a-b-c", &["", "b"]), "a-[redacted]-c");
    }
}
