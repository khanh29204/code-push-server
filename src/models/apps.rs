use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct App {
    pub id: i64,
    pub name: String,
    pub uid: i64,
    pub os: i64,
    pub platform: i64,
    pub is_use_diff_text: i64,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}
