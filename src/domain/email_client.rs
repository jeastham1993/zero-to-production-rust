use crate::domain::subscriber_email::SubscriberEmail;
use async_trait::async_trait;

#[async_trait]
pub trait EmailClient {
    async fn send_email_to(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error>;
}
