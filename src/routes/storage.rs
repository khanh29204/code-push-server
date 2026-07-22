use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;

use crate::core::app_error::AppError;
use crate::core::db::AppState;
use crate::routes::middleware::AuthUser;

pub fn router() -> Router<AppState> {
    Router::new().route("/audit", get(storage_audit).delete(delete_storage_audit))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSummary {
    pub total_files_on_disk: usize,
    pub total_size_on_disk: String,
    pub total_packages_in_db: usize,
    pub orphaned_files_count: usize,
    pub missing_files_count: usize,
    pub valid_files_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingFile {
    pub package_id: i64,
    pub label: String,
    pub r#type: String,
    pub hash: String,
    pub expected_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrphanedFile {
    pub hash: String,
    pub size: String,
    pub path: String,
    pub modified: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidFile {
    pub package_id: i64,
    pub label: String,
    pub r#type: String,
    pub hash: String,
    pub size: String,
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReport {
    pub summary: AuditSummary,
    pub missing_files: Vec<MissingFile>,
    pub orphaned_files: Vec<OrphanedFile>,
    pub valid_files: Vec<ValidFile>,
}

#[derive(sqlx::FromRow)]
struct DbPackage {
    id: i64,
    label: String,
    package_hash: Option<String>,
    blob_url: Option<String>,
    manifest_blob_url: Option<String>,
}

struct DiskFileInfo {
    hash: String,
    size: u64,
    full_path: String,
    modified_time: chrono::DateTime<chrono::Utc>,
}

pub async fn storage_audit(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<AuditReport>, AppError> {
    if state.config.storage_type != "local" {
        return Err(AppError::new(&format!(
            "Audit API not supported for storageType: {}",
            state.config.storage_type
        )));
    }

    let storage_dir = &state.config.local_storage_dir;
    if !std::path::Path::new(storage_dir).exists() {
        return Err(AppError::new(&format!(
            "Storage directory does not exist: {}",
            storage_dir
        )));
    }

    let mut disk_file_map = std::collections::HashMap::new();
    let mut total_size_bytes: u64 = 0;

    for entry in walkdir::WalkDir::new(storage_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file()
            && let Ok(metadata) = entry.metadata()
        {
            let size = metadata.len();
            total_size_bytes += size;

            let file_name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path().to_string_lossy().into_owned();
            let modified: std::time::SystemTime = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            let modified_time: chrono::DateTime<chrono::Utc> = modified.into();

            disk_file_map.insert(
                file_name.clone(),
                DiskFileInfo {
                    hash: file_name,
                    size,
                    full_path: path,
                    modified_time,
                },
            );
        }
    }

    let db_packages = sqlx::query_as::<_, DbPackage>(
        "SELECT id, label, package_hash, blob_url, manifest_blob_url FROM packages",
    )
    .fetch_all(&state.db.pool)
    .await
    .map_err(|e| AppError::new(&e.to_string()))?;

    let mut report = AuditReport {
        summary: AuditSummary {
            total_files_on_disk: disk_file_map.len(),
            total_size_on_disk: format!("{:.2} MB", total_size_bytes as f64 / 1024.0 / 1024.0),
            total_packages_in_db: db_packages.len(),
            orphaned_files_count: 0,
            missing_files_count: 0,
            valid_files_count: 0,
        },
        missing_files: Vec::new(),
        orphaned_files: Vec::new(),
        valid_files: Vec::new(),
    };

    let mut valid_hashes = std::collections::HashSet::new();

    for pkg in &db_packages {
        let mut check_file = |hash: &str, file_type: &str| {
            if hash.is_empty() {
                return;
            }
            valid_hashes.insert(hash.to_string());

            if let Some(file_info) = disk_file_map.get(hash) {
                report.valid_files.push(ValidFile {
                    package_id: pkg.id,
                    label: pkg.label.clone(),
                    r#type: file_type.to_string(),
                    hash: hash.to_string(),
                    size: format!("{:.2} MB", file_info.size as f64 / 1024.0 / 1024.0),
                    path: file_info.full_path.clone(),
                });
            } else {
                let expected_path = std::path::Path::new(storage_dir)
                    .join(hash[0..2].to_lowercase())
                    .join(hash)
                    .to_string_lossy()
                    .into_owned();

                report.missing_files.push(MissingFile {
                    package_id: pkg.id,
                    label: pkg.label.clone(),
                    r#type: file_type.to_string(),
                    hash: hash.to_string(),
                    expected_path,
                });
            }
        };

        if let Some(blob_url) = &pkg.blob_url {
            check_file(blob_url, "blob");
        }
        if let Some(manifest_url) = &pkg.manifest_blob_url {
            check_file(manifest_url, "manifest");
        }
    }

    for (hash, info) in &disk_file_map {
        if !valid_hashes.contains(hash) {
            report.orphaned_files.push(OrphanedFile {
                hash: hash.clone(),
                size: format!("{:.2} MB", info.size as f64 / 1024.0 / 1024.0),
                path: info.full_path.clone(),
                modified: info
                    .modified_time
                    .format("%Y-%m-%dT%H:%M:%S.%3fZ")
                    .to_string(),
            });
        }
    }

    report.summary.valid_files_count = report.valid_files.len();
    report.summary.missing_files_count = report.missing_files.len();
    report.summary.orphaned_files_count = report.orphaned_files.len();

    report
        .valid_files
        .sort_by(|a, b| b.package_id.cmp(&a.package_id));

    Ok(Json(report))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAuditResponse {
    pub status: String,
    pub deleted_count: usize,
    pub freed_size: String,
    pub deleted_files: Vec<String>,
}

pub async fn delete_storage_audit(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<DeleteAuditResponse>, AppError> {
    if state.config.storage_type != "local" {
        return Err(AppError::new(&format!(
            "Audit API not supported for storageType: {}",
            state.config.storage_type
        )));
    }

    let storage_dir = &state.config.local_storage_dir;
    if !std::path::Path::new(storage_dir).exists() {
        return Err(AppError::new(&format!(
            "Storage directory does not exist: {}",
            storage_dir
        )));
    }

    let db_packages = sqlx::query_as::<_, DbPackage>(
        "SELECT id, label, package_hash, blob_url, manifest_blob_url FROM packages",
    )
    .fetch_all(&state.db.pool)
    .await
    .map_err(|e| AppError::new(&e.to_string()))?;

    let mut valid_hashes = std::collections::HashSet::new();
    for pkg in &db_packages {
        if let Some(blob_url) = &pkg.blob_url {
            if !blob_url.is_empty() {
                valid_hashes.insert(blob_url.clone());
            }
        }
        if let Some(manifest_url) = &pkg.manifest_blob_url {
            if !manifest_url.is_empty() {
                valid_hashes.insert(manifest_url.clone());
            }
        }
    }

    let mut freed_bytes: u64 = 0;
    let mut deleted_files = Vec::new();
    let ten_mins_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(600);

    for entry in walkdir::WalkDir::new(storage_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if !valid_hashes.contains(&file_name) {
                if let Ok(metadata) = entry.metadata() {
                    let modified = metadata
                        .modified()
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    if modified < ten_mins_ago {
                        freed_bytes += metadata.len();
                        let path = entry.path().to_path_buf();
                        if tokio::fs::remove_file(&path).await.is_ok() {
                            deleted_files.push(path.to_string_lossy().into_owned());
                        }
                    }
                }
            }
        }
    }

    Ok(Json(DeleteAuditResponse {
        status: "OK".to_string(),
        deleted_count: deleted_files.len(),
        freed_size: format!("{:.2} MB", freed_bytes as f64 / 1024.0 / 1024.0),
        deleted_files,
    }))
}
