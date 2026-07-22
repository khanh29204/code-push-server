use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};

use crate::core::app_error::AppError;
use crate::core::db::AppState;
use crate::routes::middleware::AuthUser;

#[derive(Serialize)]
pub struct AppsResponse {
    pub apps: Vec<crate::services::app_manager::AppDetail>,
}

pub async fn list_apps(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
) -> Result<Json<AppsResponse>, AppError> {
    let apps = crate::services::app_manager::AppManager::list_apps(&state.db.pool, user.id).await?;
    Ok(Json(AppsResponse { apps }))
}

pub async fn delete_app(
    AuthUser(user): AuthUser,
    Path(app_name): Path<String>,
    State(state): State<AppState>,
) -> Result<&'static str, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    crate::services::app_manager::AppManager::delete_app(&state.db.pool, app.id).await?;
    Ok("ok")
}

#[derive(Deserialize)]
pub struct RenameAppRequest {
    pub name: String,
}

pub async fn rename_app(
    AuthUser(user): AuthUser,
    Path(app_name): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<RenameAppRequest>,
) -> Result<&'static str, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    crate::services::app_manager::AppManager::modify_app(
        &state.db.pool,
        app.id,
        Some(&payload.name),
        None,
        None,
    )
    .await?;
    Ok("ok")
}

pub async fn transfer_app(
    AuthUser(user): AuthUser,
    Path((app_name, email)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<&'static str, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let target_user =
        sqlx::query_as::<_, crate::models::users::User>("SELECT * FROM users WHERE email = ?")
            .bind(&email)
            .fetch_optional(&state.db.pool)
            .await?
            .ok_or_else(|| AppError::NotFound("User not found".into()))?;
    crate::services::app_manager::AppManager::transfer_app(
        &state.db.pool,
        app.id,
        user.id,
        target_user.id,
    )
    .await?;
    Ok("ok")
}

#[derive(Deserialize)]
pub struct CreateAppRequest {
    pub name: String,
    pub os: String,
    pub platform: String,
}

#[derive(Serialize)]
pub struct CreateAppResponse {
    pub app: serde_json::Value,
}

pub async fn create_app(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateAppRequest>,
) -> Result<Json<CreateAppResponse>, AppError> {
    let os = match payload.os.as_str() {
        "iOS" => 1,
        "Android" => 2,
        "Windows" => 3,
        _ => 0,
    };
    let platform = match payload.platform.as_str() {
        "React-Native" => 1,
        "Cordova" => 2,
        _ => 0,
    };
    crate::services::app_manager::AppManager::add_app(
        &state.db.pool,
        user.id,
        &payload.name,
        os,
        platform,
        &user.identical,
    )
    .await?;
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &payload.name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", payload.name)))?;
    let val = serde_json::json!({
        "name": app.name,
        "os": payload.os,
        "platform": payload.platform,
    });
    Ok(Json(CreateAppResponse { app: val }))
}
