use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};

use crate::core::app_error::AppError;
use crate::core::db::AppState;
use crate::routes::middleware::AuthUser;

#[derive(Serialize)]
pub struct DeploymentsResponse {
    pub deployments: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct DeploymentResponse {
    pub deployment: serde_json::Value,
}

pub async fn format_deployment(
    config: &crate::config::AppConfig,
    pool: &sqlx::SqlitePool,
    d: &crate::models::deployments::Deployment,
) -> serde_json::Value {
    let mut package_val = serde_json::Value::Null;
    if d.last_deployment_version_id > 0
        && let Ok(Some(dv)) = sqlx::query_as::<
            _,
            crate::models::deployments_versions::DeploymentVersion,
        >("SELECT * FROM deployments_versions WHERE id = ?")
        .bind(d.last_deployment_version_id)
        .fetch_optional(pool)
        .await
        && let Ok(Some(pkg)) = sqlx::query_as::<_, crate::models::packages::Packages>(
            "SELECT * FROM packages WHERE id = ?",
        )
        .bind(dv.current_package_id)
        .fetch_optional(pool)
        .await
        && let Ok(formatted) =
            crate::services::deployments_manager::DeploymentsManager::format_package(
                config, pool, &pkg,
            )
            .await
    {
        package_val = formatted;
    }
    serde_json::json!({
        "createdTime": d.created_at.map(|t| t.and_utc().timestamp_millis()).unwrap_or(0),
        "id": d.id.to_string(),
        "name": d.name,
        "key": d.deployment_key,
        "package": package_val,
    })
}

pub async fn list_deployments(
    AuthUser(user): AuthUser,
    Path(app_name): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<DeploymentsResponse>, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let deployments = sqlx::query_as::<_, crate::models::deployments::Deployment>(
        "SELECT * FROM deployments WHERE appid = ?",
    )
    .bind(app.id)
    .fetch_all(&state.db.pool)
    .await?;
    let mut mapped = Vec::new();
    for d in deployments {
        mapped.push(format_deployment(&state.config, &state.db.pool, &d).await);
    }
    Ok(Json(DeploymentsResponse {
        deployments: mapped,
    }))
}

pub async fn get_deployment(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<DeploymentResponse>, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let dep = sqlx::query_as::<_, crate::models::deployments::Deployment>(
        "SELECT * FROM deployments WHERE appid = ? AND name = ?",
    )
    .bind(app.id)
    .bind(&deployment_name)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Deployment not found".into()))?;
    Ok(Json(DeploymentResponse {
        deployment: format_deployment(&state.config, &state.db.pool, &dep).await,
    }))
}

#[derive(Deserialize)]
pub struct CreateDeploymentRequest {
    pub name: String,
}

pub async fn create_deployment(
    AuthUser(user): AuthUser,
    Path(app_name): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<CreateDeploymentRequest>,
) -> Result<Json<DeploymentResponse>, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let dep = crate::services::deployments_manager::DeploymentsManager::add_deployment(
        &state.db.pool,
        &payload.name,
        app.id,
        user.id,
    )
    .await?;
    Ok(Json(DeploymentResponse {
        deployment: format_deployment(&state.config, &state.db.pool, &dep).await,
    }))
}

#[derive(Serialize)]
pub struct MetricsResponse {
    pub metrics: serde_json::Value,
}

pub async fn get_deployment_metrics(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<MetricsResponse>, AppError> {
    use sqlx::Row;
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let dep = sqlx::query_as::<_, crate::models::deployments::Deployment>(
        "SELECT * FROM deployments WHERE appid = ? AND name = ?",
    )
    .bind(app.id)
    .bind(&deployment_name)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Deployment not found".into()))?;

    let rows = sqlx::query(
        "SELECT p.label, pm.active, pm.downloaded, pm.failed, pm.installed FROM packages p JOIN packages_metrics pm ON p.id = pm.package_id WHERE p.deployment_id = ?"
    )
    .bind(dep.id)
    .fetch_all(&state.db.pool)
    .await?;

    let mut metrics_map = serde_json::Map::new();
    for row in rows {
        let label: String = row.get("label");
        let active: i64 = row.get("active");
        let downloaded: i64 = row.get("downloaded");
        let failed: i64 = row.get("failed");
        let installed: i64 = row.get("installed");
        metrics_map.insert(
            label,
            serde_json::json!({
                "active": active,
                "downloaded": downloaded,
                "failed": failed,
                "installed": installed,
            }),
        );
    }

    Ok(Json(MetricsResponse {
        metrics: serde_json::Value::Object(metrics_map),
    }))
}

#[derive(Serialize)]
pub struct HistoryResponse {
    pub history: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub limit: Option<usize>,
}

pub async fn get_deployment_history(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    Query(query): Query<HistoryQuery>,
    State(state): State<AppState>,
) -> Result<Json<HistoryResponse>, AppError> {
    let limit = query.limit.unwrap_or(15);
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let dep = sqlx::query_as::<_, crate::models::deployments::Deployment>(
        "SELECT * FROM deployments WHERE appid = ? AND name = ?",
    )
    .bind(app.id)
    .bind(&deployment_name)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Deployment not found".into()))?;
    let packages =
        crate::services::deployments_manager::DeploymentsManager::get_deployment_history(
            &state.db.pool,
            dep.id,
            limit,
        )
        .await?;

    let mut history = Vec::new();
    for p in packages {
        if let Ok(formatted) =
            crate::services::deployments_manager::DeploymentsManager::format_package(
                &state.config,
                &state.db.pool,
                &p,
            )
            .await
        {
            history.push(formatted);
        }
    }

    Ok(Json(HistoryResponse { history }))
}

pub async fn delete_deployment(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<DeploymentResponse>, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    crate::services::deployments_manager::DeploymentsManager::delete_deployment_by_name(
        &state.db.pool,
        &deployment_name,
        app.id,
    )
    .await?;
    Ok(Json(DeploymentResponse {
        deployment: serde_json::Value::Null,
    }))
}

#[derive(Deserialize)]
pub struct UpdateDeploymentRequest {
    pub name: String,
}

pub async fn update_deployment(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateDeploymentRequest>,
) -> Result<Json<DeploymentResponse>, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    crate::services::deployments_manager::DeploymentsManager::rename_deployment_by_name(
        &state.db.pool,
        &deployment_name,
        app.id,
        &payload.name,
    )
    .await?;
    let dep = sqlx::query_as::<_, crate::models::deployments::Deployment>(
        "SELECT * FROM deployments WHERE appid = ? AND name = ?",
    )
    .bind(app.id)
    .bind(&payload.name)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Deployment not found".into()))?;
    Ok(Json(DeploymentResponse {
        deployment: serde_json::json!({ "name": dep.name, "key": dep.deployment_key }),
    }))
}

pub async fn delete_deployment_history(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<&'static str, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let dep = sqlx::query_as::<_, crate::models::deployments::Deployment>(
        "SELECT * FROM deployments WHERE appid = ? AND name = ?",
    )
    .bind(app.id)
    .bind(&deployment_name)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Deployment not found".into()))?;
    crate::services::deployments_manager::DeploymentsManager::delete_deployment_history(
        &state.db.pool,
        dep.id,
    )
    .await?;
    Ok("ok")
}
