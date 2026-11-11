use std::{path::PathBuf, sync::Arc};

use crate::config::{AppConfig, RedisConfig, SmtpConfig};
use crate::core::db::{AppState, Db};
use crate::utils::storage::StorageManager;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use bb8_redis::{RedisConnectionManager, bb8};
use jsonwebtoken::{EncodingKey, Header};
use serde::Serialize;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tower::ServiceExt;

pub struct TestApp {
    pub router: Router,
    #[allow(dead_code)]
    pub state: AppState,
    pub pool: SqlitePool,
    pub test_dir: PathBuf,
    pub storage_dir: PathBuf,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.test_dir);
    }
}

#[derive(Serialize)]
struct Claims {
    uid: i64,
    hash: String,
    exp: usize,
}

pub async fn app() -> TestApp {
    let suffix = uuid::Uuid::new_v4();
    let root = std::env::var_os("TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/khanh/.hermes/cache/scratch"));
    let test_dir = root.join(format!("code-push-bulk-{suffix}"));
    std::fs::create_dir_all(&test_dir).unwrap();
    let storage_dir = test_dir.join("storage");
    let database_url = "sqlite::memory:".to_string();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .unwrap();
    for sql in [
        "CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL, password TEXT NOT NULL, email TEXT NOT NULL, identical TEXT NOT NULL, ack_code TEXT NOT NULL, created_at DATETIME, updated_at DATETIME)",
        "CREATE TABLE apps (id INTEGER PRIMARY KEY, name TEXT NOT NULL, uid INTEGER NOT NULL, os INTEGER NOT NULL, platform INTEGER NOT NULL, is_use_diff_text INTEGER NOT NULL, created_at DATETIME, updated_at DATETIME)",
        "CREATE TABLE collaborators (id INTEGER PRIMARY KEY, appid INTEGER NOT NULL, uid INTEGER NOT NULL, roles TEXT NOT NULL, created_at DATETIME, updated_at DATETIME)",
        "CREATE TABLE deployments (id INTEGER PRIMARY KEY, appid INTEGER NOT NULL, name TEXT NOT NULL, description TEXT NOT NULL, deployment_key TEXT NOT NULL, last_deployment_version_id INTEGER NOT NULL, label_id INTEGER NOT NULL, created_at DATETIME, updated_at DATETIME)",
        "CREATE TABLE deployments_history (id INTEGER PRIMARY KEY, deployment_id INTEGER NOT NULL, package_id INTEGER NOT NULL, created_at DATETIME)",
        "CREATE TABLE deployments_versions (id INTEGER PRIMARY KEY, deployment_id INTEGER NOT NULL, app_version TEXT NOT NULL, current_package_id INTEGER NOT NULL, min_version INTEGER NOT NULL, max_version INTEGER NOT NULL, created_at DATETIME, updated_at DATETIME)",
        "CREATE TABLE packages (id INTEGER PRIMARY KEY, deployment_version_id INTEGER NOT NULL, deployment_id INTEGER NOT NULL, description TEXT NOT NULL, package_hash TEXT NOT NULL, blob_url TEXT NOT NULL, size INTEGER NOT NULL, manifest_blob_url TEXT NOT NULL, release_method TEXT NOT NULL, label TEXT NOT NULL, original_label TEXT NOT NULL, original_deployment TEXT NOT NULL, released_by INTEGER NOT NULL, is_mandatory INTEGER NOT NULL, is_disabled INTEGER NOT NULL, rollout INTEGER NOT NULL, created_at DATETIME, updated_at DATETIME)",
        "CREATE TABLE packages_diff (id INTEGER PRIMARY KEY, package_id INTEGER NOT NULL, diff_against_package_hash TEXT NOT NULL, diff_blob_url TEXT NOT NULL, diff_size INTEGER NOT NULL, created_at DATETIME, updated_at DATETIME)",
        "CREATE TABLE packages_metrics (id INTEGER PRIMARY KEY, package_id INTEGER NOT NULL, active INTEGER NOT NULL, downloaded INTEGER NOT NULL, failed INTEGER NOT NULL, installed INTEGER NOT NULL, created_at DATETIME, updated_at DATETIME)",
        "CREATE TABLE user_tokens (id INTEGER PRIMARY KEY, uid INTEGER NOT NULL, name TEXT NOT NULL, tokens TEXT NOT NULL, created_by TEXT NOT NULL, description TEXT NOT NULL, is_session INTEGER, expires_at DATETIME, created_at DATETIME)",
    ] {
        sqlx::query(sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO users VALUES (1, 'owner', '', 'owner@example.test', 'identical', 'ack', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO apps VALUES (1, 'app', 1, 1, 1, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collaborators VALUES (1, 1, 1, 'Owner', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO deployments VALUES (1, 1, 'Production', '', 'deployment-key', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO deployments VALUES (5, 1, 'Staging', '', 'staging-key', 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)")
        .execute(&pool).await.unwrap();

    let config = AppConfig {
        port: 3000,
        log_level: "error".into(),
        database_url,
        jwt_token_secret: "test-secret".into(),
        redis: RedisConfig {
            url: "redis://127.0.0.1:1/0".into(),
        },
        storage_type: "local".into(),
        local_storage_dir: storage_dir.to_string_lossy().into_owned(),
        download_url: "http://localhost/download".into(),
        allow_registration: false,
        try_login_times: 4,
        diff_nums: 3,
        data_dir: storage_dir.to_string_lossy().into_owned(),
        update_check_cache: false,
        rollout_client_unique_id_cache: false,
        smtp: SmtpConfig {
            host: String::new(),
            port: 465,
            secure: true,
            username: String::new(),
            password: String::new(),
        },
    };
    let redis_manager = RedisConnectionManager::new(config.redis.url.as_str()).unwrap();
    let state = AppState {
        db: Db {
            pool: pool.clone(),
            redis: bb8::Pool::builder().build_unchecked(redis_manager),
        },
        storage: StorageManager::new(config.clone()).await,
        config: Arc::new(config),
    };
    let router = crate::routes::apps::router().with_state(state.clone());
    TestApp {
        router,
        state,
        pool,
        test_dir,
        storage_dir,
    }
}

pub fn owner_token() -> String {
    token_for_uid(1)
}

pub fn token_for_uid(uid: i64) -> String {
    jsonwebtoken::encode(
        &Header::default(),
        &Claims {
            uid,
            hash: crate::utils::security::md5_hash("ack"),
            exp: (chrono::Utc::now().timestamp() + 600) as usize,
        },
        &EncodingKey::from_secret(b"test-secret"),
    )
    .unwrap()
}

pub async fn request(
    app: &TestApp,
    method: &str,
    uri: &str,
    body: &str,
) -> (StatusCode, serde_json::Value) {
    request_with_token(app, method, uri, body, Some(&owner_token())).await
}

pub async fn request_with_token(
    app: &TestApp,
    method: &str,
    uri: &str,
    body: &str,
    token: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .router
        .clone()
        .oneshot(builder.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null),
    )
}
