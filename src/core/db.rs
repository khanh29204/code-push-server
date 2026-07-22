use crate::config::AppConfig;
use crate::utils::security::{password_hash_sync, rand_token};
use bb8_redis::{RedisConnectionManager, bb8};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct Db {
    pub pool: SqlitePool,
    pub redis: bb8::Pool<RedisConnectionManager>,
}

impl Db {
    pub async fn connect(
        config: &AppConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // If the database URL is a local file, ensure the parent directory exists
        if config.database_url.starts_with("sqlite://") {
            let file_path = &config.database_url[9..]; // Strip "sqlite://"
            if let Some(parent) = Path::new(file_path).parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            // Create the sqlite file if it doesn't exist
            if !Path::new(file_path).exists() {
                tokio::fs::File::create(file_path).await?;
            }
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url)
            .await?;

        // Enable WAL mode for better concurrent read/write performance
        sqlx::query("PRAGMA journal_mode=WAL;")
            .execute(&pool)
            .await?;

        tracing::info!("Connected to SQLite database at {}", config.database_url);

        // Run migrations (idempotent — CREATE TABLE IF NOT EXISTS)
        Self::run_migrations(&pool).await?;

        // Seed default data
        Self::seed_defaults(&pool).await?;

        // Redis part
        let redis_url = if let Some(pwd) = &config.redis.password {
            format!(
                "redis://:{}@{}:{}/{}",
                pwd, config.redis.host, config.redis.port, config.redis.db
            )
        } else {
            format!(
                "redis://{}:{}/{}",
                config.redis.host, config.redis.port, config.redis.db
            )
        };
        let manager = RedisConnectionManager::new(redis_url)?;
        let redis = bb8::Pool::builder().build(manager).await?;

        tracing::info!(
            "Connected to Redis at {}:{}",
            config.redis.host,
            config.redis.port
        );

        Ok(Self { pool, redis })
    }

    /// Gracefully shut down the database: flush the WAL back into the main
    /// database file and close all connections so SQLite can remove the
    /// `-wal` and `-shm` side files.
    ///
    /// `wal_checkpoint(TRUNCATE)` writes every committed page from the WAL
    /// into the main db file and truncates the WAL to zero length. Closing the
    /// pool afterwards releases the last connection, which lets SQLite delete
    /// the `-wal`/`-shm` files entirely.
    pub async fn shutdown(&self) {
        tracing::info!("Checkpointing WAL into the main database file...");

        match sqlx::query("PRAGMA wal_checkpoint(TRUNCATE);")
            .execute(&self.pool)
            .await
        {
            Ok(_) => tracing::info!("WAL checkpoint completed."),
            Err(e) => tracing::error!("WAL checkpoint failed: {}", e),
        }

        // Closing the pool releases the last SQLite connection, allowing the
        // engine to remove the -wal and -shm files.
        self.pool.close().await;
        tracing::info!("SQLite connection pool closed.");
    }

    /// Create all tables if they don't exist.
    /// Column names match the Rust model structs in src/models/*.rs exactly.
    async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        tracing::info!("Running database migrations...");

