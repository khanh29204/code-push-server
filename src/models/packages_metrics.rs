use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Row of the `packages_metrics` table. Read back only by
/// `PackageManager::get_metrics_by_package_id`, which no route calls yet; the
/// counters themselves are bumped with raw UPDATE statements.
#[allow(dead_code)]
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
