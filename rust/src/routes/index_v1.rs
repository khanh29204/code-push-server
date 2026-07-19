use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::core::db::AppState;
use crate::services::client_manager::ClientManager;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/update_check", get(update_check))
        .route("/report_status/download", post(report_status_download))
        .route("/report_status/deploy", post(report_status_deploy))
}

#[derive(Deserialize)]
pub struct UpdateCheckQuery {
    pub deployment_key: String,
    pub app_version: String,
    pub label: Option<String>,
    pub package_hash: Option<String>,
    pub client_unique_id: Option<String>,
}

#[derive(Serialize)]
pub struct UpdateCheckResponse {
    pub update_info: UpdateInfo,
}

#[derive(Serialize)]
pub struct UpdateInfo {
    pub download_url: Option<String>,
    pub description: Option<String>,
    pub is_available: bool,
    pub is_disabled: bool,
    pub is_mandatory: bool,
    pub target_binary_range: Option<String>,
    pub package_hash: Option<String>,
    pub label: Option<String>,
    pub package_size: Option<i64>,
    pub update_app_version: Option<bool>,
    pub should_run_binary_version: Option<bool>,
}

async fn update_check(
    State(state): State<AppState>,
    Query(query): Query<UpdateCheckQuery>,
) -> Result<Json<UpdateCheckResponse>, crate::core::app_error::AppError> {
    let rs = ClientManager::update_check(
        &state.db.pool,
        &state.db.redis,
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
            ).await?;

            if !data {
                info.is_available = false;
            }
            
            UpdateInfo {
                download_url: Some(info.download_url),
                description: Some(info.description),
                is_available: info.is_available,
                is_disabled: info.is_disabled,
                is_mandatory: info.is_mandatory,
                target_binary_range: Some(info.target_binary_range),
                package_hash: Some(info.package_hash),
                label: Some(info.label),
                package_size: Some(info.package_size),
                update_app_version: None, // Or parse if available
                should_run_binary_version: None, // Or parse if available
            }
        },
        None => UpdateInfo {
            download_url: None,
            description: None,
            is_available: false,
            is_disabled: false,
            is_mandatory: false,
            target_binary_range: None,
            package_hash: None,
            label: None,
            package_size: None,
            update_app_version: None,
            should_run_binary_version: None,
        }
    };

    Ok(Json(UpdateCheckResponse { update_info: rs }))
}

#[derive(Deserialize)]
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
    ).await;

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
    ).await;

    "OK"
}
