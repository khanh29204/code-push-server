use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Row of the `deployments_history` table. Ported from the upstream Sequelize
/// model; the table is currently only written to via raw SQL.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeploymentHistory {
    pub id: i64,
    pub deployment_id: i64,
    pub package_id: i64,
    pub created_at: Option<NaiveDateTime>,
}
