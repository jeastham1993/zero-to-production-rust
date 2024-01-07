use crate::domain::new_subscriber::NewSubscriber;
use actix_web::ResponseError;
use async_trait::async_trait;
use std::fmt::Formatter;

#[derive(Debug)]
pub struct StoreTokenError(pub(crate) sqlx::Error);

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error encountered while trying to store a subscription token"
        )
    }
}

#[async_trait]
pub trait SubscriberRepository {
    async fn insert_subscriber(
        &self,
        new_subscriber: &NewSubscriber,
    ) -> Result<String, sqlx::Error>;

    async fn store_token(
        &self,
        subscriber_id: String,
        subscription_token: &str,
    ) -> Result<(), StoreTokenError>;

    async fn get_subscriber_id_from_token(
        &self,
        subscription_token: &str,
    ) -> Result<Option<String>, sqlx::Error>;

    async fn confirm_subscriber(&self, subscriber_id: String) -> Result<(), sqlx::Error>;

    async fn apply_migrations(&self) -> Result<(), sqlx::Error>;
}
