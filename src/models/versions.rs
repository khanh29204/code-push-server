use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Row of the `versions` table. Ported from the upstream Sequelize model; the
/// table is not read by any route yet.
#[allow(dead_code)]
/// Row of the `versions` table. The table is currently only written to via raw
/// SQL, so this struct exists for parity with the upstream model and for future
/// read paths.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Versions {
    pub id: i64,
    pub r#type: i64,
    pub version: String,
}
