use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LogReportDownload {
    pub id: i64,
    pub package_id: i64,
    pub client_unique_id: String,
    pub created_at: Option<NaiveDateTime>,
}
