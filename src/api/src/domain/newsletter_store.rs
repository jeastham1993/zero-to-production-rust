use async_trait::async_trait;
use crate::domain::newsletter_metadata::NewsletterMetadata;

use crate::utils::error_chain_fmt;
#[derive(thiserror::Error)]
pub enum NewsletterStoreError {
    #[error("{0}")]
    IssueExists(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for NewsletterStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[async_trait]
pub trait NewsletterStore {
    async fn store_newsletter_metadata(&self, metadata: NewsletterMetadata) -> Result<String, NewsletterStoreError>;
}