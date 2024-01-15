use crate::domain::newsletter_metadata::NewsletterMetadata;
use crate::domain::newsletter_store::{NewsletterStore, NewsletterStoreError};
use anyhow::Context;
use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
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
        let mut get_result = self
            .s3_client
            .get_object()
            .bucket(&self.bucket_name)
            .key(path)
            .send()
            .await
            .context("Failure retrieving S3 item")?;

        let bytes = get_result.body.collect().await.unwrap();

        let metadata: NewsletterMetadata = wrapper(bytes.to_vec()).unwrap();

        Ok(metadata)
    }
}

fn wrapper<T>(vec: Vec<u8>) -> Result<T, serde_json::Error>
where
    T: for<'de> serde::Deserialize<'de>,
{
    serde_json::from_slice(&vec)
}
