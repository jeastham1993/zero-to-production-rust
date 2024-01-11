use crate::domain::subscriber_email::SubscriberEmail;

pub trait EmailClient {
    async fn send_email_to(
        &self,
        recipient: &SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error>;
}
