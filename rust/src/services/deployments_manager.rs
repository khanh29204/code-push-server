use sqlx::SqlitePool;

use crate::core::app_error::AppError;
use crate::models::deployments::Deployment;
use crate::models::packages::Packages;
use crate::models::users::User;
pub struct DeploymentsManager;

impl DeploymentsManager {
    pub async fn get_deployment_history(pool: &SqlitePool, deployments_id: i64, limit: usize) -> Result<Vec<Packages>, AppError> {
        let history = sqlx::query_as::<_, crate::models::deployments_history::DeploymentHistory>(
            "SELECT * FROM deployments_history WHERE deployment_id = ? ORDER BY id DESC LIMIT ?"
        )
        .bind(deployments_id)
        .bind(limit as i64)
        .fetch_all(pool)
        .await?;

        let mut packages = Vec::new();
        for h in history {
            if let Some(pkg) = sqlx::query_as::<_, Packages>(
                "SELECT * FROM packages WHERE id = ?"
            )
            .bind(h.package_id)
            .fetch_optional(pool)
            .await? {
                packages.push(pkg);
            }
        }
        
        Ok(packages)
    }

    pub async fn exist_deployment_name(pool: &SqlitePool, app_id: i64, name: &str) -> Result<Option<Deployment>, AppError> {
        let data = sqlx::query_as::<_, Deployment>(
            "SELECT * FROM deployments WHERE appid = ? AND name = ?"
        )
        .bind(app_id)
        .bind(name)
        .fetch_optional(pool)
        .await?;

        if data.is_some() {
            Err(AppError::new(&format!("{} name does Exist!", name)))
        } else {
            Ok(data)
        }
    }

    pub async fn add_deployment(pool: &SqlitePool, name: &str, app_id: i64, uid: i64) -> Result<Deployment, AppError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE id = ?"
        )
        .bind(uid)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::new("can't find user"))?;

        Self::exist_deployment_name(pool, app_id, name).await?;

        let identical = user.identical;
        let deployment_key = format!("{}{}", crate::utils::security::rand_token(28), identical);

        let row = sqlx::query_as::<_, Deployment>(
            "INSERT INTO deployments (appid, name, deployment_key, last_deployment_version_id, label_id, created_at, updated_at) VALUES (?, ?, ?, 0, 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) RETURNING *"
        )
        .bind(app_id)
        .bind(name)
        .bind(deployment_key)
        .fetch_one(pool)
        .await?;

        Ok(row)
    }

    pub async fn rename_deployment_by_name(pool: &SqlitePool, deployment_name: &str, app_id: i64, new_name: &str) -> Result<String, AppError> {
        Self::exist_deployment_name(pool, app_id, new_name).await?;

        let result = sqlx::query(
            "UPDATE deployments SET name = ?, updated_at = CURRENT_TIMESTAMP WHERE name = ? AND appid = ?"
        )
        .bind(new_name)
        .bind(deployment_name)
        .bind(app_id)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            Ok(new_name.to_string())
        } else {
            Err(AppError::new(&format!("does not find the deployment \"{}\"", deployment_name)))
        }
    }

    pub async fn delete_deployment_by_name(pool: &SqlitePool, deployment_name: &str, app_id: i64) -> Result<String, AppError> {
        let result = sqlx::query(
            "DELETE FROM deployments WHERE name = ? AND appid = ?"
        )
        .bind(deployment_name)
        .bind(app_id)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            Ok(deployment_name.to_string())
        } else {
            Err(AppError::new(&format!("does not find the deployment \"{}\"", deployment_name)))
        }
    }

    pub async fn delete_deployment_history(pool: &SqlitePool, deployment_id: i64) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;

        sqlx::query(
            "UPDATE deployments SET last_deployment_version_id = 0, label_id = 0 WHERE id = ?"
        )
        .bind(deployment_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "DELETE FROM deployments_history WHERE deployment_id = ?"
        )
        .bind(deployment_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "DELETE FROM deployments_versions WHERE deployment_id = ?"
        )
        .bind(deployment_id)
        .execute(&mut *tx)
        .await?;

        let packages = sqlx::query_as::<_, Packages>(
            "SELECT * FROM packages WHERE deployment_id = ?"
        )
        .bind(deployment_id)
        .fetch_all(&mut *tx)
        .await?;

        for pkg in packages {
            sqlx::query(
                "DELETE FROM packages WHERE id = ?"
            )
            .bind(pkg.id)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "DELETE FROM packages_metrics WHERE package_id = ?"
            )
            .bind(pkg.id)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "DELETE FROM packages_diff WHERE package_id = ?"
            )
            .bind(pkg.id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(())
    }
}
