use crate::domain::new_subscriber::NewSubscriber;
use async_trait::async_trait;
use sqlx::{Error, PgPool, Postgres, Transaction};

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
    ) -> Result<(), sqlx::Error>;

    async fn get_subscriber_id_from_token(
        &self,
        subscription_token: &str,
    ) -> Result<Option<String>, sqlx::Error>;

    async fn confirm_subscriber(&self, subscriber_id: String) -> Result<(), sqlx::Error>;

    async fn apply_migrations(&self) -> Result<(), sqlx::Error>;
}
