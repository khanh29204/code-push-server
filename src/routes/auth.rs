use axum::{
    Json, Router,
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::core::db::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/password", get(password_page))
        .route("/login", get(login_page).post(login))
        .route("/link", get(link_redirect))
        .route("/register", get(register_page))
        .route("/confirm", get(confirm_page))
        .route("/logout", post(logout))
}

#[derive(Deserialize)]
pub struct EmailQuery {
    pub email: Option<String>,
}

async fn password_page() -> Html<String> {
    Html(crate::utils::common::read_public_html("password.html"))
}

async fn login_page(Query(_query): Query<EmailQuery>) -> Html<String> {
    Html(crate::utils::common::read_public_html("login.html"))
}

async fn link_redirect() -> Redirect {
    Redirect::to("/auth/login")
}

async fn register_page(
    State(state): State<AppState>,
    Query(_query): Query<EmailQuery>,
) -> axum::response::Response {
    if state.config.allow_registration {
        Html(crate::utils::common::read_public_html("register.html")).into_response()
    } else {
        Redirect::to("/auth/login").into_response()
    }
}

async fn confirm_page(Query(_query): Query<EmailQuery>) -> Html<String> {
    Html(crate::utils::common::read_public_html("confirm.html"))
}

async fn logout() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub account: Option<String>,
    pub password: Option<String>,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub status: String,
    pub results: Option<TokensResult>,
    pub message: Option<String>,
}

#[derive(Serialize)]
pub struct TokensResult {
    pub tokens: String,
}

async fn login(
    State(state): State<AppState>,
    crate::utils::extractors::JsonOrForm(payload): crate::utils::extractors::JsonOrForm<
        LoginRequest,
    >,
) -> Result<Json<LoginResponse>, crate::core::app_error::AppError> {
    let account = payload.account.unwrap_or_default();
    let password = payload.password.unwrap_or_default();

    let user = crate::services::account_manager::AccountManager::login(&state, &account, &password)
        .await?;

    #[derive(serde::Serialize)]
    struct Claims {
        uid: i64,
        hash: String,
        exp: usize,
    }

    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        uid: user.id,
        hash: crate::utils::security::md5_hash(&user.ack_code),
        exp: now + 7200,
    };

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.config.jwt_token_secret.as_bytes()),
    )
    .map_err(|_| crate::core::app_error::AppError::General("Token creation error".into()))?;

    Ok(Json(LoginResponse {
        status: "OK".to_string(),
        results: Some(TokensResult { tokens: token }),
        message: None,
    }))
}
