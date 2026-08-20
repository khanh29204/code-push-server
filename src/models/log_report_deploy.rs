use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Row of the `log_report_deploy` table. Ported from the upstream Sequelize
/// model; the table is currently only written to via raw SQL.
#[allow(dead_code)]
/// Row of the `log_report_deploy` table. The table is currently only written
/// to via raw SQL, so this struct exists for parity with the upstream model and
/// for future read paths.
#[allow(dead_code)]
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
