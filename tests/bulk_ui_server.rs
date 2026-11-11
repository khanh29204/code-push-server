#[allow(dead_code)]
#[path = "../src/config/mod.rs"]
mod config;
#[allow(dead_code)]
#[path = "../src/core/mod.rs"]
mod core;
#[allow(dead_code)]
#[path = "../src/models/mod.rs"]
mod models;
#[allow(dead_code)]
#[path = "../src/routes/mod.rs"]
mod routes;
#[allow(dead_code)]
#[path = "../src/services/mod.rs"]
mod services;
#[allow(dead_code)]
#[path = "../src/utils/mod.rs"]
mod utils;

#[path = "support_bulk/mod.rs"]
mod support;

use axum::{Json, Router, extract::State, routing::get};
use serde_json::json;
use tower_http::services::ServeDir;

async fn test_session() -> Json<serde_json::Value> {
    Json(json!({ "token": support::owner_token() }))
}

async fn test_snapshot(State(state): State<core::db::AppState>) -> Json<serde_json::Value> {
    let counts: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM packages), (SELECT count(*) FROM packages_diff), (SELECT count(*) FROM packages_metrics), (SELECT count(*) FROM deployments_history)",
    )
    .fetch_one(&state.db.pool)
    .await
    .unwrap();
    let storage = &state.config.local_storage_dir;
    let files: Vec<bool> = ["blob-v1", "manifest-v1", "blob-v2", "manifest-v2"]
        .iter()
        .map(|key| {
            std::path::Path::new(storage)
                .join(&key[..2])
                .join(key)
                .is_file()
        })
        .collect();
    Json(json!({
        "packages": counts.0,
        "diffs": counts.1,
        "metrics": counts.2,
        "history": counts.3,
        "files": files,
    }))
}

#[tokio::test]
#[ignore = "manual browser fixture; binds localhost:4174 until stopped"]
async fn serve_fixture_for_browser() {
    let fixture = support::app().await;
    sqlx::query("INSERT INTO packages VALUES (1, 1, 1, 'First fixture release', 'hash-v1', 'blob-v1', 12, 'manifest-v1', 'Upload', 'v1', '', '', 1, 0, 1, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&fixture.pool).await.unwrap();
    sqlx::query("INSERT INTO packages VALUES (2, 2, 1, 'Newest fixture release', 'hash-v2', 'blob-v2', 15, 'manifest-v2', 'Upload', 'v2', '', '', 1, 0, 1, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&fixture.pool).await.unwrap();
    sqlx::query("INSERT INTO deployments_versions VALUES (1, 1, '1.0', 1, 0, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP), (2, 1, '1.0', 2, 0, 100, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&fixture.pool).await.unwrap();
    sqlx::query("INSERT INTO deployments_history VALUES (1, 1, 1, CURRENT_TIMESTAMP), (2, 1, 2, CURRENT_TIMESTAMP)")
        .execute(&fixture.pool).await.unwrap();
    sqlx::query("INSERT INTO packages_metrics VALUES (1, 1, 2, 3, 0, 2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP), (2, 2, 4, 7, 1, 3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&fixture.pool).await.unwrap();
    sqlx::query("UPDATE deployments SET last_deployment_version_id = 2 WHERE id = 1")
        .execute(&fixture.pool)
        .await
        .unwrap();
    for (key, contents) in [
        ("blob-v1", &b"fixture bundle one"[..]),
        ("manifest-v1", &b"fixture manifest one"[..]),
        ("blob-v2", &b"fixture bundle two"[..]),
        ("manifest-v2", &b"fixture manifest two"[..]),
    ] {
        let dir = fixture.storage_dir.join(&key[..2]);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(key), contents).unwrap();
    }

    let app = Router::new()
        .merge(crate::routes::api_router())
        .route("/health", get(|| async { "OK" }))
        .route("/__test_session", get(test_session))
        .route("/__test_snapshot", get(test_snapshot))
        .nest_service("/download", ServeDir::new(&fixture.storage_dir))
        .fallback_service(ServeDir::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/public"
        )))
        .with_state(fixture.state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:4174")
        .await
        .expect("port 4174 must be free for the browser fixture");
    eprintln!("Bulk UI fixture: http://127.0.0.1:4174 (synthetic local owner; Ctrl-C to stop)");
    axum::serve(listener, app).await.unwrap();
}
