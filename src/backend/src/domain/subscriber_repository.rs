use crate::domain::confirmed_subscriber::ConfirmedSubscriber;
use async_trait::async_trait;

use crate::utils::error_chain_fmt;

#[derive(thiserror::Error)]
pub enum DatabaseError {
    #[error("{0}")]
    DatabaseReadError(String),
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
    async fn get_confirmed_subscribers(
        &self,
    ) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error>;
}
