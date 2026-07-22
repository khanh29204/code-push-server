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
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::AppConfig;
use crate::core::db::{AppState, Db};
use crate::utils::storage::StorageManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing. Default log level is `info`, and we enable
    // request tracing at `info` level so incoming HTTP requests are logged.
    // Override with the RUST_LOG env-var, e.g. RUST_LOG=debug.
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("info,tower_http=info")
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();

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

    // Static Web UI directory. Tries the PUBLIC_DIR env-var first, then falls
    // back to common relative locations so the same binary works both in local
    // development (where public/ is at ../public relative to the rust/ dir)
    // and inside Docker (WORKDIR /app, public at ./public).
    let public_dir = std::env::var("PUBLIC_DIR").unwrap_or_else(|_| {
        for candidate in &["./public"] {
            if std::path::Path::new(candidate).is_dir() {
                return candidate.to_string();
            }
        }
        "./public".to_string()
    });

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
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    tracing::info_span!(
                        "request",
                        method = %request.method(),
                        uri = %request.uri(),
                    )
                })
                .on_response(
                    |response: &axum::http::Response<_>,
                     latency: std::time::Duration,
                     _span: &tracing::Span| {
                        tracing::info!(
                            status = %response.status(),
                            latency = ?latency,
                            "response"
                        );
                    },
                ),
        )
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
