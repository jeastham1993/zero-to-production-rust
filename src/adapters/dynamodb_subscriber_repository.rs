use crate::domain::new_subscriber::{ConfirmedSubscriber, NewSubscriber};
use crate::domain::subscriber_repository::{StoreTokenError, SubscriberRepository};
use anyhow::{Context, Result};
use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;

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
    async fn insert_subscriber(
        &self,
        new_subscriber: &NewSubscriber,
    ) -> Result<String, anyhow::Error> {
        dbg!(&self.table_name);

        let put_res = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(new_subscriber.email.to_string()))
            .item("SK", AttributeValue::S(new_subscriber.email.to_string()))
            .condition_expression("attribute_not_exists(PK)".to_string())
            .send()
            .await
            .context(format!(
                "Failure inserting record to DynamoDB. Using table {}",
                &self.table_name
            ))?;

        Ok(new_subscriber.email.to_string())
    }

    async fn store_token(
        &self,
        subscriber_id: String,
        subscription_token: &str,
    ) -> Result<(), anyhow::Error> {
        let put_res = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(subscription_token.to_string()))
            .item("SK", AttributeValue::S(subscriber_id.to_string()))
            .condition_expression("attribute_not_exists(PK)".to_string())
            .send()
            .await
            .context(format!(
                "Failure inserting record to DynamoDB. Using table {}",
                &self.table_name
            ))?;

        Ok(())
    }

    async fn get_subscriber_id_from_token(
        &self,
        subscription_token: &str,
    ) -> Result<Option<String>, anyhow::Error> {
        todo!()
    }

    async fn confirm_subscriber(&self, subscriber_id: String) -> Result<(), anyhow::Error> {
        todo!()
    }

    async fn get_confirmed_subscribers(
        &self,
    ) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
        todo!()
    }

    async fn apply_migrations(&self) -> Result<(), anyhow::Error> {
        todo!()
    }
}
