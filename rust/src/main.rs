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

use axum::{routing::get, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use tower::Layer;
use axum::ServiceExt;

use crate::config::AppConfig;
use crate::core::db::{AppState, Db};
use crate::utils::storage::StorageManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Load configuration
    let config = AppConfig::load();
    let port = config.port;
    
    info!("Starting CodePush server on Rust...");

    // Connect to MongoDB
    let db = Db::connect(&config).await?;
    
    // Initialize Storage Manager (Local or R2)
    let storage = StorageManager::new(config.clone()).await;

    let local_storage_dir = config.local_storage_dir.clone();

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
        .nest_service("/download", tower_http::services::ServeDir::new(local_storage_dir))
        .fallback_service(tower_http::services::ServeDir::new("../public"))
        .with_state(state);

    let app = tower_http::normalize_path::NormalizePathLayer::trim_trailing_slash().layer(app);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, tower::make::Shared::new(app)).await?;
    
    Ok(())
}
