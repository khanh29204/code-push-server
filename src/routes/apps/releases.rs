use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};

use crate::core::app_error::AppError;
use crate::core::db::AppState;
use crate::routes::middleware::AuthUser;

pub async fn release_package(
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

pub async fn modify_release_package(
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

pub async fn promote_package(
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

pub async fn rollback(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<&'static str, AppError> {
    handle_rollback(user, app_name, deployment_name, None, state).await
}

pub async fn rollback_with_label(
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
    let diff_nums: i64 = state.config.diff_nums.into();
    let update_check_cache = state.config.update_check_cache;
    let redis_pool = state.db.redis.clone();
    let dep_key = dep.deployment_key.clone();
    let config = state.config.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if let Err(e) =
            crate::services::package_manager::PackageManager::create_diff_packages_by_last_nums(
                &config, &pool, &storage, app_id, &package, diff_nums,
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

pub async fn delete_release_by_label(
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

    crate::services::account_manager::AccountManager::owner_can(&state.db.pool, user.id, &app_name)
        .await?;

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
    } else {
        return Err(AppError::NotFound("Package not found.".into()));
    }

    Ok(Json(serde_json::json!("ok")))
}

pub async fn get_release_by_label(
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

    let pkg = crate::services::package_manager::PackageManager::find_package_info_by_deployment_id_and_label(
        &state.db.pool,
        dep.id,
        &label,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Package not found".into()))?;

    Ok(Json(serde_json::to_value(&pkg).unwrap_or_default()))
}

pub async fn modify_release_package_with_label(
    AuthUser(user): AuthUser,
    Path((app_name, deployment_name, label)): Path<(String, String, String)>,
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

    let mut params: crate::services::package_manager::ReleaseParams =
        serde_json::from_value(payload.package_info).unwrap_or_default();
    params.label = Some(label.clone());

    let pkg = crate::services::package_manager::PackageManager::find_package_info_by_deployment_id_and_label(
        &state.db.pool,
        dep.id,
        &label,
    )
    .await?
    .ok_or_else(|| AppError::NotFound("Package not found".into()))?;

    crate::services::package_manager::PackageManager::modify_release_package(
        &state.db.pool,
        pkg.id,
        params,
    )
    .await?;

    Ok("ok")
}
