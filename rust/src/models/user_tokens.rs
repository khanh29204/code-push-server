use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserTokens {
    pub id: i64,
    pub uid: i64,
    pub name: String,
    pub tokens: String,
    pub description: String,
    pub is_session: Option<i8>,
    pub created_by: String,
    pub created_at: Option<NaiveDateTime>,
    pub expires_at: Option<NaiveDateTime>,
}
