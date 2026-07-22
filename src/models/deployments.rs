use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Deployment {
    pub id: i64,
    pub appid: i64,
    pub name: String,
    pub description: String,
    pub deployment_key: String,
    pub last_deployment_version_id: i64,
    pub label_id: i64,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}
