use crate::domain::newsletter_metadata::NewsletterMetadata;
use crate::domain::newsletter_store::{NewsletterStore, NewsletterStoreError};
use anyhow::Context;
use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use aws_sdk_s3::operation::get_object::{GetObjectError, GetObjectOutput};
use serde::Serialize;

pub struct S3NewsletterMetadataStorage {
    s3_client: Client,
    bucket_name: String,
}

impl S3NewsletterMetadataStorage {
    pub fn new(s3_client: Client, bucket_name: String) -> Self {
        Self {
            s3_client,
            bucket_name,
        }
    }
}

#[async_trait]
impl NewsletterStore for S3NewsletterMetadataStorage {
    #[tracing::instrument(skip(self, path))]
    async fn retrieve_newsletter(
        &self,
        path: &str,
    ) -> Result<NewsletterMetadata, NewsletterStoreError> {
        let get_result = self
            .s3_client
            .get_object()
            .bucket(&self.bucket_name)
            .key(path)
            .send()
            .await;

        match get_result {
            Ok(res) => {
                let bytes = res.body.collect().await.unwrap();

                let metadata: NewsletterMetadata = wrapper(bytes.to_vec()).unwrap();

                tracing::info!("Newsletter title to work on is {}", &metadata.issue_title);

                Ok(metadata)
            }
            Err(s3_error) => {
                match s3_error.into_service_error() {
                    GetObjectError::InvalidObjectState(_) => tracing::error!("Invalid object state"),
                    GetObjectError::NoSuchKey(_) => tracing::error!("No such key"),
                    GetObjectError::Unhandled(e)  => tracing::error!("Unhandled error"),
                    _  => tracing::error!("Unknown error"),
                }

                Err(NewsletterStoreError::IssueExists("Error".to_string()))
            }
        }
    }
}

fn wrapper<T>(vec: Vec<u8>) -> Result<T, serde_json::Error>
where
    T: for<'de> serde::Deserialize<'de>,
{
    serde_json::from_slice(&vec)
}
