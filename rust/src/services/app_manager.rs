use sqlx::{Row, SqlitePool};
use crate::models::apps::App;
use crate::models::collaborators::Collaborator;
use crate::models::deployments::Deployment;
use crate::models::users::User;
use crate::core::app_error::AppError;
use crate::core::consts::*;
use crate::utils::security::rand_token;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppDetail {
    pub id: i64,
    pub name: String,
    pub os: String,
    pub platform: String,
    pub collaborators: HashMap<String, CollaboratorDetail>,
    pub deployments: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollaboratorDetail {
    pub permission: String,
    #[serde(rename = "isCurrentAccount")]
    pub is_current_account: bool,
}

pub struct AppManager;

impl AppManager {
    pub async fn find_app_by_name(pool: &SqlitePool, uid: i64, app_name: &str) -> Result<Option<App>, AppError> {
        let app = sqlx::query_as::<_, App>(
            "SELECT * FROM apps WHERE name = ? AND uid = ?"
        )
        .bind(app_name)
        .bind(uid)
        .fetch_optional(pool)
        .await?;
        Ok(app)
    }

    pub async fn add_app(pool: &SqlitePool, uid: i64, app_name: &str, os: i64, platform: i64, identical: &str) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;
        
        let app_res = sqlx::query(
            "INSERT INTO apps (name, uid, os, platform, created_at) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP) RETURNING id"
        )
        .bind(app_name)
        .bind(uid)
        .bind(os)
        .bind(platform)
        .fetch_one(&mut *tx)
        .await?;
        
        let app_id: i64 = app_res.get("id");
        
        let prod_key = format!("{}{}", rand_token(28), identical);
        let staging_key = format!("{}{}", rand_token(28), identical);

        sqlx::query(
            "INSERT INTO deployments (appid, name, last_deployment_version_id, label_id, deployment_key, created_at) VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP), (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"
        )
        .bind(app_id).bind(PRODUCTION).bind(0_i64).bind(0_i64).bind(&prod_key)
        .bind(app_id).bind(STAGING).bind(0_i64).bind(0_i64).bind(&staging_key)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO collaborators (appid, uid, roles, created_at) VALUES (?, ?, ?, CURRENT_TIMESTAMP)"
        )
        .bind(app_id)
        .bind(uid)
        .bind("Owner")
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_app(pool: &SqlitePool, app_id: i64) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM apps WHERE id = ?").bind(app_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM collaborators WHERE appid = ?").bind(app_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM deployments WHERE appid = ?").bind(app_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn modify_app(pool: &SqlitePool, app_id: i64, name: Option<&str>, os: Option<i64>, platform: Option<i64>) -> Result<u64, AppError> {
        let mut updates = Vec::new();
        if name.is_some() { updates.push("name = ?"); }
        if os.is_some() { updates.push("os = ?"); }
        if platform.is_some() { updates.push("platform = ?"); }
        
        if updates.is_empty() {
            return Ok(0);
        }
        
        let query_str = format!("UPDATE apps SET {} WHERE id = ?", updates.join(", "));
        let mut q = sqlx::query(&query_str);
        if let Some(v) = name { q = q.bind(v); }
        if let Some(v) = os { q = q.bind(v); }
        if let Some(v) = platform { q = q.bind(v); }
        q = q.bind(app_id);
        
        let res = q.execute(pool).await?;
        if res.rows_affected() == 0 {
            return Err(AppError::General("modify errors".to_string()));
        }
        Ok(res.rows_affected())
    }

    pub async fn transfer_app(pool: &SqlitePool, app_id: i64, from_uid: i64, to_uid: i64) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;
        
        sqlx::query("UPDATE apps SET uid = ? WHERE id = ?")
            .bind(to_uid).bind(app_id).execute(&mut *tx).await?;
            
        sqlx::query("DELETE FROM collaborators WHERE appid = ? AND uid = ?")
            .bind(app_id).bind(from_uid).execute(&mut *tx).await?;
            
        sqlx::query("DELETE FROM collaborators WHERE appid = ? AND uid = ?")
            .bind(app_id).bind(to_uid).execute(&mut *tx).await?;
            
        sqlx::query("INSERT INTO collaborators (appid, uid, roles, created_at) VALUES (?, ?, 'Owner', CURRENT_TIMESTAMP)")
            .bind(app_id).bind(to_uid).execute(&mut *tx).await?;
            
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_apps(pool: &SqlitePool, uid: i64) -> Result<Vec<AppDetail>, AppError> {
        let cols = sqlx::query_as::<_, Collaborator>("SELECT * FROM collaborators WHERE uid = ?")
            .bind(uid)
            .fetch_all(pool)
            .await?;
            
        if cols.is_empty() {
            return Ok(vec![]);
        }
        
        let app_ids: Vec<i64> = cols.into_iter().map(|c| c.appid).collect();
        let app_placeholders = app_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!("SELECT * FROM apps WHERE id IN ({})", app_placeholders);
        let mut q = sqlx::query_as::<_, App>(&query_str);
        for id in &app_ids {
            q = q.bind(id);
        }
        
        let apps = q.fetch_all(pool).await?;
        let mut result = Vec::new();
        
        for app in apps {
            let detail = Self::get_app_detail_info(pool, &app, uid).await?;
            result.push(detail);
        }
        
        Ok(result)
    }

    async fn get_app_detail_info(pool: &SqlitePool, app: &App, current_uid: i64) -> Result<AppDetail, AppError> {
        let deps = sqlx::query_as::<_, Deployment>("SELECT * FROM deployments WHERE appid = ?")
            .bind(app.id)
            .fetch_all(pool)
            .await?;
            
        let deployment_names = deps.into_iter().map(|d| d.name).collect();
        
        let cols = sqlx::query_as::<_, Collaborator>("SELECT * FROM collaborators WHERE appid = ?")
            .bind(app.id)
            .fetch_all(pool)
            .await?;
            
        let mut collaborators = HashMap::new();
        for col in cols {
            if let Some(u) = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
                .bind(col.uid)
                .fetch_optional(pool)
                .await? {
                
                collaborators.insert(u.email, CollaboratorDetail {
                    permission: col.roles,
                    is_current_account: u.id == current_uid,
                });
            }
        }
        
        let os_name = match app.os {
            IOS => IOS_NAME.to_string(),
            ANDROID => ANDROID_NAME.to_string(),
            WINDOWS => WINDOWS_NAME.to_string(),
            _ => "".to_string(),
        };
        
        let platform_name = match app.platform {
            REACT_NATIVE => REACT_NATIVE_NAME.to_string(),
            CORDOVA => CORDOVA_NAME.to_string(),
            _ => "".to_string(),
        };
        
        Ok(AppDetail {
            id: app.id,
            name: app.name.clone(),
            os: os_name,
            platform: platform_name,
            collaborators,
            deployments: deployment_names,
        })
    }
}
