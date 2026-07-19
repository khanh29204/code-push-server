use crate::core::app_error::AppError;

pub struct EmailManager;

impl EmailManager {
    pub async fn send_register_code_mail(email: &str, code: &str) -> Result<(), AppError> {
        println!("TODO: EmailManager::send_register_code_mail({}, {})", email, code);
        Ok(())
    }
}
