use crate::domain::subscriber_repository::{DatabaseError, SubscriberRepository};
use anyhow::{Context, Result};
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use tonic::async_trait;

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
    name = "store_token",
    skip(subscriber_id, subscription_token)
    )]
    async fn store_token(
        &self,
        subscriber_id: &str,
        subscription_token: &str,
    ) -> Result<(), anyhow::Error> {
        let _put_res = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(subscription_token.to_string()))
            .item(
                "subscriber_id",
                AttributeValue::S(subscriber_id.to_string()),
            )
            .condition_expression("attribute_not_exists(PK)".to_string())
            .send()
            .await
            .context(format!(
                "Failure inserting record to DynamoDB. Using table {}",
                &self.table_name
            ))?;

        Ok(())
    }
}
