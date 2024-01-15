use crate::domain::new_subscriber::{ConfirmedSubscriber, NewSubscriber};

use async_trait::async_trait;

use crate::routes::error_chain_fmt;


#[derive(thiserror::Error)]
pub enum DatabaseError {
    #[error("{0}")]
    UserExists(String),
    #[error("{0}")]
    TokenNotFoundError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[async_trait]
pub trait SubscriberRepository {
    async fn insert_subscriber(
        &self,
        new_subscriber: &NewSubscriber,
    ) -> Result<String, anyhow::Error>;

    async fn store_token(
        &self,
        subscriber_id: String,
        subscription_token: &str,
    ) -> Result<(), anyhow::Error>;

    async fn get_subscriber_id_from_token(
        &self,
        subscription_token: &str,
    ) -> Result<Option<String>, anyhow::Error>;

    async fn confirm_subscriber(&self, subscriber_id: String) -> Result<(), anyhow::Error>;

    async fn apply_migrations(&self) -> Result<(), anyhow::Error>;
}
