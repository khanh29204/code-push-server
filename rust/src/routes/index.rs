use axum::{
    Json, Router,
    extract::{Query, State},
    response::Html,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::core::db::AppState;
use crate::services::client_manager::ClientManager;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/tokens", get(tokens))
        .route("/authenticated", get(authenticated))
        .route("/updateCheck", get(update_check))
        .route("/reportStatus/download", post(report_status_download))
        .route("/reportStatus/deploy", post(report_status_deploy))
        .route("/storage/audit", get(storage_audit))
        .route("/config", get(get_config))
}

async fn index() -> Html<String> {
    Html(crate::utils::common::read_public_html("index.html"))
}

async fn tokens() -> Html<String> {
    Html(crate::utils::common::read_public_html("tokens.html"))
}

async fn authenticated(_user: crate::routes::middleware::AuthUser) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "authenticated": true }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub allow_registration: bool,
}

async fn get_config(State(state): State<AppState>) -> Json<ConfigResponse> {
    Json(ConfigResponse {
        allow_registration: state.config.allow_registration,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckQuery {
    pub deployment_key: String,
    pub app_version: String,
    pub label: Option<String>,
    pub package_hash: Option<String>,
    pub client_unique_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResponse {
    pub update_info: UpdateInfo,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub download_url: Option<String>,
    pub description: Option<String>,
    pub is_available: bool,
    pub is_disabled: bool,
    pub is_mandatory: bool,
    pub app_version: Option<String>,
    pub package_hash: Option<String>,
    pub label: Option<String>,
    pub package_size: Option<i64>,
    pub update_app_version: bool,
    pub should_run_binary_version: bool,
    pub rollout: Option<i64>,
}

async fn update_check(
    State(state): State<AppState>,
    Query(query): Query<UpdateCheckQuery>,
) -> Result<Json<UpdateCheckResponse>, crate::core::app_error::AppError> {
    let rs = ClientManager::update_check(
        &state,
        &query.deployment_key,
        &query.app_version,
        query.label.as_deref().unwrap_or(""),
        query.package_hash.as_deref().unwrap_or(""),
        query.client_unique_id.as_deref().unwrap_or(""),
    )
    .await?;

    let rs = match rs {
        Some(mut info) => {
            let data = ClientManager::chosen_man(
                &state.db.pool,
                &state.db.redis,
                info.package_id,
                Some(info.rollout),
                query.client_unique_id.as_deref().unwrap_or(""),
            )
            .await?;

            if !data {
                info.is_available = false;
            }

            UpdateInfo {
                download_url: Some(info.download_url),
                description: Some(info.description),
                is_available: info.is_available,
                is_disabled: info.is_disabled,
                is_mandatory: info.is_mandatory,
                app_version: Some(info.app_version),
                package_hash: Some(info.package_hash),
                label: Some(info.label),
                package_size: Some(info.package_size),
                update_app_version: false,
                should_run_binary_version: false,
                rollout: None,
            }
        }
        None => UpdateInfo {
            download_url: None,
            description: None,
            is_available: false,
            is_disabled: false,
            is_mandatory: false,
            app_version: None,
            package_hash: None,
            label: None,
            package_size: None,
            update_app_version: false,
            should_run_binary_version: false,
            rollout: None,
        },
    };

    Ok(Json(UpdateCheckResponse { update_info: rs }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportStatusBody {
    pub client_unique_id: String,
    pub label: String,
    pub deployment_key: String,
    pub status: Option<String>,
    pub previous_deployment_key: Option<String>,
    pub previous_label_or_app_version: Option<String>,
}

async fn report_status_download(
    State(state): State<AppState>,
    Json(body): Json<ReportStatusBody>,
) -> &'static str {
    let _ = ClientManager::report_status_download(
        &state.db.pool,
        &body.deployment_key,
        &body.label,
        &body.client_unique_id,
    )
    .await;

    "OK"
}

async fn report_status_deploy(
    State(state): State<AppState>,
    Json(body): Json<ReportStatusBody>,
) -> &'static str {
    let _ = ClientManager::report_status_deploy(
        &state.db.pool,
        &body.deployment_key,
        &body.label,
        &body.client_unique_id,
        body.status.as_deref(),
        body.previous_deployment_key.as_deref(),
        body.previous_label_or_app_version.as_deref(),
    )
    .await;

    "OK"
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

async fn storage_audit(
    _user: crate::routes::middleware::AuthUser,
    State(state): State<AppState>,
) -> Result<Json<AuditReport>, crate::core::app_error::AppError> {
    if state.config.storage_type != "local" {
        return Err(crate::core::app_error::AppError::new(&format!(
            "Audit API not supported for storageType: {}",
            state.config.storage_type
        )));
    }

    let storage_dir = &state.config.local_storage_dir;
    if !std::path::Path::new(storage_dir).exists() {
        return Err(crate::core::app_error::AppError::new(&format!(
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
    .map_err(|e| crate::core::app_error::AppError::new(&e.to_string()))?;

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
