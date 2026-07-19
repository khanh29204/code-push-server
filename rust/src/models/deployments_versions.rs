use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeploymentVersion {
    pub id: i64,
    pub deployment_id: i64,
    pub app_version: String,
    pub current_package_id: i64,
    pub min_version: i64,
    pub max_version: i64,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}
