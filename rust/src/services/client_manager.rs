use sqlx::SqlitePool;
use bb8_redis::{bb8, RedisConnectionManager, redis::AsyncCommands};
use serde::{Deserialize, Serialize};
use rand::Rng;

use crate::core::app_error::AppError;
use crate::models::deployments::Deployment;
use crate::models::deployments_versions::DeploymentVersion;
use crate::models::packages::Packages;
use crate::models::packages_diff::PackagesDiff;
use crate::utils::common::{parse_version, get_blob_download_url};

#[derive(Serialize, Deserialize)]
pub struct UpdateCheckInfo {
    pub package_id: i64,
    pub download_url: String,
    pub description: String,
    pub is_available: bool,
    pub is_disabled: bool,
    pub is_mandatory: bool,
    pub app_version: String,
    pub target_binary_range: String,
    pub package_hash: String,
    pub label: String,
    pub package_size: i64,
    pub rollout: i64,
}

pub struct ClientManager;

impl ClientManager {
    async fn compute_merged_info(
        pool: &SqlitePool,
        deployment_id: i64,
        deployment_version_id: i64,
        min_id: i64,
        target_id: i64,
    ) -> Result<(String, bool), AppError> {
        let has_mandatory: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM packages WHERE deployment_id = ? AND deployment_version_id = ? AND id > ? AND id <= ? AND is_disabled = 0 AND is_mandatory = 1"
        )
        .bind(deployment_id)
        .bind(deployment_version_id)
        .bind(min_id)
        .bind(target_id)
        .fetch_one(pool)
        .await?;

        let intermediate_packages = sqlx::query_as::<_, Packages>(
            "SELECT * FROM packages WHERE deployment_id = ? AND deployment_version_id = ? AND id > ? AND id <= ? AND is_disabled = 0 ORDER BY id DESC LIMIT 15"
        )
        .bind(deployment_id)
        .bind(deployment_version_id)
        .bind(min_id)
        .bind(target_id)
        .fetch_all(pool)
        .await?;

        let mut messages = Vec::new();
        for pkg in intermediate_packages {
            if !pkg.description.is_empty() {
                let pkg_label = if pkg.label.is_empty() { "unknown" } else { &pkg.label };
                messages.push(format!("[{}]: {}", pkg_label, pkg.description));
            }
        }

        let description = if messages.is_empty() {
            "".to_string()
        } else {
            messages.join("\n")
        };

