use axum::{
    extract::{FromRequest, Request},
    http::header::CONTENT_TYPE,
    Form, Json,
};
use serde::de::DeserializeOwned;
use crate::core::app_error::AppError;

pub struct JsonOrForm<T>(pub T);

impl<T, S> FromRequest<S> for JsonOrForm<T>
where
    T: DeserializeOwned + Send + 'static,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");

        if content_type.starts_with("application/x-www-form-urlencoded") {
            let Form(payload) = Form::<T>::from_request(req, state)
                .await
                .map_err(|e| AppError::General(e.to_string()))?;
            Ok(JsonOrForm(payload))
        } else {
            let Json(payload) = Json::<T>::from_request(req, state)
                .await
                .map_err(|e| AppError::General(e.to_string()))?;
            Ok(JsonOrForm(payload))
        }
    }
}
