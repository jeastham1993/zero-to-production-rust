use crate::domain::confirmed_subscriber::ConfirmedSubscriber;
use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_repository::{DatabaseError, SubscriberRepository};
use anyhow::Result;
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
    #[tracing::instrument(skip())]
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
            Err(DatabaseError::DatabaseReadError("Error reading from database".to_string()).into())
        }
    }
}
