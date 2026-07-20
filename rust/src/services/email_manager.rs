use crate::core::app_error::AppError;
use crate::config::AppConfig;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    transport::smtp::authentication::Credentials,
};

pub struct EmailManager;

impl EmailManager {
    pub async fn send_register_code_mail(config: &AppConfig, email: &str, code: &str) -> Result<(), AppError> {
        let email = Message::builder()
            .from(format!("CodePush Server <{}>", config.smtp.username).parse().unwrap())
            .to(email.parse().unwrap())
            .subject("CodePush Server Registration Code")
            .body(format!("Your registration code is: {}", code))
            .map_err(|e| AppError::new(&format!("Failed to build email: {}", e)))?;

        let creds = Credentials::new(config.smtp.username.clone(), config.smtp.password.clone());

        // We use Rustls for TLS. If secure is true, we should use port 465 (SMTPS) or STARTTLS on 587.
        // For simplicity, we just use relay with credentials.
        let mailer: AsyncSmtpTransport<Tokio1Executor> = if config.smtp.secure {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp.host)
                .map_err(|e| AppError::new(&format!("SMTP relay error: {}", e)))?
                .port(config.smtp.port)
                .credentials(creds)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.smtp.host)
                .port(config.smtp.port)
                .credentials(creds)
                .build()
        };

        mailer.send(email).await.map_err(|e| AppError::new(&format!("Failed to send email: {}", e)))?;

        Ok(())
    }
}
