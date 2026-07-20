use std::fs;
use std::path::Path;
use tracing::{debug, error};

use crate::core::app_error::AppError;
use crate::utils::common::{copy_dir_all, create_empty_folder};
use crate::utils::security::{calc_all_file_sha256, package_hash_sync};

const MANIFEST_FILENAME: &str = "manifest.json";
const CONTENTS_NAME: &str = "contents";

pub struct PackageInfo {
    pub package_hash: String,
    pub path: String,
    pub content_path: String,
    pub manifest_file_path: String,
}

pub struct DataCenterManager;

impl DataCenterManager {
    pub fn get_data_dir(config: &crate::config::AppConfig) -> String {
        config.data_dir.clone()
    }

    pub fn has_package_store_sync(config: &crate::config::AppConfig, package_hash: &str) -> bool {
        let data_dir = Self::get_data_dir(config);
        let package_hash_path = Path::new(&data_dir).join(package_hash);
        let manifest_file = package_hash_path.join(MANIFEST_FILENAME);
        let content_path = package_hash_path.join(CONTENTS_NAME);
        manifest_file.exists() && content_path.exists()
    }

    pub fn get_package_info(config: &crate::config::AppConfig, package_hash: &str) -> Result<PackageInfo, AppError> {
        if Self::has_package_store_sync(config, package_hash) {
            let data_dir = Self::get_data_dir(config);
            let package_hash_path = Path::new(&data_dir).join(package_hash);
            let manifest_file = package_hash_path.join(MANIFEST_FILENAME);
            let content_path = package_hash_path.join(CONTENTS_NAME);
            
            return Ok(Self::build_package_info(
                package_hash.to_string(),
                package_hash_path.to_string_lossy().to_string(),
                content_path.to_string_lossy().to_string(),
                manifest_file.to_string_lossy().to_string(),
            ));
        }
        Err(AppError::new("can't get PackageInfo"))
    }

    pub fn build_package_info(
        package_hash: String,
        package_hash_path: String,
        content_path: String,
        manifest_file: String,
    ) -> PackageInfo {
        PackageInfo {
            package_hash,
            path: package_hash_path,
            content_path,
            manifest_file_path: manifest_file,
        }
    }

    pub async fn validate_store(config: &crate::config::AppConfig, provide_package_hash: &str) -> bool {
        let data_dir = Self::get_data_dir(config);
        let package_hash_path = Path::new(&data_dir).join(provide_package_hash);
        let manifest_file = package_hash_path.join(MANIFEST_FILENAME);
        let content_path = package_hash_path.join(CONTENTS_NAME);
        
        if !Self::has_package_store_sync(config, provide_package_hash) {
            debug!("validateStore providePackageHash not exist");
            return false;
        }

        let manifest_json = match calc_all_file_sha256(content_path.to_string_lossy().as_ref()).await {
            Ok(v) => v,
            Err(_) => return false,
        };
        
        let package_hash = package_hash_sync(&manifest_json);
        
        let manifest_json_local = match fs::read_to_string(&manifest_file) {
            Ok(content) => content,
            Err(_) => {
                debug!("validateStore manifestFile contents invalid");
                return false;
            }
        };
        
        let manifest_local: std::collections::BTreeMap<String, String> = match serde_json::from_str(&manifest_json_local) {
            Ok(v) => v,
            Err(_) => return false,
        };
        
        let package_hash_local = package_hash_sync(&manifest_local);
        
        if provide_package_hash == package_hash && provide_package_hash == package_hash_local {
            debug!("validateStore store files is ok");
            return true;
        }
        
        debug!("validateStore store files broken");
        false
    }

    pub async fn store_package(config: &crate::config::AppConfig, source_dst: &str, force: bool) -> Result<PackageInfo, AppError> {
        let manifest_json = calc_all_file_sha256(source_dst).await?;
        let package_hash = package_hash_sync(&manifest_json);
        
        let data_dir = Self::get_data_dir(config);
        let package_hash_path = Path::new(&data_dir).join(&package_hash);
        let manifest_file = package_hash_path.join(MANIFEST_FILENAME);
        let content_path = package_hash_path.join(CONTENTS_NAME);
        
        let is_validate = Self::validate_store(config, &package_hash).await;
        
        if !force && is_validate {
            return Ok(Self::build_package_info(
                package_hash,
                package_hash_path.to_string_lossy().to_string(),
                content_path.to_string_lossy().to_string(),
                manifest_file.to_string_lossy().to_string(),
            ));
        }
        
        create_empty_folder(package_hash_path.to_string_lossy().as_ref()).await?;
        copy_dir_all(source_dst, content_path.to_string_lossy().as_ref()).await?;
        
        let manifest_string = serde_json::to_string(&manifest_json).unwrap_or_default();
        fs::write(&manifest_file, manifest_string).map_err(|e| AppError::new(&e.to_string()))?;
        
        Ok(Self::build_package_info(
            package_hash,
            package_hash_path.to_string_lossy().to_string(),
            content_path.to_string_lossy().to_string(),
            manifest_file.to_string_lossy().to_string(),
        ))
    }

    pub fn delete_package_tmp(config: &crate::config::AppConfig, package_hash: &str) -> bool {
        let data_dir = Self::get_data_dir(config);
        let package_hash_path = Path::new(&data_dir).join(package_hash);
        
        if package_hash_path.exists() {
            match fs::remove_dir_all(&package_hash_path) {
                Ok(_) => {
                    debug!("Successfully deleted package directory: {:?}", package_hash_path);
                    return true;
                }
                Err(e) => {
                    error!("Failed to delete package directory {:?}: {}", package_hash_path, e);
                    return false;
                }
            }
        }
        debug!("Package directory not found: {:?}", package_hash_path);
        false
    }

    pub fn delete_package_storage(config: &crate::config::AppConfig, package_hash: &str) -> bool {
        Self::delete_package_tmp(config, package_hash)
    }
}
