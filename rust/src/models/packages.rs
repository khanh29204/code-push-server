use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Packages {
    pub id: i64,
    pub deployment_version_id: i64,
    pub deployment_id: i64,
    pub description: String,
    pub package_hash: String,
    pub blob_url: String,
    pub size: i64,
    pub manifest_blob_url: String,
    pub release_method: String,
    pub label: String,
    pub original_label: String,
    pub original_deployment: String,
    pub released_by: i64,
    pub is_mandatory: i64,
    pub is_disabled: i64,
    pub rollout: i64,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}
