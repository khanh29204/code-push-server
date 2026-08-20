use crate::config::AppConfig;
use std::path::PathBuf;
use tokio::fs;

#[derive(Clone)]
pub struct StorageManager {
    config: AppConfig,
}

impl StorageManager {
    pub async fn new(config: AppConfig) -> Self {
        // Ensure the local storage directory exists
        let dir = &config.local_storage_dir;
        if let Err(e) = fs::create_dir_all(dir).await {
            tracing::error!("Failed to create local storage directory {}: {}", dir, e);
        }

        Self { config }
    }

    pub async fn upload_file(
        &self,
        key: &str,
        file_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let base_dir = &self.config.local_storage_dir;

        // Similar logic to JS: subDir = key.substring(0, 2).toLowerCase()
        let sub_dir = if key.len() >= 2 {
            key[0..2].to_lowercase()
        } else {
            "00".to_string()
        };

        let target_dir = PathBuf::from(base_dir).join(sub_dir);
        fs::create_dir_all(&target_dir).await?;

        let target_path = target_dir.join(key);

        if target_path.exists() {
            tracing::info!("File {} already exists locally, skipping copy", key);
            return Ok(());
        }

        fs::copy(file_path, &target_path).await?;
        tracing::info!("Uploaded {} to local storage", key);
        Ok(())
    }

    /// Counterpart of `upload_file`. Unused so far: packages are served
    /// straight off disk by the /download ServeDir.
    #[allow(dead_code)]
    pub async fn download_file(
        &self,
        key: &str,
        target_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let base_dir = &self.config.local_storage_dir;
        let sub_dir = if key.len() >= 2 {
            key[0..2].to_lowercase()
        } else {
            "00".to_string()
        };
        let source_path = PathBuf::from(base_dir).join(sub_dir).join(key);

        fs::copy(source_path, target_path).await?;
        Ok(())
    }

    pub async fn delete_file(
        &self,
        key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let base_dir = &self.config.local_storage_dir;
        let sub_dir = if key.len() >= 2 {
            key[0..2].to_lowercase()
        } else {
            "00".to_string()
        };
        let target_path = PathBuf::from(base_dir).join(sub_dir).join(key);

        if target_path.exists() {
            fs::remove_file(&target_path).await?;
            tracing::info!("Deleted local file: {:?}", target_path);
        }
        Ok(())
    }
}
