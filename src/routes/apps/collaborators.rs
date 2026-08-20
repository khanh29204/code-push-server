use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;

use crate::core::app_error::AppError;
use crate::core::db::AppState;
use crate::routes::middleware::AuthUser;

#[derive(Serialize)]
pub struct CollaboratorsResponse {
    pub collaborators: serde_json::Value,
}

pub async fn get_collaborators(
    AuthUser(user): AuthUser,
    Path(app_name): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<CollaboratorsResponse>, AppError> {
    let app = crate::services::app_manager::AppManager::find_app_by_name(
        &state.db.pool,
        user.id,
        &app_name,
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("App {} not found", app_name)))?;
    let cols = crate::services::collaborators_manager::CollaboratorsManager::list_collaborators(
        &state.db.pool,
        app.id,
    )
    .await?;
    Ok(Json(CollaboratorsResponse {
        collaborators: serde_json::to_value(cols).unwrap_or(serde_json::Value::Null),
    }))
}

pub async fn add_collaborator(
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
    crate::services::collaborators_manager::CollaboratorsManager::add_collaborator(
        &state.db.pool,
        app.id,
        target_user.id,
    )
    .await?;
    Ok("ok")
}

pub async fn remove_collaborator(
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
    crate::services::collaborators_manager::CollaboratorsManager::delete_collaborator(
        &state.db.pool,
        app.id,
        target_user.id,
    )
    .await?;
    Ok("ok")
}
