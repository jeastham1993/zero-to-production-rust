use crate::domain::new_subscriber::{ConfirmedSubscriber, NewSubscriber};
use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_repository::{DatabaseError, SubscriberRepository};
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

                Ok(Some(first_item["SK"].as_s().unwrap().clone()))
            }
        } else {
            Err(DatabaseError::TokenNotFoundError("Token not found".to_string()).into())
        }
    }

    async fn confirm_subscriber(&self, subscriber_id: String) -> Result<(), anyhow::Error> {
        let put_res = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(subscriber_id.to_string()))
            .item("SK", AttributeValue::S(subscriber_id.to_string()))
            .item("GSI1PK", AttributeValue::S("confirmed".to_string()))
            .item("GSI1SK", AttributeValue::S(subscriber_id.to_string()))
            .send()
            .await
            .context(format!(
                "Failure inserting record to DynamoDB. Using table {}",
                &self.table_name
            ))?;

        Ok(())
    }

    async fn get_confirmed_subscribers(
        &self,
    ) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
        let query_res = self
            .client
            .query()
            .table_name(&self.table_name)
            .index_name("GSI1".to_string())
            .key_condition_expression("#gsi1pk = :gsi1pk")
            .expression_attribute_names("#gsi1pk", "GSI1PK")
            .expression_attribute_values(":gsi1pk", AttributeValue::S("confirmed".to_string()))
            .send()
            .await?;

        if let Some(items) = query_res.items {
            let subscribers = items
                .iter()
                .map(|v| {
                    Ok(ConfirmedSubscriber {
                        email: SubscriberEmail::parse(v["PK"].as_s().unwrap().clone()).unwrap(),
                    })
                })
                .collect();
            Ok(subscribers)
        } else {
            Err(DatabaseError::TokenNotFoundError("Token not found".to_string()).into())
        }
    }

    async fn apply_migrations(&self) -> Result<(), anyhow::Error> {
        todo!()
    }
}
