use crate::domain::new_subscriber::{ConfirmedSubscriber, NewSubscriber};
use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_repository::{DatabaseError, SubscriberRepository};
use anyhow::{Context, Result};
use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use opentelemetry::{trace::TraceContextExt};
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Debug, Clone)]
pub struct DynamoDbSubscriberRepository {
    client: Client,
    table_name: String,
}

impl DynamoDbSubscriberRepository {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }
}

#[async_trait]
impl SubscriberRepository for DynamoDbSubscriberRepository {
    #[tracing::instrument(
    skip(new_subscriber)
    )]
    async fn insert_subscriber(
        &self,
        new_subscriber: &NewSubscriber,
    ) -> Result<String, anyhow::Error> {
        let trace_details = get_trace_and_span_id();

        let mut _put_res_builder = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(new_subscriber.email.to_string()))
            .item("Type", AttributeValue::S("Subscriber".to_string()))
            .item("EmailAddress", AttributeValue::S(new_subscriber.email.to_string()))
            .condition_expression("attribute_not_exists(PK)".to_string());

        _put_res_builder = match trace_details {
            None => _put_res_builder,
            Some((trace_id, span_id)) => {
                _put_res_builder
                    .item("TraceParent", AttributeValue::S(trace_id))
                    .item("ParentSpan", AttributeValue::S(span_id))
            }
        };

        _put_res_builder
            .send()
            .await
            .context(format!(
                "Failure inserting record to DynamoDB. Using table {}",
                &self.table_name
            ))?;

        Ok(new_subscriber.email.to_string())
    }
    #[tracing::instrument(
    skip(subscriber_id, subscription_token)
    )]
    async fn store_token(
        &self,
        subscriber_id: String,
        subscription_token: &str,
    ) -> Result<(), anyhow::Error> {
        let trace_details = get_trace_and_span_id();

        let mut _put_res_builder = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(subscription_token.to_string()))
            .item(
                "EmailAddress",
                AttributeValue::S(subscriber_id.to_string()),
            )
            .item(
                "Type",
                AttributeValue::S("SubscriberToken".to_string()),
            )
            .condition_expression("attribute_not_exists(PK)".to_string());

        _put_res_builder = match trace_details {
            None => _put_res_builder,
            Some((trace_id, span_id)) => {
                _put_res_builder
                    .item("TraceParent", AttributeValue::S(trace_id))
                    .item("ParentSpan", AttributeValue::S(span_id))
            }
        };

        _put_res_builder.send()
            .await
            .context(format!(
                "Failure inserting record to DynamoDB. Using table {}",
                &self.table_name
            ))?;

        Ok(())
    }

    #[tracing::instrument(
    skip(subscription_token)
    )]
    async fn get_subscriber_id_from_token(
        &self,
        subscription_token: &str,
    ) -> Result<Option<String>, anyhow::Error> {
        let query_res = self
            .client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("#pk = :pk")
            .expression_attribute_names("#pk", "PK")
            .expression_attribute_values(":pk", AttributeValue::S(subscription_token.to_string()))
            .send()
            .await?;

        if let Some(items) = query_res.items {
            if items.is_empty() {
                Err(DatabaseError::TokenNotFoundError("Token not found".to_string()).into())
            } else {
                let first_item = items.first().unwrap();

                Ok(Some(first_item["EmailAddress"].as_s().unwrap().clone()))
            }
        } else {
            Err(DatabaseError::TokenNotFoundError("Token not found".to_string()).into())
        }
    }

    #[tracing::instrument(
    skip(subscriber_id)
    )]
    async fn confirm_subscriber(&self, subscriber_id: String) -> Result<(), anyhow::Error> {
        let trace_details = get_trace_and_span_id();

        let mut _put_res_builder = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(subscriber_id.to_string()))
            .item("Type", AttributeValue::S("Subscriber".to_string()))
            .item("EmailAddress", AttributeValue::S(subscriber_id.to_string()))
            .item("GSI1PK", AttributeValue::S("confirmed".to_string()))
            .item("GSI1SK", AttributeValue::S(subscriber_id.to_string()));

        _put_res_builder = match trace_details {
            None => _put_res_builder,
            Some((trace_id, span_id)) => {
                _put_res_builder
                    .item("TraceParent", AttributeValue::S(trace_id))
                    .item("ParentSpan", AttributeValue::S(span_id))
            }
        };

        _put_res_builder
            .send()
            .await
            .context(format!(
                "Failure inserting record to DynamoDB. Using table {}",
                &self.table_name
            ))?;

        Ok(())
    }

    async fn apply_migrations(&self) -> Result<(), anyhow::Error> {
        todo!()
    }
}

fn get_trace_and_span_id() -> Option<(String, String)> {
    // Access the current span
    let current_span = Span::current();
    // Retrieve the context from the current span
    let context = current_span.context();
    // Use OpenTelemetry's API to retrieve the TraceContext
    let span_context = context.span().span_context().clone();

    // Check if the span context is valid
    if span_context.is_valid() {
        // Retrieve traceId and spanId
        let trace_id = span_context.trace_id().to_string().clone();
        let span_id = span_context.span_id().to_string().clone();
        Some((trace_id, span_id))
    } else {
        // No valid span context found
        None
    }
}
