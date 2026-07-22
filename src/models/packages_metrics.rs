use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PackagesMetrics {
    pub id: i64,
    pub package_id: i64,
    pub active: i64,
    pub downloaded: i64,
    pub failed: i64,
    pub installed: i64,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}
