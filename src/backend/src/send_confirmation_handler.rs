use crate::domain::email_client::EmailClient;
use crate::domain::subscriber_email::SubscriberEmail;
use anyhow::Context;
use aws_lambda_events::sqs::{SqsBatchResponse, SqsEvent, SqsEventObj, SqsMessage};
use lambda_runtime::LambdaEvent;
use opentelemetry::trace::TraceContextExt;
use rand::distributions::Alphanumeric;
use rand::{Rng, thread_rng};
use serde::{Deserialize, Serialize};
use serde_dynamo::AttributeValue;
use serde_json::Error;
use tokio::sync::mpsc::UnboundedSender;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use opentelemetry_sdk::trace::{config, Config, TracerProvider};
use crate::configuration::Settings;
use crate::telemetry::parse_context_from;
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

/// Implements the main event handler for processing events from an SQS queue.
pub struct SendConfirmationEventHandler {
    request_done_sender: UnboundedSender<()>,
}

impl SendConfirmationEventHandler {
    pub fn new(request_done_sender: UnboundedSender<()>) -> Self {
        Self { request_done_sender }
    }

    pub async fn invoke<TEmail: EmailClient>(
        &self,
        event: LambdaEvent<SqsEvent>,
        configuration: &Settings,
        email_client: &TEmail,
    ) -> Result<SqsBatchResponse, Error> {
        for record in event.payload.records {
            let ctx = match parse_context_from(&record).await {
                Ok(res) => res,
                Err(_) => continue,
            };

            match self.handle(&ctx, record, email_client, &configuration.base_url).await {
                Ok(_) => {}
                Err(e) => {
                    let error_msg = format!("Failure handling SQS record. Error: {}", e);

                    lambda_extension::tracing::error!(error_msg);
                }
            };
        }
        
        let _ = self.request_done_sender.send(()).map_err(Box::new);

        Ok(SqsBatchResponse::default())
    }
    
    

    pub async fn handle<TEmail: EmailClient>(
        &self,
        context: &opentelemetry::Context,
        record: SqsMessage,
        email_client: &TEmail,
        base_url: &str,
    ) -> Result<(), EmailSendingError> {
        tracing::Span::current().set_parent(context.clone());

        let context = tracing::Span::current().context();
        let span_context = context.span().span_context().clone();

        let trace_id = span_context.trace_id().to_string().clone();
        let span_id = span_context.span_id().to_string().clone();

        let dd_trace_id = u64::from_str_radix(&trace_id[16..], 16)
            .expect("Failed to convert string_trace_id to a u64.")
            .to_string();

        let dd_span_id = u64::from_str_radix(&span_id, 16)
            .expect("Failed to convert string_span_id to a u64.")
            .to_string();

        tracing::Span::current().record("dd.trace_id", dd_trace_id);
        tracing::Span::current().record("dd.span_id", dd_span_id);

        let body = parse_message_body(&record).expect("Failure parsing message");

        send_confirmation_email(
            email_client,
            SubscriberEmail::parse(body.email_address).unwrap(),
            &body.subscriber_token,
            base_url,
        )
            .await
            .context("Failed to send confirmation email")?;

        // Notify the extension to flush traces.
        let _ = self.request_done_sender.send(()).map_err(Box::new);

        Ok(())
    }
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

#[derive(Serialize, Deserialize)]
pub struct SendConfirmationMessageBody {
    trace_parent: String,
    parent_span: String,
    email_address: String,
    subscriber_token: String,
}
