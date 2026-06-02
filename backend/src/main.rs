use std::net::SocketAddr;

use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use wara_backend::{
    libs::{config::Config, db, telemetry},
    routes,
    services::auth::AuthService,
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::from_env();
    config.validate_jwt_key_config()?;
    let telemetry_guard = telemetry::init(&config)?;
    let db = db::connect(&config).await?;
    AuthService::new(db.clone(), config.clone())
        .bootstrap_admin()
        .await?;
    let state = AppState::new(config.clone(), db);

    let app = routes::router(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = config.bind_addr.parse()?;
    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "Wara backend listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    telemetry::shutdown(telemetry_guard)?;
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
