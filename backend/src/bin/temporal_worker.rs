use std::time::Duration;

use tokio::time::{self, MissedTickBehavior};
use wara_backend::{
    errors::wara::WaraError,
    libs::{config::Config, db, telemetry},
    services::{
        activities::server_checks::ServerCheckActivities, queues::QueueName,
        workflows::server_checks::ServerConnectivityWorkflowInput,
    },
};

#[tokio::main]
async fn main() -> Result<(), WaraError> {
    let queue = std::env::args()
        .nth(1)
        .map(|value| value.parse::<QueueName>())
        .transpose()
        .map_err(|error| WaraError::InvalidQueue(error.to_string()))?
        .unwrap_or(QueueName::Default);
    let config = Config::from_env();
    let guard = telemetry::init(&config)?;
    let db = db::connect(&config).await?;
    tracing::info!(
        task_queue = %queue.as_task_queue(),
        temporal_address = %config.temporal_address,
        namespace = %config.temporal_namespace,
        "Wara Temporal worker scaffold started"
    );
    run_server_connectivity_workflow(config.clone(), db).await?;
    telemetry::shutdown(guard)?;
    Ok(())
}

async fn run_server_connectivity_workflow(
    config: Config,
    db: wara_backend::libs::db::Database,
) -> Result<(), WaraError> {
    let input = ServerConnectivityWorkflowInput {
        interval_seconds: config.server_connectivity_check_interval_seconds,
    };
    if input.interval_seconds == 0 {
        tracing::info!("scheduled server connectivity checks disabled");
        shutdown_signal().await;
        return Ok(());
    }

    let activities = ServerCheckActivities::new(db, config.secret_key.clone());
    let mut timer = time::interval(Duration::from_secs(input.interval_seconds));
    timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
    tracing::info!(
        interval_seconds = input.interval_seconds,
        "scheduled server connectivity workflow started"
    );

    loop {
        tokio::select! {
            _ = timer.tick() => {
                match activities.check_all_servers().await {
                    Ok(checks) => {
                        tracing::info!(
                            checked_servers = checks.len(),
                            "scheduled server connectivity workflow completed"
                        );
                    }
                    Err(error) => {
                        tracing::warn!(
                            error = %error,
                            "scheduled server connectivity workflow failed"
                        );
                    }
                }
            }
            _ = shutdown_signal() => {
                tracing::info!("scheduled server connectivity workflow stopping");
                break;
            }
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
