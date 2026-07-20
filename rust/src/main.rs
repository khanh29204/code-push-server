#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unreachable_patterns)]
#![allow(unused_imports)]

mod config;
mod core;
mod models;
mod routes;
mod services;
mod utils;

use axum::ServiceExt;
use axum::{Router, routing::get};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::Layer;
use tracing::info;

use crate::config::AppConfig;
use crate::core::db::{AppState, Db};
use crate::utils::storage::StorageManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = AppConfig::load();
    let port = config.port;

    info!("Starting CodePush server on Rust...");

    // Connect to the SQLite database
    let db = Db::connect(&config).await?;

    // Keep a handle to the DB so we can checkpoint the WAL on shutdown even
    // after `db` is moved into the AppState below.
    let db_for_shutdown = db.clone();

    // Initialize the local storage manager
    let storage = StorageManager::new(config.clone()).await;

    let local_storage_dir = config.local_storage_dir.clone();

    // Static Web UI directory. Defaults to "../public" (matches the Docker
    // WORKDIR /app/rust layout where UI files live at /app/public), but can be
    // overridden via PUBLIC_DIR so it does not depend on the current working dir.
    let public_dir = std::env::var("PUBLIC_DIR").unwrap_or_else(|_| "../public".to_string());

    // Create AppState
    let state = AppState {
        db,
        storage,
        config: Arc::new(config),
    };

    // Build application routing
    let app = Router::new()
        .merge(crate::routes::api_router())
        .route("/health", get(|| async { "OK" }))
        .nest_service(
            "/download",
            tower_http::services::ServeDir::new(local_storage_dir),
        )
        .fallback_service(tower_http::services::ServeDir::new(&public_dir))
        .with_state(state);

    let app = tower_http::normalize_path::NormalizePathLayer::trim_trailing_slash().layer(app);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, tower::make::Shared::new(app))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // The server has stopped accepting connections. Flush the WAL back into
    // the main database file and close the pool so the -wal/-shm side files
    // are removed cleanly.
    db_for_shutdown.shutdown().await;

    info!("Shutdown complete.");
    Ok(())
}

/// Resolve when the process receives a Ctrl-C (SIGINT) or SIGTERM signal.
/// SIGTERM is what `docker stop`, `kill`, and most process managers send.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown...");
}
