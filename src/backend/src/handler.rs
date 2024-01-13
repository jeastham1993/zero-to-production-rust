use anyhow::Context;
use aws_lambda_events::dynamodb::EventRecord;
use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};
use serde_dynamo::AttributeValue;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use crate::domain::email_client::EmailClient;
use crate::domain::subscriber_email::SubscriberEmail;
use crate::utils::error_chain_fmt;

#[derive(thiserror::Error)]
pub enum EmailSendingError {
    #[error("{0}")]
    ParseEmailError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for EmailSendingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[tracing::instrument(name="handle_dynamo_db_stream_record", skip(context, email_client))]
pub async fn handle_record<TEmail: EmailClient>(
    context: &opentelemetry::Context,
    record: EventRecord,
    email_client: &TEmail,
    base_url: &str,
) -> Result<(), EmailSendingError> {
    tracing::Span::current().set_parent(context.clone());

    let subscription_token = generate_subscription_token();

    let subscriber_id = get_email_adress_from(&record)?;

    send_confirmation_email(
        email_client,
        SubscriberEmail::parse(subscriber_id).unwrap(),
        &subscription_token,
        base_url,
    )
        .await
        .context("Failed to send confirmation email")?;

    Ok(())
}

#[tracing::instrument(
name = "send_confirmation_email_to_subscriber",
skip(email_client, new_subscriber, subscription_token, base_url)
)]
pub async fn send_confirmation_email(
    email_client: &dyn EmailClient,
    new_subscriber: SubscriberEmail,
    subscription_token: &str,
    base_url: &str,
) -> Result<(), anyhow::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );
    let plain_body = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    let html_body = format!(
        "Welcome to our newsletter!<br />Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );

    email_client
        .send_email_to(&new_subscriber, "Welcome!", &html_body, &plain_body)
        .await
}

fn get_email_adress_from(record: &EventRecord) -> Result<String, EmailSendingError> {
    let (_, type_value) = record
        .change
        .new_image
        .get_key_value("EmailAddress")
        .unwrap();

    let parsed_type_value = match type_value {
        AttributeValue::S(val) => val,
        _ => {
            return Err(EmailSendingError::ParseEmailError(
                "Failed to parse email address from DynamoDB item".to_lowercase(),
            ))
        }
    };

    Ok(parsed_type_value.to_string())
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}