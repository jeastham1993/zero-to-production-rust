use tonic::async_trait;
use crate::utils::error_chain_fmt;

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
    async fn store_token(
        &self,
        subscriber_id: &str,
        subscription_token: &str,
    ) -> Result<(), anyhow::Error>;
}
