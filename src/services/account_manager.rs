use crate::core::app_error::AppError;
use crate::core::db::AppState;
use crate::models::collaborators::Collaborator;
use crate::models::user_tokens::UserTokens;
use crate::models::users::User;
use crate::services::email_manager::EmailManager;
use crate::utils::security::{md5_hash, password_hash_sync, password_verify_sync, rand_token};
use chrono::{Duration, NaiveTime, Utc};
use redis::AsyncCommands;
use sqlx::SqlitePool;

const LOGIN_LIMIT_PRE: &str = "LOGIN_LIMIT_PRE_";
const REGISTER_CODE: &str = "REGISTER_CODE_";
const EXPIRED: u64 = 1200;
const EXPIRED_SPEED: i64 = 10;

#[derive(Debug, serde::Serialize)]
pub struct AccessKeyDetail {
    pub name: String,
    #[serde(rename = "createdTime")]
    pub created_time: i64,
    #[serde(rename = "createdBy")]
    pub created_by: String,
    pub expires: i64,
    #[serde(rename = "friendlyName")]
    pub friendly_name: String,
    pub description: String,
}

pub struct AccountManager;

impl AccountManager {
    /// Port of the upstream `collaboratorCan`. Every route currently needs
    /// owner rights, so only `owner_can` is wired up.
    #[allow(dead_code)]
    pub async fn collaborator_can(
        pool: &SqlitePool,
        uid: i64,
        app_name: &str,
    ) -> Result<Collaborator, AppError> {
        let col = Self::get_collaborator(pool, uid, app_name).await?;
        if let Some(c) = col {
            Ok(c)
        } else {
            Err(AppError::General(format!("App {} not exists.", app_name)))
        }
    }

    pub async fn owner_can(
        pool: &SqlitePool,
        uid: i64,
        app_name: &str,
    ) -> Result<Collaborator, AppError> {
        let col = Self::get_collaborator(pool, uid, app_name).await?;
        if let Some(c) = col {
            if c.roles != "Owner" {
                return Err(AppError::General(
                    "Permission Deny, You are not owner!".to_string(),
                ));
            }
            Ok(c)
        } else {
            Err(AppError::General(format!("App {} not exists.", app_name)))
        }
    }

    pub async fn find_user_by_email(pool: &SqlitePool, email: &str) -> Result<User, AppError> {
        let u = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(pool)
            .await?;
        if let Some(user) = u {
            Ok(user)
        } else {
            Err(AppError::General(format!("{} does not exist.", email)))
        }
    }

    pub async fn get_all_access_key_by_uid(
        pool: &SqlitePool,
        uid: i64,
    ) -> Result<Vec<AccessKeyDetail>, AppError> {
        let tokens = sqlx::query_as::<_, UserTokens>(
            "SELECT * FROM user_tokens WHERE uid = ? ORDER BY id DESC",
        )
        .bind(uid)
        .fetch_all(pool)
        .await?;

        let res = tokens
            .into_iter()
            .map(|t| AccessKeyDetail {
                name: "(hidden)".to_string(),
                created_time: t
                    .created_at
                    .unwrap_or_else(|| Utc::now().naive_utc())
                    .and_utc()
                    .timestamp_millis(),
                created_by: t.created_by,
                expires: t
                    .expires_at
                    .unwrap_or_else(|| Utc::now().naive_utc())
                    .and_utc()
                    .timestamp_millis(),
                friendly_name: t.name,
                description: t.description,
            })
            .collect();
        Ok(res)
    }

    pub async fn is_exist_access_key_name(
        pool: &SqlitePool,
        uid: i64,
        friendly_name: &str,
    ) -> Result<Option<UserTokens>, AppError> {
        let tk =
            sqlx::query_as::<_, UserTokens>("SELECT * FROM user_tokens WHERE uid = ? AND name = ?")
                .bind(uid)
                .bind(friendly_name)
                .fetch_optional(pool)
                .await?;
        Ok(tk)
    }

    pub async fn create_access_key(
        pool: &SqlitePool,
        uid: i64,
        new_access_key: &str,
        ttl: i64,
        friendly_name: &str,
        created_by: &str,
        description: &str,
    ) -> Result<(), AppError> {
        let expires_at = Utc::now().naive_utc() + Duration::milliseconds(ttl);
        let created_at = Utc::now().naive_utc();

        sqlx::query("INSERT INTO user_tokens (uid, name, tokens, description, created_by, is_session, created_at, expires_at) VALUES (?, ?, ?, ?, ?, 0, ?, ?)")
            .bind(uid)
            .bind(friendly_name)
            .bind(new_access_key)
            .bind(description)
            .bind(created_by)
            .bind(created_at)
            .bind(expires_at)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn login(state: &AppState, account: &str, password: &str) -> Result<User, AppError> {
        if account.is_empty() {
            return Err(AppError::General(
                "Please enter your email address".to_string(),
            ));
        }
        if password.is_empty() {
            return Err(AppError::General("Please enter your password".to_string()));
        }

        let q = if account.contains('@') {
            "SELECT * FROM users WHERE email = ?"
        } else {
            "SELECT * FROM users WHERE username = ?"
        };

        let user = sqlx::query_as::<_, User>(q)
            .bind(account)
            .fetch_optional(&state.db.pool)
            .await?;

        let u = if let Some(u) = user {
            u
        } else {
            return Err(AppError::General(
                "The email or password you entered is incorrect".to_string(),
            ));
        };

        let try_login_times = state.config.try_login_times;
        let mut redis = state
            .db
            .redis
            .get()
            .await
            .map_err(|_| AppError::General("Redis connection error".to_string()))?;

        if try_login_times > 0 {
            let login_key = format!("{}{}", LOGIN_LIMIT_PRE, u.id);
            let login_error_times: Option<i32> = redis.get(&login_key).await.unwrap_or(None);
            if let Some(times) = login_error_times
                && times > try_login_times
            {
                return Err(AppError::General("The number of times you entered the wrong password exceeds the limit, and the account is locked".to_string()));
            }
        }

        let is_valid = password_verify_sync(password, &u.password).unwrap_or(false);
        if !is_valid {
            if try_login_times > 0 {
                let login_key = format!("{}{}", LOGIN_LIMIT_PRE, u.id);
                let is_exists: bool = redis.exists(&login_key).await.unwrap_or(false);
                if !is_exists {
                    let now = Utc::now();
                    let end_of_day = now
                        .date_naive()
                        .and_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap())
                        .and_local_timezone(Utc)
                        .unwrap();
                    let expires = (end_of_day.timestamp() - now.timestamp()).max(0) as u64;
                    let _: () = redis.set_ex(&login_key, 1, expires).await.unwrap_or(());
                } else {
                    let _: () = redis.incr(&login_key, 1).await.unwrap_or(());
                }
            }
            return Err(AppError::General(
                "The email or password you entered is incorrect".to_string(),
            ));
        }

        Ok(u)
    }

