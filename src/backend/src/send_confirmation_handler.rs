use crate::domain::email_client::EmailClient;
use crate::domain::subscriber_email::SubscriberEmail;
use crate::utils::error_chain_fmt;
use anyhow::Context;
use aws_lambda_events::dynamodb::EventRecord;
use aws_lambda_events::sqs::{SqsEvent, SqsMessage};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::Deserialize;
use serde_dynamo::AttributeValue;
use serde_json::Error;
use tracing_opentelemetry::OpenTelemetrySpanExt;

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

#[tracing::instrument(name = "handle_queued_message", skip(context, email_client))]
pub async fn handle_record<TEmail: EmailClient>(
    context: &opentelemetry::Context,
    record: SqsMessage,
    email_client: &TEmail,
    base_url: &str,
) -> Result<(), EmailSendingError> {
    tracing::Span::current().set_parent(context.clone());

    let subscription_token = generate_subscription_token();

    let body = parse_message_body(&record).expect("Failure parsing message");

    send_confirmation_email(
        email_client,
        SubscriberEmail::parse(body.email_address).unwrap(),
        &body.subscriber_token,
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

fn parse_message_body(record: &SqsMessage) -> Result<SendConfirmationMessageBody, ()> {
    let message_body: Result<SendConfirmationMessageBody, serde_json::Error> =
        serde_json::from_str(record.body.as_ref().unwrap().as_str());

    match message_body {
        Ok(body) => Ok(body),
        Err(_) => Err(()),
    }
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[derive(Deserialize)]
struct SendConfirmationMessageBody {
    trace_parent: String,
    parent_span: String,
    email_address: String,
    subscriber_token: String,
}
