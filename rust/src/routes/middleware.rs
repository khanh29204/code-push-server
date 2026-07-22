use crate::core::app_error::AppError;
use crate::core::db::AppState;
use crate::models::users::User;
use axum::{extract::FromRequestParts, http::request::Parts};

pub struct AuthUser(pub User);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");

        if auth_header.is_empty() {
            return Err(AppError::Unauthorized(
                "Missing Authorization header".into(),
            ));
        }

        let mut auth_type = 1;
        let mut auth_token = String::new();

        let auth_arr: Vec<&str> = auth_header.split_whitespace().collect();
        if auth_arr.len() == 2 {
            if auth_arr[0] == "Bearer" {
                auth_token = auth_arr[1].to_string();
                if auth_token.len() > 64 {
                    auth_type = 2;
                } else {
                    auth_type = 1;
                }
            } else if auth_arr[0] == "Basic" {
                auth_type = 2;
                use base64::{Engine as _, engine::general_purpose};
                let b = general_purpose::STANDARD
                    .decode(auth_arr[1])
                    .unwrap_or_default();
                let user_pass = String::from_utf8(b).unwrap_or_default();
                let user_arr: Vec<&str> = user_pass.split(':').collect();
                if user_arr.len() == 2 {
                    auth_token = user_arr[1].to_string();
                }
            }
        }

        if auth_token.is_empty() {
            return Err(AppError::Unauthorized(
                "Invalid Authorization header format".into(),
            ));
        }

        let db = &state.db.pool;

        if auth_type == 1 {
            let (identical, _token_part) = crate::utils::security::parse_token(&auth_token);

            let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE identical = $1")
                .bind(&identical)
                .fetch_optional(db)
                .await?;

            let user = user.ok_or_else(|| AppError::Unauthorized("User not found".into()))?;

            let now = chrono::Utc::now().naive_utc();

            // Note: sqlx sqlite doesn't easily map `query!` when url isn't known, so we use `query`
            let token_info = sqlx::query(
                "SELECT id FROM user_tokens WHERE tokens = $1 AND uid = $2 AND expires_at > $3",
            )
            .bind(&auth_token)
            .bind(user.id)
            .bind(now)
            .fetch_optional(db)
            .await?;

            if token_info.is_none() {
                return Err(AppError::Unauthorized("Token expired or invalid".into()));
            }

            return Ok(AuthUser(user));
        } else if auth_type == 2 {
            #[derive(serde::Deserialize)]
            struct Claims {
                uid: i64,
                hash: String,
                exp: usize,
            }

            let token_data = jsonwebtoken::decode::<Claims>(
                &auth_token,
                &jsonwebtoken::DecodingKey::from_secret(state.config.jwt_token_secret.as_bytes()),
                &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
            )
            .map_err(|_| AppError::Unauthorized("Invalid JWT token".into()))?;

            let claims = token_data.claims;
            if claims.uid <= 0 {
                return Err(AppError::Unauthorized("Invalid user ID in token".into()));
            }

            let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
                .bind(claims.uid)
                .fetch_optional(db)
                .await?;

            let user = user.ok_or_else(|| AppError::Unauthorized("User not found".into()))?;

            if claims.hash != crate::utils::security::md5_hash(&user.ack_code) {
                return Err(AppError::Unauthorized("Hash mismatch".into()));
            }

            return Ok(AuthUser(user));
        }

        Err(AppError::Unauthorized("Auth type not supported".into()))
    }
}
