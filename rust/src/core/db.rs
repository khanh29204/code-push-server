use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use crate::config::AppConfig;
use std::sync::Arc;
use std::path::Path;
use bb8_redis::{bb8, RedisConnectionManager};

#[derive(Clone)]
pub struct Db {
    pub pool: SqlitePool,
    pub redis: bb8::Pool<RedisConnectionManager>,
}

impl Db {
    pub async fn connect(config: &AppConfig) -> Result<Self, Box<dyn std::error::Error>> {
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

        tracing::info!("Connected to SQLite database at {}", config.database_url);

        // Redis part
        let redis_url = if let Some(pwd) = &config.redis.password {
            format!("redis://:{}@{}:{}/{}", pwd, config.redis.host, config.redis.port, config.redis.db)
        } else {
            format!("redis://{}:{}/{}", config.redis.host, config.redis.port, config.redis.db)
        };
        let manager = RedisConnectionManager::new(redis_url)?;
        let redis = bb8::Pool::builder().build(manager).await?;
        
        tracing::info!("Connected to Redis at {}:{}", config.redis.host, config.redis.port);

        Ok(Self { pool, redis })
    }
}

// AppState will hold global objects like DB connection and Storage manager
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub storage: crate::utils::storage::StorageManager,
    pub config: Arc<AppConfig>,
}
