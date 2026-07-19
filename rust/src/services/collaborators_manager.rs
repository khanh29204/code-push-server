use sqlx::{Row, SqlitePool};
use crate::models::collaborators::Collaborator;
use crate::models::users::User;
use crate::core::app_error::AppError;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CollaboratorPermission {
    pub permission: String,
}

pub struct CollaboratorsManager;

impl CollaboratorsManager {
    pub async fn list_collaborators(pool: &SqlitePool, app_id: i64) -> Result<HashMap<String, CollaboratorPermission>, AppError> {
        let cols = sqlx::query_as::<_, Collaborator>(
            "SELECT * FROM collaborators WHERE appid = ?"
        )
        .bind(app_id)
        .fetch_all(pool)
        .await?;

        let mut uids = Vec::new();
        let mut col_by_uid = HashMap::new();
        
        for c in cols {
            uids.push(c.uid);
            col_by_uid.insert(c.uid, c);
        }
        
        let mut result = HashMap::new();
        if uids.is_empty() {
            return Ok(result);
        }
        
        let uid_placeholders = uids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!("SELECT * FROM users WHERE id IN ({})", uid_placeholders);
        
        let mut query = sqlx::query_as::<_, User>(&query_str);
        for uid in &uids {
            query = query.bind(uid);
        }
        let users = query.fetch_all(pool).await?;
        
        for u in users {
            let permission = col_by_uid.get(&u.id).map(|c| c.roles.clone()).unwrap_or_default();
            result.insert(u.email.clone(), CollaboratorPermission { permission });
        }
        
        Ok(result)
    }

    pub async fn add_collaborator(pool: &SqlitePool, app_id: i64, uid: i64) -> Result<(), AppError> {
        let exists = sqlx::query("SELECT 1 FROM collaborators WHERE appid = ? AND uid = ?")
            .bind(app_id)
            .bind(uid)
            .fetch_optional(pool)
            .await?;
            
        if exists.is_some() {
            return Err(AppError::General("user already is Collaborator.".to_string()));
        }
        
        sqlx::query("INSERT INTO collaborators (appid, uid, roles, created_at) VALUES (?, ?, 'Collaborator', CURRENT_TIMESTAMP)")
            .bind(app_id)
            .bind(uid)
            .execute(pool)
            .await?;
            
        Ok(())
    }
    
    pub async fn delete_collaborator(pool: &SqlitePool, app_id: i64, uid: i64) -> Result<(), AppError> {
        let data = sqlx::query_as::<_, Collaborator>("SELECT * FROM collaborators WHERE appid = ? AND uid = ?")
            .bind(app_id)
            .bind(uid)
            .fetch_optional(pool)
            .await?;
            
        if let Some(col) = data {
            sqlx::query("DELETE FROM collaborators WHERE id = ?")
                .bind(col.id)
                .execute(pool)
                .await?;
            Ok(())
        } else {
            Err(AppError::General("user is not a Collaborator".to_string()))
        }
    }
}
