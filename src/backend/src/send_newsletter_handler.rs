use crate::domain::confirmed_subscriber::ConfirmedSubscriber;
use crate::domain::email_client::EmailClient;
use crate::domain::newsletter_metadata::NewsletterMetadata;
use crate::domain::newsletter_store::NewsletterStore;
use crate::domain::subscriber_repository::SubscriberRepository;
use crate::utils::error_chain_fmt;
use anyhow::Context;
use aws_lambda_events::sqs::SqsMessage;
use opentelemetry::trace::TraceContextExt;
use serde::Deserialize;
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
    name = "handle_queued_message",
    skip(context, record, email_client, repo, newsletter_store),
    fields(dd.trace_id=tracing::field::Empty, dd.span_id=tracing::field::Empty)
)]
pub async fn handle_record<
    TEmail: EmailClient,
    TRepo: SubscriberRepository,
    TNewsletterStore: NewsletterStore,
>(
    context: &opentelemetry::Context,
    record: SqsMessage,
    email_client: &TEmail,
    repo: &TRepo,
    newsletter_store: &TNewsletterStore,
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

    let newsletter_data_path = parse_message_body(&record).unwrap();

    tracing::info!(
        "Newsletter data path is {}",
        &newsletter_data_path.s3_pointer
    );

    let newsletter_information = newsletter_store
        .retrieve_newsletter(newsletter_data_path.s3_pointer.as_str())
        .await
        .context("Failure retrieving metadata informationx")?;

    let send_res = send_emails_to_subscribers(email_client, repo, &newsletter_information).await;

    Ok(())
}

#[tracing::instrument(
name = "send_emails_to_subscribers",
skip(email_client, repo, newsletter_information)
)]
async fn send_emails_to_subscribers<TEmail: EmailClient, TRepo: SubscriberRepository>(
    email_client: &TEmail,
    repo: &TRepo,
    newsletter_information: &NewsletterMetadata,
) -> Result<(), anyhow::Error> {
    let subscribers = repo
        .get_confirmed_subscribers()
        .await
        .context("Failure retrieving confirmed subscribers")?;

    tracing::info!("There are {} confirmed subscribers", subscribers.len());

    for subscriber in subscribers {
        match subscriber {
            Ok(subscriber) => {
                tracing::info!("Sending email to {}", &subscriber.email.to_string());

                send_email(email_client, &subscriber, newsletter_information)
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

#[tracing::instrument(skip(email_client, subscriber, newsletter_information))]
async fn send_email<TEmail: EmailClient>(
    email_client: &TEmail,
    subscriber: &ConfirmedSubscriber,
    newsletter_information: &NewsletterMetadata,
) -> Result<(), anyhow::Error> {
    email_client
        .send_email_to(
            &subscriber.email,
            &newsletter_information.issue_title,
            &newsletter_information.html_content,
            &newsletter_information.text_content,
        )
        .await
        .with_context(|| format!("Failed to send newsletter issue to {}", subscriber.email))?;

    Ok(())
}

fn parse_message_body(record: &SqsMessage) -> Result<SendNewsletterMessageBody, ()> {
    let message_body: Result<SendNewsletterMessageBody, serde_json::Error> =
        serde_json::from_str(record.body.as_ref().unwrap().as_str());

    match message_body {
        Ok(body) => Ok(body),
        Err(_) => Err(()),
    }
}

#[derive(Deserialize)]
struct SendNewsletterMessageBody {
    trace_parent: String,
    parent_span: String,
    issue_title: String,
    s3_pointer: String,
}
