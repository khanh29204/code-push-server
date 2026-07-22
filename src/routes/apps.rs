use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};

use crate::core::app_error::AppError;
use crate::core::db::AppState;
use crate::routes::middleware::AuthUser;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_apps))
        .route(
            "/{app_name}/deployments",
            get(list_deployments).post(create_deployment),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}",
            get(get_deployment)
                .patch(update_deployment)
                .delete(delete_deployment),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/metrics",
            get(get_deployment_metrics),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/releases/{label}",
            delete(delete_release_by_label),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/history",
            get(get_deployment_history).delete(delete_deployment_history),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/release",
            post(release_package).patch(modify_release_package),
        )
        .route(
            "/{app_name}/deployments/{source_deployment_name}/promote/{dest_deployment_name}",
            post(promote_package),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/rollback/{label}",
            post(rollback_with_label),
        )
        .route(
            "/{app_name}/deployments/{deployment_name}/rollback",
            post(rollback),
        )
        .route("/{app_name}/collaborators", get(get_collaborators))
        .route(
            "/{app_name}/collaborators/{email}",
            post(add_collaborator).delete(remove_collaborator),
        )
        .route("/{app_name}/transfer/{email}", post(transfer_app))
        .route("/{app_name}", delete(delete_app).patch(rename_app))
        .route("/", post(create_app))
}

#[derive(Serialize)]
pub struct AppsResponse {
    pub apps: Vec<crate::services::app_manager::AppDetail>,
}

async fn list_apps(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
) -> Result<Json<AppsResponse>, AppError> {
    let apps = crate::services::app_manager::AppManager::list_apps(&state.db.pool, user.id).await?;
    Ok(Json(AppsResponse { apps }))
}

#[derive(Serialize)]
pub struct DeploymentsResponse {
    pub deployments: Vec<serde_json::Value>,
}

