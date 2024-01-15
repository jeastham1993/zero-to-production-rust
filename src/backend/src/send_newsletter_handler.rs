use crate::domain::email_client::EmailClient;
use crate::domain::newsletter_store::NewsletterStore;
use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_repository::SubscriberRepository;
use crate::utils::error_chain_fmt;
use anyhow::Context;
use aws_lambda_events::dynamodb::EventRecord;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde_dynamo::AttributeValue;
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

#[tracing::instrument(
    name = "handle_dynamo_db_stream_record",
    skip(context, email_client, repo, newsletter_store)
)]
pub async fn handle_record<
    TEmail: EmailClient,
    TRepo: SubscriberRepository,
    TNewsletterStore: NewsletterStore,
>(
    context: &opentelemetry::Context,
    record: EventRecord,
    email_client: &TEmail,
    repo: &TRepo,
    newsletter_store: &TNewsletterStore,
) -> Result<(), EmailSendingError> {
    tracing::Span::current().set_parent(context.clone());

    let subscribers = repo
        .get_confirmed_subscribers()
        .await
        .context("Failure retrieving confirmed subscribers")?;

    tracing::info!("There are {} confirmed subscribers", subscribers.len());

    let newsletter_data_path = parse_object_path(&record).unwrap();

    let newsletter_information = newsletter_store
        .retrieve_newsletter(newsletter_data_path.as_str())
        .await
        .context("Failure retrieving metadata")?;

    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                email_client
                    .send_email_to(
                        &subscriber.email,
                        &newsletter_information.issue_title,
                        &newsletter_information.html_content,
                        &newsletter_information.text_content,
                    )
                    .await
                    .with_context(|| {
                        format!("Failed to send newsletter issue to {}", subscriber.email)
                    })?;
            }
            Err(error) => {
                tracing::warn!(
                    error.cause_chain = ?error,
                    error.message = %error,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid",
                );
            }
        }
    }

    Ok(())
}

fn parse_object_path(record: &EventRecord) -> Result<String, ()> {
    let (_, type_value) = record.change.new_image.get_key_value("S3Pointer").unwrap();

    match type_value {
        AttributeValue::S(val) => Ok(val.clone()),
        _ => return Err(()),
    }
}
