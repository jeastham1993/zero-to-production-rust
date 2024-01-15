use anyhow::Context;
use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_s3::Client;
use aws_sdk_s3::primitives::ByteStream;
use serde::Serialize;
use crate::domain::{NewsletterMetadata, NewsletterStore, NewsletterStoreError};
use crate::telemetry::get_trace_and_span_id;

pub struct S3NewsletterMetadataStorage {
    s3_client: Client,
    dynamo_db_client: aws_sdk_dynamodb::Client,
    bucket_name: String,
    table_name: String
}

impl S3NewsletterMetadataStorage {
    pub fn new(s3_client: Client, dynamo_db_client: aws_sdk_dynamodb::Client, bucket_name: String, table_name: String) -> Self {
        Self{
            s3_client,
            bucket_name,
            dynamo_db_client,
            table_name
        }
    }
}

#[async_trait]
impl NewsletterStore for S3NewsletterMetadataStorage {
    #[tracing::instrument(
    name = "store_newsletter_metadata_in_s3",
    skip(self, metadata)
    )]
    async fn store_newsletter_metadata(&self, metadata: NewsletterMetadata) -> Result<String, NewsletterStoreError> {
        let json_bytes = json_bytes(&metadata);

        let body = ByteStream::from(json_bytes);

        let object_key = format!("{}.json", &metadata.issue_title);

        let put_object_result = self.s3_client
            .put_object()
            .bucket(&self.bucket_name)
            .key(&object_key)
            .body(body)
            .send()
            .await;

        match put_object_result {
            Ok(_ok_res) => {
                let _ = self.store_issue_in_dynamo(&metadata.issue_title, &object_key).await;

                Ok(object_key)
            },
            Err(e) => Err(NewsletterStoreError::UnexpectedError(e.into()))
        }
    }
}

impl S3NewsletterMetadataStorage {
    #[tracing::instrument(
    skip(self),
    fields(issue_title=%*issue_title)
    )]
    async fn store_issue_in_dynamo(&self, issue_title: &str, s3_uri: &str) -> Result<(), anyhow::Error> {
        let trace_details = get_trace_and_span_id();

        let mut _put_res_builder = self
            .dynamo_db_client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(issue_title.to_string()))
            .item(
                "IssueTitle",
                AttributeValue::S(issue_title.to_string()),
            )
            .item(
                "S3Pointer",
                AttributeValue::S(s3_uri.to_string()),
            )
            .item(
                "Type",
                AttributeValue::S("NewsletterIssue".to_string()),
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
}

pub fn json_bytes<T>(structure: T) -> Vec<u8> where T: Serialize {
    let mut bytes: Vec<u8> = Vec::new();
    serde_json::to_writer(&mut bytes, &structure).unwrap();
    bytes
}