async fn format_deployment(
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

async fn list_deployments(
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

#[derive(Serialize)]
pub struct DeploymentResponse {
    pub deployment: serde_json::Value,
}

async fn get_deployment(
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

async fn create_deployment(
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

async fn get_deployment_metrics(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<Json<MetricsResponse>, AppError> {
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
    let dv = sqlx::query_as::<_, crate::models::deployments_versions::DeploymentVersion>(
        "SELECT * FROM deployments_versions WHERE id = ?",
    )
    .bind(dep.last_deployment_version_id)
    .fetch_optional(&state.db.pool)
    .await?;
    let package_id = dv.map(|v| v.current_package_id).unwrap_or(0);
    let metrics = crate::services::package_manager::PackageManager::get_metrics_by_package_id(
        &state.db.pool,
        package_id,
    )
    .await?;
    Ok(Json(MetricsResponse {
        metrics: serde_json::to_value(metrics).unwrap_or(serde_json::Value::Null),
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

async fn get_deployment_history(
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

async fn delete_deployment(
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

async fn update_deployment(
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

async fn delete_deployment_history(
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

async fn release_package(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    State(state): State<AppState>,
    mut multipart: axum::extract::Multipart,
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

    let mut package_info = crate::services::package_manager::ReleaseParams::default();
    let mut file_path = std::path::PathBuf::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::new(&e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "packageInfo" {
            let data = field.text().await.unwrap_or_default();
            if let Ok(info) = serde_json::from_str(&data) {
                package_info = info;
            }
        } else if name == "package" {
            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::new(&e.to_string()))?;
            let tmp_path =
                std::path::Path::new(&state.config.data_dir).join(uuid::Uuid::new_v4().to_string());
            tokio::fs::write(&tmp_path, data)
                .await
                .map_err(|e| AppError::new(&e.to_string()))?;
            file_path = tmp_path;
        }
    }

    if file_path.exists() {
        let package = crate::services::package_manager::PackageManager::release_package(
            &state.config,
            &state.db.pool,
            &state.storage,
            app.id,
            dep.id,
            package_info,
            &file_path,
            user.id,
        )
        .await?;
        let _ = tokio::fs::remove_file(file_path).await;

        let pool = state.db.pool.clone();
        let storage = state.storage.clone();
        let app_id = app.id;
        let diff_nums = state.config.diff_nums;
        let update_check_cache = state.config.update_check_cache;
        let redis_pool = state.db.redis.clone();
        let dep_key = dep.deployment_key.clone();
        let config = state.config.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if let Err(e) =
                crate::services::package_manager::PackageManager::create_diff_packages_by_last_nums(
                    &config,
                    &pool,
                    &storage,
                    app_id,
                    &package,
                    diff_nums.into(),
                )
                .await
            {
                tracing::error!("Failed to create diff packages: {:?}", e);
            }
            if update_check_cache {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                crate::services::client_manager::ClientManager::clear_update_check_cache(
                    &redis_pool,
                    &dep_key,
                )
                .await;
            }
        });
    }
    Ok("{\"msg\": \"succeed\"}")
}

#[derive(Deserialize)]
pub struct ModifyReleasePackageRequest {
    #[serde(rename = "packageInfo")]
    pub package_info: serde_json::Value,
}

async fn modify_release_package(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<ModifyReleasePackageRequest>,
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
    let params: crate::services::package_manager::ReleaseParams =
        serde_json::from_value(payload.package_info).unwrap_or_default();

    let package_id = if let Some(label) = &params.label {
        let pkg = crate::services::package_manager::PackageManager::find_package_info_by_deployment_id_and_label(&state.db.pool, dep.id, label)
            .await?.ok_or_else(|| AppError::NotFound("Package not found".into()))?;
        pkg.id
    } else {
        let dv = sqlx::query_as::<_, crate::models::deployments_versions::DeploymentVersion>(
            "SELECT * FROM deployments_versions WHERE id = ?",
        )
        .bind(dep.last_deployment_version_id)
        .fetch_optional(&state.db.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("DeploymentVersion not found".into()))?;
        dv.current_package_id
    };

    crate::services::package_manager::PackageManager::modify_release_package(
        &state.db.pool,
        package_id,
        params,
    )
    .await?;

    if state.config.update_check_cache {
        let redis_pool = state.db.redis.clone();
        let dep_key = dep.deployment_key.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
            crate::services::client_manager::ClientManager::clear_update_check_cache(
                &redis_pool,
                &dep_key,
            )
            .await;
        });
    }

    Ok("ok")
}

#[derive(Deserialize)]
pub struct PromotePackageRequest {
    #[serde(rename = "packageInfo")]
    pub package_info: serde_json::Value,
}

#[derive(Serialize)]
pub struct PromoteResponse {
    pub package: serde_json::Value,
}

async fn promote_package(
    AuthUser(user): AuthUser,
    Path((app_name, source_deployment_name, dest_deployment_name)): Path<(String, String, String)>,
    State(state): State<AppState>,
    Json(payload): Json<PromotePackageRequest>,
) -> Result<Json<PromoteResponse>, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let source_dep = sqlx::query_as::<_, crate::models::deployments::Deployment>(
        "SELECT * FROM deployments WHERE appid = ? AND name = ?",
    )
    .bind(app.id)
    .bind(&source_deployment_name)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Source deployment not found".into()))?;
    let dest_dep = sqlx::query_as::<_, crate::models::deployments::Deployment>(
        "SELECT * FROM deployments WHERE appid = ? AND name = ?",
    )
    .bind(app.id)
    .bind(&dest_deployment_name)
    .fetch_optional(&state.db.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Dest deployment not found".into()))?;

    let params: crate::services::package_manager::ReleaseParams =
        serde_json::from_value(payload.package_info).unwrap_or_default();
    let package = crate::services::package_manager::PackageManager::promote_package(
        &state.db.pool,
        &source_dep,
        &dest_dep,
        params,
        user.id,
    )
    .await?;

    let pool = state.db.pool.clone();
    let storage = state.storage.clone();
    let app_id = app.id;
    let diff_nums: i64 = state.config.diff_nums.into();
    let pkg_clone = package.clone();
    let update_check_cache = state.config.update_check_cache;
    let redis_pool = state.db.redis.clone();
    let dep_key = dest_dep.deployment_key.clone();
    let config = state.config.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if let Err(e) =
            crate::services::package_manager::PackageManager::create_diff_packages_by_last_nums(
                &config, &pool, &storage, app_id, &pkg_clone, diff_nums,
            )
            .await
        {
            tracing::error!("Failed to create diff packages on promote: {:?}", e);
        }
        if update_check_cache {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            crate::services::client_manager::ClientManager::clear_update_check_cache(
                &redis_pool,
                &dep_key,
            )
            .await;
        }
    });

    Ok(Json(PromoteResponse {
        package: serde_json::to_value(package)
            .ok()
            .unwrap_or(serde_json::Value::Null),
    }))
}

async fn rollback(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<&'static str, AppError> {
    handle_rollback(user, app_name, deployment_name, None, state).await
}

async fn rollback_with_label(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name, label)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Result<&'static str, AppError> {
    handle_rollback(user, app_name, deployment_name, Some(label), state).await
}

async fn handle_rollback(
    user: crate::models::users::User,
    app_name: String,
    deployment_name: String,
    label: Option<String>,
    state: AppState,
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

    let package = crate::services::package_manager::PackageManager::rollback_package(
        &state.db.pool,
        dep.last_deployment_version_id,
        label.as_deref(),
        user.id,
    )
    .await?;

    let pool = state.db.pool.clone();
    let storage = state.storage.clone();
    let app_id = app.id;
    let update_check_cache = state.config.update_check_cache;
    let redis_pool = state.db.redis.clone();
    let dep_key = dep.deployment_key.clone();
    let config = state.config.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if let Err(e) =
            crate::services::package_manager::PackageManager::create_diff_packages_by_last_nums(
                &config, &pool, &storage, app_id, &package, 1,
            )
            .await
        {
            tracing::error!("Failed to create diff packages on rollback: {:?}", e);
        }
        if update_check_cache {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            crate::services::client_manager::ClientManager::clear_update_check_cache(
                &redis_pool,
                &dep_key,
            )
            .await;
        }
    });

    Ok("ok")
}

#[derive(Serialize)]
pub struct CollaboratorsResponse {
    pub collaborators: serde_json::Value,
}

async fn get_collaborators(
    AuthUser(user): AuthUser,
    Path(app_name): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<CollaboratorsResponse>, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let cols = crate::services::collaborators_manager::CollaboratorsManager::list_collaborators(
        &state.db.pool,
        app.id,
    )
    .await?;
    Ok(Json(CollaboratorsResponse {
        collaborators: serde_json::to_value(cols).unwrap_or(serde_json::Value::Null),
    }))
}

#[derive(Deserialize)]
pub struct AddCollaboratorRequest {
    pub email: String,
}

async fn add_collaborator(
    AuthUser(user): AuthUser,
    Path((app_name, email)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<&'static str, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let target_user =
        sqlx::query_as::<_, crate::models::users::User>("SELECT * FROM users WHERE email = ?")
            .bind(&email)
            .fetch_optional(&state.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    crate::services::collaborators_manager::CollaboratorsManager::add_collaborator(
        &state.db.pool,
        app.id,
        target_user.id,
    )
    .await?;
    Ok("ok")
}

async fn remove_collaborator(
    AuthUser(user): AuthUser,
    Path((app_name, email)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<&'static str, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let target_user =
        sqlx::query_as::<_, crate::models::users::User>("SELECT * FROM users WHERE email = ?")
            .bind(&email)
            .fetch_optional(&state.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    crate::services::collaborators_manager::CollaboratorsManager::delete_collaborator(
        &state.db.pool,
        app.id,
        target_user.id,
    )
    .await?;
    Ok("ok")
}

async fn delete_app(
    AuthUser(user): AuthUser,
    Path(app_name): Path<String>,
    State(state): State<AppState>,
) -> Result<&'static str, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    crate::services::app_manager::AppManager::delete_app(&state.db.pool, app.id).await?;
    Ok("ok")
}

#[derive(Deserialize)]
pub struct RenameAppRequest {
    pub name: String,
}

async fn rename_app(
    AuthUser(user): AuthUser,
    Path(app_name): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<RenameAppRequest>,
) -> Result<&'static str, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    crate::services::app_manager::AppManager::modify_app(
        &state.db.pool,
        app.id,
        Some(&payload.name),
        None,
        None,
    )
    .await?;
    Ok("ok")
}

#[derive(Deserialize)]
pub struct TransferAppRequest {
    pub email: String,
}

async fn transfer_app(
    AuthUser(user): AuthUser,
    Path((app_name, email)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<&'static str, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let target_user =
        sqlx::query_as::<_, crate::models::users::User>("SELECT * FROM users WHERE email = ?")
            .bind(&email)
            .fetch_optional(&state.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    crate::services::app_manager::AppManager::transfer_app(
        &state.db.pool,
        app.id,
        user.id,
        target_user.id,
    )
    .await?;
    Ok("ok")
}

#[derive(Deserialize)]
pub struct CreateAppRequest {
    pub name: String,
    pub os: String,
    pub platform: String,
}

#[derive(Serialize)]
pub struct CreateAppResponse {
    pub app: serde_json::Value,
}

async fn create_app(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateAppRequest>,
) -> Result<Json<CreateAppResponse>, AppError> {
    let os = match payload.os.as_str() {
        "iOS" => 1,
        "Android" => 2,
        "Windows" => 3,
        _ => 0,
    };
    let platform = match payload.platform.as_str() {
        "React-Native" => 1,
        "Cordova" => 2,
        _ => 0,
    };
    crate::services::app_manager::AppManager::add_app(
        &state.db.pool,
        user.id,
        &payload.name,
        os,
        platform,
        &user.identical,
    )
    .await?;
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &payload.name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", payload.name)))?;
    let val = serde_json::json!({
        "name": app.name,
        "os": payload.os,
        "platform": payload.platform,
    });
    Ok(Json(CreateAppResponse { app: val }))
}

async fn delete_release_by_label(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name, label)): Path<(String, String, String)>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
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

    // Check ownership
    crate::services::account_manager::AccountManager::owner_can(&state.db.pool, user.id, &app_name)
        .await?;

    // Find package and delete storage
    if let Some(pkg) = sqlx::query_as::<_, crate::models::packages::Packages>(
        "SELECT * FROM packages WHERE deployment_id = ? AND label = ?",
    )
    .bind(dep.id)
    .bind(&label)
    .fetch_optional(&state.db.pool)
    .await?
    {
        if !pkg.blob_url.is_empty() {
            state.storage.delete_file(&pkg.blob_url).await.ok();
        }
        if !pkg.manifest_blob_url.is_empty() {
            state.storage.delete_file(&pkg.manifest_blob_url).await.ok();
        }
        // Note: The database record is NOT deleted, just the storage files to save space, matching Node.js logic!
    } else {
        return Err(AppError::NotFound("Package not found.".into()));
    }

    Ok(Json(serde_json::json!("ok")))
}
