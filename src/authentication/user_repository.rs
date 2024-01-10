use crate::routes::error_chain_fmt;
use async_trait::async_trait;
use secrecy::Secret;

#[derive(thiserror::Error)]
pub enum UserAuthenticationError {
    #[error("{0}")]
    UserNotFoundError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for UserAuthenticationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[async_trait]
pub trait UserRepository {
    async fn get_stored_credentials(
        &self,
        username: &str,
    ) -> Result<Option<(String, Secret<String>)>, UserAuthenticationError>;

    async fn change_password(
        &self,
        user_id: &str,
        password: Secret<String>,
    ) -> Result<(), anyhow::Error>;

    async fn seed(&self) -> Result<(), anyhow::Error>;
}