        Ok((description, has_mandatory > 0))
    }

    pub async fn clear_update_check_cache(
        redis: &bb8_redis::bb8::Pool<bb8_redis::RedisConnectionManager>,
        deployment_key: &str,
    ) {
        let cache_key = format!("UPDATE_CHECK:{}:*", deployment_key);
        if let Ok(mut conn) = redis.get().await {
            use bb8_redis::redis::AsyncCommands;
            if let Ok(keys) = conn.keys::<_, Vec<String>>(&cache_key).await {
                for key in keys {
                    let _: () = conn.del(key).await.unwrap_or(());
                }
            }
        }
    }

    pub async fn update_check(
        state: &crate::core::db::AppState,
        deployment_key: &str,
        app_version: &str,
        label: &str,
        package_hash: &str,
        _client_unique_id: &str,
    ) -> Result<Option<UpdateCheckInfo>, AppError> {
        let pool = &state.db.pool;
        let redis = &state.db.redis;
        let use_cache = state.config.update_check_cache;
        if deployment_key.is_empty() || app_version.is_empty() {
            return Err(AppError::new("please input deploymentKey and appVersion"));
        }

        let cache_key = format!("UPDATE_CHECK:{}:{}:{}:{}", deployment_key, app_version, label, package_hash);
        let mut conn = redis.get().await.map_err(|e| AppError::new(&format!("Redis error: {}", e)))?;
        
        if use_cache {
            let cached_data: Option<String> = bb8_redis::redis::AsyncCommands::get(&mut *conn, &cache_key).await.unwrap_or(None);
            if let Some(data) = cached_data {
                if data == "null" {
                    return Ok(None);
                }
                if let Ok(info) = serde_json::from_str::<UpdateCheckInfo>(&data) {
                    return Ok(Some(info));
                }
            }
        }

        let dep = sqlx::query_as::<_, Deployment>(
            "SELECT * FROM deployments WHERE deployment_key = ?"
        )
        .bind(deployment_key)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::new("Not found deployment"))?;

        let version = parse_version(app_version);

        let deployments_versions = sqlx::query_as::<_, DeploymentVersion>(
            "SELECT * FROM deployments_versions WHERE deployment_id = ? AND min_version <= ? AND max_version > ? ORDER BY created_at DESC LIMIT 1"
        )
        .bind(dep.id)
        .bind(&version)
        .bind(&version)
        .fetch_optional(pool)
        .await?;

        let deployments_version = match deployments_versions {
            Some(dv) => dv,
            None => return Ok(None),
        };

        let target_package_id = deployments_version.current_package_id;
        if target_package_id <= 0 {
            return Ok(None);
        }

        let target_package = sqlx::query_as::<_, Packages>(
            "SELECT * FROM packages WHERE id = ?"
        )
        .bind(target_package_id)
        .fetch_optional(pool)
        .await?;

        let target_package = match target_package {
            Some(p) => p,
            None => return Ok(None),
        };

        let is_same_deployment = target_package.deployment_id == deployments_version.deployment_id;
        let is_different_hash = target_package.package_hash != package_hash;

        if is_same_deployment && is_different_hash {
            let mut rs = UpdateCheckInfo {
                package_id: target_package.id,
                target_binary_range: deployments_version.app_version.clone(),
                download_url: get_blob_download_url(&target_package.blob_url),
                description: target_package.description.clone(),
                is_available: target_package.is_disabled == 0,
                is_disabled: target_package.is_disabled == 1,
                is_mandatory: target_package.is_mandatory == 1,
                app_version: app_version.to_string(),
                package_hash: target_package.package_hash.clone(),
                label: target_package.label.clone(),
                package_size: target_package.size as i64,
                rollout: target_package.rollout as i64,
            };

            let mut final_description = rs.description.clone();
            let mut final_is_mandatory = rs.is_mandatory;
            let mut min_id = 0;

            if !package_hash.is_empty() {
                let current_package = sqlx::query_as::<_, Packages>(
                    "SELECT * FROM packages WHERE package_hash = ? AND deployment_id = ?"
                )
                .bind(package_hash)
                .bind(target_package.deployment_id)
                .fetch_optional(pool)
                .await?;
                
                if let Some(cp) = current_package {
                    min_id = cp.id;
                }
            } else {
                min_id = target_package.id - 1;
            }

            if target_package.id > min_id {
                let merged_cache_key = format!("MERGED_INFO:{}:{}:{}", deployment_key, package_hash, target_package.id);
                let merged_data: Option<String> = conn.get(&merged_cache_key).await.unwrap_or(None);
                
                let mut hit = false;
                if let Some(data) = merged_data {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                        if let (Some(desc), Some(mand)) = (parsed.get("description").and_then(|v| v.as_str()), parsed.get("isMandatory").and_then(|v| v.as_bool())) {
                            final_description = desc.to_string();
                            final_is_mandatory = mand;
                            hit = true;
                        }
                    }
                }

                if !hit {
                    let (desc, mand) = Self::compute_merged_info(pool, target_package.deployment_id, deployments_version.id, min_id, target_package.id).await?;
                    final_description = desc.clone();
                    final_is_mandatory = mand;

                    let cache_val = serde_json::json!({
                        "description": desc,
                        "isMandatory": mand
                    });
                    let _: () = conn.set_ex(&merged_cache_key, cache_val.to_string(), 86400).await.unwrap_or(());
                }
            }

            rs.description = final_description;
            rs.is_mandatory = final_is_mandatory;

            // diff check
            if !package_hash.is_empty() {
                let diff_package = sqlx::query_as::<_, PackagesDiff>(
                    "SELECT * FROM packages_diff WHERE package_id = ? AND diff_against_package_hash = ?"
                )
                .bind(target_package.id)
                .bind(package_hash)
                .fetch_optional(pool)
                .await?;

                if let Some(diff) = diff_package {
                    rs.download_url = get_blob_download_url(&diff.diff_blob_url);
                    rs.package_size = diff.diff_size as i64;
                }
            }

            if let Ok(json) = serde_json::to_string(&rs) {
                if use_cache {
                    let _: () = bb8_redis::redis::AsyncCommands::set_ex(&mut *conn, &cache_key, json, 600).await.unwrap_or(());
                }
            }

            return Ok(Some(rs));
        }

        if use_cache {
            let _: () = bb8_redis::redis::AsyncCommands::set_ex(&mut *conn, &cache_key, "null", 600).await.unwrap_or(());
        }
        Ok(None)
    }

    pub async fn report_status_download(
        pool: &SqlitePool,
        deployment_key: &str,
        label: &str,
        client_unique_id: &str,
    ) -> Result<(), AppError> {
        let dep = sqlx::query_as::<_, Deployment>(
            "SELECT * FROM deployments WHERE deployment_key = ?"
        )
        .bind(deployment_key)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::new("does not found deployment"))?;

        let package = sqlx::query_as::<_, Packages>(
            "SELECT * FROM packages WHERE deployment_id = ? AND label = ?"
        )
        .bind(dep.id)
        .bind(label)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::new("does not found packages"))?;

        let mut tx = pool.begin().await?;

        sqlx::query(
            "UPDATE packages_metrics SET downloaded = downloaded + 1 WHERE package_id = ?"
        )
        .bind(package.id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO log_report_download (package_id, client_unique_id, created_at) VALUES (?, ?, CURRENT_TIMESTAMP)"
        )
        .bind(package.id)
        .bind(client_unique_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn report_status_deploy(
        pool: &SqlitePool,
        deployment_key: &str,
        label: &str,
        client_unique_id: &str,
        status_opt: Option<&str>,
        previous_deployment_key: Option<&str>,
        previous_label_or_app_version: Option<&str>,
    ) -> Result<(), AppError> {
        let dep = sqlx::query_as::<_, Deployment>(
            "SELECT * FROM deployments WHERE deployment_key = ?"
        )
        .bind(deployment_key)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::new("does not found deployment"))?;

        let package = sqlx::query_as::<_, Packages>(
            "SELECT * FROM packages WHERE deployment_id = ? AND label = ?"
        )
        .bind(dep.id)
        .bind(label)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::new("does not found packages"))?;

        let status = match status_opt {
            Some("DeploymentSucceeded") => 1,
            Some("DeploymentFailed") => 2,
            _ => 0,
        };

        if status > 0 {
            let mut tx = pool.begin().await?;

            sqlx::query(
                "INSERT INTO log_report_deploy (package_id, client_unique_id, previous_label, previous_deployment_key, status, created_at) VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"
            )
            .bind(package.id)
            .bind(client_unique_id)
            .bind(previous_label_or_app_version)
            .bind(previous_deployment_key)
            .bind(status)
            .execute(&mut *tx)
            .await?;

            if status == 1 {
                sqlx::query(
                    "UPDATE packages_metrics SET installed = installed + 1, active = active + 1 WHERE package_id = ?"
                )
                .bind(package.id)
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query(
                    "UPDATE packages_metrics SET installed = installed + 1, failed = failed + 1 WHERE package_id = ?"
                )
                .bind(package.id)
                .execute(&mut *tx)
                .await?;
            }

            if let (Some(prev_dep_key), Some(prev_label)) = (previous_deployment_key, previous_label_or_app_version) {
                let prev_dep = sqlx::query_as::<_, Deployment>(
                    "SELECT * FROM deployments WHERE deployment_key = ?"
                )
                .bind(prev_dep_key)
                .fetch_optional(&mut *tx)
                .await?;

                if let Some(p_dep) = prev_dep {
                    let prev_pkg = sqlx::query_as::<_, Packages>(
                        "SELECT * FROM packages WHERE deployment_id = ? AND label = ?"
                    )
                    .bind(p_dep.id)
                    .bind(prev_label)
                    .fetch_optional(&mut *tx)
                    .await?;

                    if let Some(p_pkg) = prev_pkg {
                        sqlx::query(
                            "UPDATE packages_metrics SET active = active - 1 WHERE package_id = ?"
                        )
                        .bind(p_pkg.id)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
            }

            tx.commit().await?;
        }

        Ok(())
    }

    pub async fn chosen_man(
        _pool: &SqlitePool,
        redis: &bb8::Pool<RedisConnectionManager>,
        package_id: i64,
        rollout: Option<i64>,
        client_unique_id: &str,
    ) -> Result<bool, AppError> {
        let rollout = rollout.unwrap_or(100);
        if rollout >= 100 {
            return Ok(true);
        }

        let cache_key = format!("CHOSEN_MAN:{}:{}:{}", package_id, rollout, client_unique_id);
        let mut conn = redis.get().await.map_err(|e| AppError::new(&format!("Redis error: {}", e)))?;
        
        let data: Option<String> = conn.get(&cache_key).await.unwrap_or(None);
        if let Some(val) = data {
            if val == "1" {
                return Ok(true);
            }
            if val == "2" {
                return Ok(false);
            }
        }

        let chosen = {
            let mut rng = rand::thread_rng();
            rng.gen_range(0..10000) < rollout * 100
        };

        let val = if chosen { "1" } else { "2" };
        let _: () = conn.set_ex(&cache_key, val, 60 * 60 * 24 * 7).await.unwrap_or(());

        Ok(chosen)
    }
}
