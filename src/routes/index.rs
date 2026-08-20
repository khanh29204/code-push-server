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

/// Body of a `reportStatus/*` call. See the v0.1 counterpart for why every
/// field must tolerate being absent.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ReportStatusBody {
    pub client_unique_id: Option<String>,
    pub label: Option<String>,
    pub deployment_key: Option<String>,
    pub status: Option<String>,
    pub previous_deployment_key: Option<String>,
    pub previous_label_or_app_version: Option<String>,
}

async fn report_status_download(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> &'static str {
    let body = serde_json::from_slice::<ReportStatusBody>(&body).unwrap_or_default();

    let _ = ClientManager::report_status_download(
        &state.db.pool,
        body.deployment_key.as_deref().unwrap_or(""),
        body.label.as_deref().unwrap_or(""),
        body.client_unique_id.as_deref().unwrap_or(""),
    )
    .await;

    "OK"
}

async fn report_status_deploy(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> &'static str {
    let body = serde_json::from_slice::<ReportStatusBody>(&body).unwrap_or_default();

    let _ = ClientManager::report_status_deploy(
        &state.db.pool,
        body.deployment_key.as_deref().unwrap_or(""),
        body.label.as_deref().unwrap_or(""),
        body.client_unique_id.as_deref().unwrap_or(""),
        body.status.as_deref(),
        body.previous_deployment_key.as_deref(),
        body.previous_label_or_app_version.as_deref(),
    )
    .await;

    "OK"
}

