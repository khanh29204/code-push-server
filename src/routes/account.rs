use axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};

use crate::core::{app_error::AppError, db::AppState};
use crate::routes::middleware::AuthUser;

#[derive(Serialize, Deserialize)]
pub struct AccountResponse {
    pub account: AccountInfo,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub email: String,
    pub linked_providers: Vec<String>,
    pub name: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_account))
}

async fn get_account(AuthUser(user): AuthUser) -> Result<Json<AccountResponse>, AppError> {
    let account = AccountInfo {
        email: user.email,
        linked_providers: vec![],
        name: user.username,
    };

    Ok(Json(AccountResponse { account }))
}
