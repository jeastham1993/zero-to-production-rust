use crate::authentication::{compute_password_hash, UserAuthenticationError, UserRepository};
use crate::routes::error_chain_fmt;
use crate::telemetry::spawn_blocking_with_tracing;
use anyhow::{Context, Error};
use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use secrecy::{ExposeSecret, Secret};

#[derive(Debug, Clone)]
pub struct DynamoDbUserRepository {
    client: Client,
    table_name: String,
}

impl DynamoDbUserRepository {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }
}

#[async_trait]
impl UserRepository for DynamoDbUserRepository {
    #[tracing::instrument(name = "Retrieving stored credentials", skip(username))]
    async fn get_stored_credentials(
        &self,
        username: &str,
    ) -> std::result::Result<Option<(String, Secret<String>)>, UserAuthenticationError> {
        let creds_result = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("PK", AttributeValue::S(username.to_string()))
            .key("SK", AttributeValue::S("CREDENTIALS".to_string()))
            .send()
            .await
            .context("Failed to get user credentials")?;

        match creds_result.item {
            None => Err(UserAuthenticationError::UserNotFoundError(
                "User not found".to_string(),
            )),
            Some(creds) => Ok(Some((
                creds["PK"].as_s().unwrap().to_string(),
                Secret::new(creds["password_hash"].as_s().unwrap().to_string()),
            ))),
        }
    }

    #[tracing::instrument(name = "Changing password", skip(user_id, password))]
    async fn change_password(
        &self,
        user_id: &str,
        password: Secret<String>,
    ) -> std::result::Result<(), Error> {
        let password_hash = spawn_blocking_with_tracing(move || compute_password_hash(password))
            .await?
            .context("Failed to hash password")?;

        let _put_res = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(user_id.to_string()))
            .item("SK", AttributeValue::S("CREDENTIALS".to_string()))
            .item(
                "password_hash",
                AttributeValue::S(password_hash.expose_secret().to_string()),
            )
            .send()
            .await
            .context(format!(
                "Failure inserting record to DynamoDB. Using table {}",
                &self.table_name
            ))?;

        Ok(())
    }

    async fn seed(&self) -> Result<(), Error> {
        let _put_res = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S("admin".to_string()))
            .item("SK", AttributeValue::S("CREDENTIALS".to_string()))
            .item(
                "password_hash",
                AttributeValue::S("$argon2id$v=19$m=15000,t=2,p=1$94TDOx1pXBY0GzKpi774dQ$v9LyxFtOk2qYPI5AQNzut0D6H6/3bOurbCVbZqYD1aM".to_string()),
            )
            .send()
            .await
            .context(format!(
                "Failure inserting record to DynamoDB. Using table {}",
                &self.table_name
            ))?;

        Ok(())
    }
}
