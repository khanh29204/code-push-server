use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{delete, get},
};
use serde::{Deserialize, Serialize};

use crate::core::{app_error::AppError, db::AppState};
use crate::routes::middleware::AuthUser;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessKeyInfo {
    pub name: String,
    pub created_time: i64,
    pub created_by: String,
    pub expires: i64,
    pub description: String,
    pub friendly_name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAccessKeysResponse {
    pub access_keys: Vec<AccessKeyInfo>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccessKeyRequest {
    pub created_by: String,
    pub friendly_name: String,
    pub ttl: String,
    pub description: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAccessKeyResponse {
    pub access_key: AccessKeyInfo,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccessKeyResponse {
    pub friendly_name: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_access_keys).post(create_access_key))
        .route("/{name}", delete(delete_access_key))
}

async fn get_access_keys(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<GetAccessKeysResponse>, AppError> {
    let keys = crate::services::account_manager::AccountManager::get_all_access_key_by_uid(
        &state.db.pool,
        user.id,
    )
    .await?;
    let access_keys = keys
        .into_iter()
        .map(|k| AccessKeyInfo {
            name: k.name,
            created_time: k.created_time,
            created_by: k.created_by,
            expires: k.expires,
            description: k.description,
            friendly_name: k.friendly_name,
        })
        .collect();
    Ok(Json(GetAccessKeysResponse { access_keys }))
}

async fn create_access_key(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    crate::utils::extractors::JsonOrForm(payload): crate::utils::extractors::JsonOrForm<
        CreateAccessKeyRequest,
    >,
) -> Result<Json<CreateAccessKeyResponse>, AppError> {
    let pool = &state.db.pool;
    let manager = crate::services::account_manager::AccountManager;

    if crate::services::account_manager::AccountManager::is_exist_access_key_name(
        pool,
        user.id,
        &payload.friendly_name,
    )
    .await?
    .is_some()
    {
        return Err(AppError::General(format!(
            "The access key \"{}\" already exists.",
            payload.friendly_name
        )));
    }

    let new_access_key = format!(
        "{}{}",
        crate::utils::security::rand_token(28),
        user.identical
    );
    let ttl = payload
        .ttl
        .parse::<i64>()
        .unwrap_or(60 * 60 * 24 * 30 * 1000); // default to something if needed, wait, Node parses it, wait, let's just parse

    crate::services::account_manager::AccountManager::create_access_key(
        pool,
        user.id,
        &new_access_key,
        ttl,
        &payload.friendly_name,
        &payload.created_by,
        &payload.description,
    )
    .await?;

    Ok(Json(CreateAccessKeyResponse {
        access_key: AccessKeyInfo {
            name: new_access_key,
            created_time: chrono::Utc::now().timestamp_millis(),
            created_by: payload.created_by,
            expires: chrono::Utc::now().timestamp_millis() + ttl,
            description: payload.description,
            friendly_name: payload.friendly_name,
        },
    }))
}

async fn delete_access_key(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(name): Path<String>,
) -> Result<Json<DeleteAccessKeyResponse>, AppError> {
    sqlx::query("DELETE FROM user_tokens WHERE name = $1 AND uid = $2")
        .bind(&name)
        .bind(user.id)
        .execute(&state.db.pool)
        .await?;

    Ok(Json(DeleteAccessKeyResponse {
        friendly_name: name,
    }))
}
