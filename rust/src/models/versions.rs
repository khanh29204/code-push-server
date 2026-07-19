use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Versions {
    pub id: i64,
    pub r#type: i64,
    pub version: String,
}
