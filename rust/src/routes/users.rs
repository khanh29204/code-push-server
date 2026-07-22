use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};

use crate::core::{app_error::AppError, db::AppState};
use crate::routes::middleware::AuthUser;

#[derive(Serialize)]
pub struct OkResponse {
    pub status: String,
}

impl Default for OkResponse {
    fn default() -> Self {
        Self {
            status: "OK".to_string(),
        }
    }
}

// GET / -> Requires Auth
#[derive(Serialize)]
pub struct IndexResponse {
    pub title: String,
}

async fn get_index(_user: AuthUser) -> Result<Json<IndexResponse>, AppError> {
    Ok(Json(IndexResponse {
        title: "CodePushServer".to_string(),
    }))
}

// POST /
#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub token: String,
    pub password: String,
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<OkResponse>, AppError> {
    if !state.config.allow_registration {
        return Err(AppError::new("Registration is not allowed"));
    }

    let email = payload.email.trim();
    let token = payload.token.trim();
    let password = payload.password.trim();

    if password.len() < 6 {
        return Err(AppError::new(
            "Please enter a password between 6 and 20 characters long",
        ));
    }

    crate::services::account_manager::AccountManager::check_register_code(&state, email, token)
        .await?;
    crate::services::account_manager::AccountManager::register(&state.db.pool, email, password)
        .await?;

    Ok(Json(OkResponse::default()))
}

// GET /exists
#[derive(Deserialize)]
pub struct ExistsQuery {
    pub email: Option<String>,
}

#[derive(Serialize)]
pub struct ExistsResponse {
    pub status: String,
    pub exists: bool,
}

async fn get_exists(
    State(state): State<AppState>,
    Query(query): Query<ExistsQuery>,
) -> Result<Json<ExistsResponse>, AppError> {
    let email = match query.email {
        Some(e) => e.trim().to_string(),
        None => return Err(AppError::new("Please enter your email address")),
    };

    if email.is_empty() {
        return Err(AppError::new("Please enter your email address"));
    }

    let exists = crate::services::account_manager::AccountManager::find_user_by_email(
        &state.db.pool,
        &email,
    )
    .await
    .is_ok();

    Ok(Json(ExistsResponse {
        status: "OK".to_string(),
        exists,
    }))
}

// POST /registerCode
#[derive(Deserialize)]
pub struct RegisterCodeRequest {
    pub email: String,
}

#[axum::debug_handler]
async fn send_register_code(
    State(state): State<AppState>,
    Json(payload): Json<RegisterCodeRequest>,
) -> Result<Json<OkResponse>, AppError> {
    let email = payload.email.trim();

    crate::services::account_manager::AccountManager::send_register_code(&state, email).await?;

    Ok(Json(OkResponse::default()))
}

// GET /registerCode/exists
#[derive(Deserialize)]
pub struct RegisterCodeExistsQuery {
    pub email: String,
    pub token: String,
}

async fn check_register_code(
    State(state): State<AppState>,
    Query(query): Query<RegisterCodeExistsQuery>,
) -> Result<Json<OkResponse>, AppError> {
    let email = query.email.trim();
    let token = query.token.trim();

    crate::services::account_manager::AccountManager::check_register_code(&state, email, token)
        .await?;

    Ok(Json(OkResponse::default()))
}

// PATCH /password
#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    #[serde(rename = "oldPassword")]
    pub old_password: String,
    #[serde(rename = "newPassword")]
    pub new_password: String,
}

async fn change_password(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<Json<OkResponse>, AppError> {
    let old_password = payload.old_password.trim();
    let new_password = payload.new_password.trim();

    crate::services::account_manager::AccountManager::change_password(
        &state.db.pool,
        user.id,
        old_password,
        new_password,
    )
    .await?;

    Ok(Json(OkResponse::default()))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_index).post(register))
        .route("/exists", get(get_exists))
        .route("/registerCode", post(send_register_code))
        .route("/registerCode/exists", get(check_register_code))
        .route("/password", patch(change_password))
}