    pub async fn send_register_code(state: &AppState, email: &str) -> Result<(), AppError> {
        if email.is_empty() {
            return Err(AppError::General(
                "Please enter your email address".to_string(),
            ));
        }

        let u = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(&state.db.pool)
            .await?;
        if u.is_some() {
            return Err(AppError::General(format!(
                "\"{}\" has already been registered, please use another email address to register",
                email
            )));
        }

        let token = rand_token(40);
        let key = md5_hash(email);

        let mut redis = state
            .db
            .redis
            .get()
            .await
            .map_err(|_| AppError::General("Redis error".to_string()))?;
        let redis_key = format!("{}{}", REGISTER_CODE, key);
        let _: () = redis
            .set_ex(&redis_key, &token, EXPIRED)
            .await
            .unwrap_or(());

        EmailManager::send_register_code_mail(&state.config, email, &token).await?;
        Ok(())
    }

    pub async fn check_register_code(
        state: &AppState,
        email: &str,
        token: &str,
    ) -> Result<(), AppError> {
        let u = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(&state.db.pool)
            .await?;
        if u.is_some() {
            return Err(AppError::General(format!(
                "\"{}\" has already been registered, please use another email address to register",
                email
            )));
        }

        let key = md5_hash(email);
        let mut redis = state
            .db
            .redis
            .get()
            .await
            .map_err(|_| AppError::General("Redis error".to_string()))?;
        let redis_key = format!("{}{}", REGISTER_CODE, key);

        let storage_token: Option<String> = redis.get(&redis_key).await.unwrap_or(None);
        if let Some(stored) = storage_token {
            if stored != token {
                let ttl: i64 = redis.ttl(&redis_key).await.unwrap_or(-1);
                if ttl > 0 {
                    let new_ttl = (ttl - EXPIRED_SPEED).max(1) as u64;
                    let _: () = redis.expire(&redis_key, new_ttl as i64).await.unwrap_or(());
                }
                return Err(AppError::General(
                    "The verification code you entered is incorrect, please re-enter it"
                        .to_string(),
                ));
            }
            Ok(())
        } else {
            Err(AppError::General(
                "The verification code has expired, please get it again".to_string(),
            ))
        }
    }

    pub async fn register(pool: &SqlitePool, email: &str, password: &str) -> Result<(), AppError> {
        let u = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(pool)
            .await?;
        if u.is_some() {
            return Err(AppError::General(format!(
                "\"{}\" has already been registered, please use another email address to register",
                email
            )));
        }

        let identical = rand_token(9);
        let hashed = password_hash_sync(password).unwrap_or_default();
        let username = "";

        sqlx::query("INSERT INTO users (email, username, password, identical, ack_code, created_at) VALUES (?, ?, ?, ?, '', CURRENT_TIMESTAMP)")
            .bind(email).bind(username).bind(&hashed).bind(&identical)
            .execute(pool).await?;

        Ok(())
    }

    pub async fn change_password(
        pool: &SqlitePool,
        uid: i64,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), AppError> {
        if new_password.len() < 6 {
            return Err(AppError::General(
                "Please enter a new password between 6 and 20 characters long".to_string(),
            ));
        }

        let u = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(uid)
            .fetch_optional(pool)
            .await?;
        if let Some(user) = u {
            if !password_verify_sync(old_password, &user.password).unwrap_or(false) {
                return Err(AppError::General(
                    "The old password you entered is incorrect, please re-enter it".to_string(),
                ));
            }

            let hashed = password_hash_sync(new_password).unwrap_or_default();
            let ack_code = rand_token(5);

            sqlx::query("UPDATE users SET password = ?, ack_code = ? WHERE id = ?")
                .bind(hashed)
                .bind(ack_code)
                .bind(uid)
                .execute(pool)
                .await?;

            Ok(())
        } else {
            Err(AppError::General("User information not found".to_string()))
        }
    }

    async fn get_collaborator(
        pool: &SqlitePool,
        uid: i64,
        app_name: &str,
    ) -> Result<Option<Collaborator>, AppError> {
        let query_str = "
            SELECT c.* 
            FROM collaborators c 
            JOIN apps a ON c.appid = a.id 
            WHERE c.uid = ? AND a.name = ?
        ";
        let col = sqlx::query_as::<_, Collaborator>(query_str)
            .bind(uid)
            .bind(app_name)
            .fetch_optional(pool)
            .await?;

        Ok(col)
    }
}
