use tonic::async_trait;
use crate::domain::subscriber_email::SubscriberEmail;

#[async_trait]
pub trait EmailClient {
    async fn send_email_to(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), anyhow::Error>;
}
