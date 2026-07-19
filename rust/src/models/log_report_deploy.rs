use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LogReportDeploy {
    pub id: i64,
    pub status: i64,
    pub package_id: i64,
    pub client_unique_id: String,
    pub previous_label: String,
    pub previous_deployment_key: String,
    pub created_at: Option<NaiveDateTime>,
}

