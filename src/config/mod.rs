use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub db: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub secure: bool,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    // Server
    pub port: u16,
    /// Parsed from `LOG_LEVEL` for `.env` compatibility. Not wired up: the
    /// tracing filter is driven by `RUST_LOG` (see main.rs).
    #[allow(dead_code)]
    pub log_level: String,

    // Database
    pub database_url: String,

    // JWT
    pub jwt_token_secret: String,

    // Redis
    pub redis: RedisConfig,

    // Storage
    pub storage_type: String, // "local" | "s3" | "qiniu" | "oss" | "tencentcloud"
    pub local_storage_dir: String,
    pub download_url: String, // e.g. http://127.0.0.1:3000/download

    // Common
    pub allow_registration: bool,
    pub try_login_times: i32,
    pub diff_nums: i32,   // Number of diff packages to generate (default 3)
    pub data_dir: String, // Temp dir for diff calculation
    pub update_check_cache: bool,
    /// Parsed from `ROLLOUT_CLIENT_UNIQUE_ID_CACHE` for `.env` compatibility.
    /// Not wired up: `ClientManager::chosen_man` always caches its verdict.
    #[allow(dead_code)]
    pub rollout_client_unique_id_cache: bool,

    // SMTP (for sending registration codes)
    pub smtp: SmtpConfig,
}

impl AppConfig {
    pub fn load() -> Self {
        dotenv::dotenv().ok();

        let port = env::var("PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .unwrap_or(3000);
        let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        // Database
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            if let Ok(db_file) = env::var("DB_STORAGE_FILE") {
                format!("sqlite://{}", db_file)
            } else if let Ok(data_dir) = env::var("DATA_DIR") {
                format!("sqlite://{}/codepush.sqlite", data_dir)
            } else {
                "sqlite:///data/codepush.sqlite".to_string()
            }
        });

        // JWT
        let jwt_token_secret = env::var("JWT_TOKEN_SECRET")
            .or_else(|_| env::var("TOKEN_SECRET"))
            .unwrap_or_else(|_| "INSERT_RANDOM_TOKEN_KEY".to_string());

        // Redis
        let redis = RedisConfig {
            host: env::var("REDIS_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("REDIS_PORT")
                .unwrap_or_else(|_| "6379".to_string())
                .parse()
                .unwrap_or(6379),
            password: env::var("REDIS_PASSWORD").ok().filter(|s| !s.is_empty()),
            db: env::var("REDIS_DB")
                .unwrap_or_else(|_| "0".to_string())
                .parse()
                .unwrap_or(0),
        };

        // Storage
        let storage_type = env::var("STORAGE_TYPE").unwrap_or_else(|_| "local".to_string());
        let local_storage_dir = env::var("STORAGE_DIR")
            .or_else(|_| env::var("LOCAL_STORAGE_DIR"))
            .unwrap_or_else(|_| "../data-storage".to_string());
        let download_url = env::var("LOCAL_DOWNLOAD_URL")
            .or_else(|_| env::var("DOWNLOAD_URL"))
            .unwrap_or_else(|_| format!("http://127.0.0.1:{}/download", port));

        // Common
        let allow_registration = env::var("ALLOW_REGISTRATION")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        let try_login_times = env::var("TRY_LOGIN_TIMES")
            .unwrap_or_else(|_| "4".to_string())
            .parse()
            .unwrap_or(4);
        let diff_nums = env::var("DIFF_NUMS")
            .unwrap_or_else(|_| "3".to_string())
            .parse()
            .unwrap_or(3);
        let data_dir = env::var("DATA_DIR")
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string());
        let update_check_cache = env::var("UPDATE_CHECK_CACHE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        let rollout_client_unique_id_cache = env::var("ROLLOUT_CLIENT_UNIQUE_ID_CACHE")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        // SMTP
        let smtp = SmtpConfig {
            host: env::var("SMTP_HOST").unwrap_or_default(),
            port: env::var("SMTP_PORT")
                .unwrap_or_else(|_| "465".to_string())
                .parse()
                .unwrap_or(465),
            secure: true,
            username: env::var("SMTP_USERNAME").unwrap_or_default(),
            password: env::var("SMTP_PASSWORD").unwrap_or_default(),
        };

        AppConfig {
            port,
            log_level,
            database_url,
            jwt_token_secret,
            redis,
            storage_type,
            local_storage_dir,
            download_url,
            allow_registration,
            try_login_times,
            diff_nums,
            data_dir,
            update_check_cache,
            rollout_client_unique_id_cache,
            smtp,
        }
    }
}
