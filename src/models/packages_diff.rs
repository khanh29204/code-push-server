use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PackagesDiff {
    pub id: i64,
    pub package_id: i64,
    pub diff_against_package_hash: String,
    pub diff_blob_url: String,
    pub diff_size: i64,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}