        // users
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL DEFAULT '',
                password TEXT NOT NULL DEFAULT '',
                email TEXT NOT NULL DEFAULT '',
                identical TEXT NOT NULL DEFAULT '',
                ack_code TEXT NOT NULL DEFAULT '',
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME DEFAULT NULL
            )",
        )
        .execute(pool)
        .await?;

        // apps
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS apps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL DEFAULT '',
                uid INTEGER NOT NULL DEFAULT 0,
                os INTEGER NOT NULL DEFAULT 0,
                platform INTEGER NOT NULL DEFAULT 0,
                is_use_diff_text INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME DEFAULT NULL
            )",
        )
        .execute(pool)
        .await?;

        // collaborators
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS collaborators (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                appid INTEGER NOT NULL DEFAULT 0,
                uid INTEGER NOT NULL DEFAULT 0,
                roles TEXT NOT NULL DEFAULT '',
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME DEFAULT NULL
            )",
        )
        .execute(pool)
        .await?;

        // deployments
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS deployments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                appid INTEGER NOT NULL DEFAULT 0,
                name TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                deployment_key TEXT NOT NULL DEFAULT '',
                last_deployment_version_id INTEGER NOT NULL DEFAULT 0,
                label_id INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME DEFAULT NULL
            )",
        )
        .execute(pool)
        .await?;

        // deployments_history
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS deployments_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                deployment_id INTEGER NOT NULL DEFAULT 0,
                package_id INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(pool)
        .await?;

        // deployments_versions
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS deployments_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                deployment_id INTEGER NOT NULL DEFAULT 0,
                app_version TEXT NOT NULL DEFAULT '',
                current_package_id INTEGER NOT NULL DEFAULT 0,
                min_version INTEGER NOT NULL DEFAULT 0,
                max_version INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME DEFAULT NULL
            )",
        )
        .execute(pool)
        .await?;

        // packages
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS packages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                deployment_version_id INTEGER NOT NULL DEFAULT 0,
                deployment_id INTEGER NOT NULL DEFAULT 0,
                description TEXT NOT NULL DEFAULT '',
                package_hash TEXT NOT NULL DEFAULT '',
                blob_url TEXT NOT NULL DEFAULT '',
                size INTEGER NOT NULL DEFAULT 0,
                manifest_blob_url TEXT NOT NULL DEFAULT '',
                release_method TEXT NOT NULL DEFAULT '',
                label TEXT NOT NULL DEFAULT '',
                original_label TEXT NOT NULL DEFAULT '',
                original_deployment TEXT NOT NULL DEFAULT '',
                released_by INTEGER NOT NULL DEFAULT 0,
                is_mandatory INTEGER NOT NULL DEFAULT 0,
                is_disabled INTEGER NOT NULL DEFAULT 0,
                rollout INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME DEFAULT NULL
            )",
        )
        .execute(pool)
        .await?;

        // packages_diff
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS packages_diff (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                package_id INTEGER NOT NULL DEFAULT 0,
                diff_against_package_hash TEXT NOT NULL DEFAULT '',
                diff_blob_url TEXT NOT NULL DEFAULT '',
                diff_size INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME DEFAULT NULL
            )",
        )
        .execute(pool)
        .await?;

        // packages_metrics
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS packages_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                package_id INTEGER NOT NULL DEFAULT 0,
                active INTEGER NOT NULL DEFAULT 0,
                downloaded INTEGER NOT NULL DEFAULT 0,
                failed INTEGER NOT NULL DEFAULT 0,
                installed INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                created_at DATETIME DEFAULT NULL
            )",
        )
        .execute(pool)
        .await?;

        // user_tokens
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                uid INTEGER NOT NULL DEFAULT 0,
                name TEXT NOT NULL DEFAULT '',
                tokens TEXT NOT NULL DEFAULT '',
                created_by TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                is_session INTEGER DEFAULT 0,
                expires_at DATETIME DEFAULT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(pool)
        .await?;

        // versions
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type INTEGER NOT NULL DEFAULT 0,
                version TEXT NOT NULL DEFAULT ''
            )",
        )
        .execute(pool)
        .await?;

        // log_report_deploy
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS log_report_deploy (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                status INTEGER NOT NULL DEFAULT 0,
                package_id INTEGER NOT NULL DEFAULT 0,
                client_unique_id TEXT NOT NULL DEFAULT '',
                previous_label TEXT NOT NULL DEFAULT '',
                previous_deployment_key TEXT NOT NULL DEFAULT '',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(pool)
        .await?;

        // log_report_download
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS log_report_download (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                package_id INTEGER NOT NULL DEFAULT 0,
                client_unique_id TEXT NOT NULL DEFAULT '',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(pool)
        .await?;

        // Create indexes (IF NOT EXISTS is implicit for CREATE INDEX IF NOT EXISTS)
        let indexes = [
            "CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)",
            "CREATE UNIQUE INDEX IF NOT EXISTS udx_users_identical ON users(identical)",
            "CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)",
            "CREATE INDEX IF NOT EXISTS idx_apps_name ON apps(name)",
            "CREATE INDEX IF NOT EXISTS idx_collaborators_appid ON collaborators(appid)",
            "CREATE INDEX IF NOT EXISTS idx_collaborators_uid ON collaborators(uid)",
            "CREATE INDEX IF NOT EXISTS idx_deployments_appid ON deployments(appid)",
            "CREATE INDEX IF NOT EXISTS idx_deployments_key ON deployments(deployment_key)",
            "CREATE INDEX IF NOT EXISTS idx_dh_deployment_id ON deployments_history(deployment_id)",
            "CREATE INDEX IF NOT EXISTS idx_dv_did_minversion ON deployments_versions(deployment_id, min_version)",
            "CREATE INDEX IF NOT EXISTS idx_dv_did_maxversion ON deployments_versions(deployment_id, max_version)",
            "CREATE INDEX IF NOT EXISTS idx_dv_did_appversion ON deployments_versions(deployment_id, app_version)",
            "CREATE INDEX IF NOT EXISTS idx_packages_did_label ON packages(deployment_id, label)",
            "CREATE INDEX IF NOT EXISTS idx_packages_versions_id ON packages(deployment_version_id)",
            "CREATE INDEX IF NOT EXISTS idx_pd_packageid_hash ON packages_diff(package_id, diff_against_package_hash)",
            "CREATE INDEX IF NOT EXISTS idx_pm_packageid ON packages_metrics(package_id)",
            "CREATE INDEX IF NOT EXISTS idx_ut_uid ON user_tokens(uid)",
            "CREATE INDEX IF NOT EXISTS idx_ut_tokens ON user_tokens(tokens)",
            "CREATE UNIQUE INDEX IF NOT EXISTS udx_versions_type ON versions(type)",
        ];

        for idx_sql in &indexes {
            sqlx::query(idx_sql).execute(pool).await?;
        }

        tracing::info!("Database migrations completed successfully.");
        Ok(())
    }

    /// Seed default data: versions row + admin account.
    /// Idempotent — skips if data already exists.
    async fn seed_defaults(
        pool: &SqlitePool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 1. Seed versions table: type=1, version='0.5.0'
        let version_exists: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM versions WHERE type = 1")
                .fetch_optional(pool)
                .await?;

        if version_exists.is_none() {
            sqlx::query("INSERT INTO versions (type, version) VALUES (1, '0.5.0')")
                .execute(pool)
                .await?;
            tracing::info!("Initialized DB Version to 0.5.0");
        }

        // 2. Seed default admin account
        let admin_exists: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM users WHERE username = 'admin'")
                .fetch_optional(pool)
                .await?;

        if admin_exists.is_none() {
            tracing::info!("Creating default admin account...");

            let password_hash = password_hash_sync("123456")
                .map_err(|e| format!("Failed to hash admin password: {}", e))?;
            let identical = rand_token(9);
            let ack_code = rand_token(5);

            sqlx::query(
                "INSERT INTO users (username, password, email, identical, ack_code, created_at, updated_at)
                 VALUES (?, ?, 'admin@codepush.com', ?, ?, datetime('now'), datetime('now'))"
            )
            .bind("admin")
            .bind(&password_hash)
            .bind(&identical)
            .bind(&ack_code)
            .execute(pool)
            .await?;

            tracing::info!("-------------------------------------------------------");
            tracing::info!("  Admin account created successfully!");
            tracing::info!("  Username: admin");
            tracing::info!("  Password: 123456");
            tracing::info!("-------------------------------------------------------");
        } else {
            tracing::info!("Admin account already exists. Skipping creation.");
        }

        tracing::info!("SUCCESS: Database is ready.");
        Ok(())
    }
}

// AppState will hold global objects like DB connection and Storage manager
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub storage: crate::utils::storage::StorageManager,
    pub config: Arc<AppConfig>,
}
