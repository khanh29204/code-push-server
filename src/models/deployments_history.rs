use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeploymentHistory {
    pub id: i64,
    pub deployment_id: i64,
    pub package_id: i64,
    pub created_at: Option<NaiveDateTime>,
}
