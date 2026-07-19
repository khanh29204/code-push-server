use axum::Router;
use crate::core::db::AppState;

pub mod index;
pub mod index_v1;
pub mod middleware;
pub mod apps;
pub mod auth;
pub mod users;
pub mod access_keys;
pub mod account;

pub fn api_router() -> Router<AppState> {
    Router::new()
        .merge(index::router())
        .nest("/v0.1/public/codepush", index_v1::router())
        .nest("/apps", apps::router())
        .nest("/auth", auth::router())
        .nest("/users", users::router())
        .nest("/accessKeys", access_keys::router())
        .nest("/account", account::router())
}
