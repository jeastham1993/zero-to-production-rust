use crate::domain::new_subscriber::NewSubscriber;
use async_trait::async_trait;

#[async_trait]
pub trait SubscriberRepository {
    async fn insert_subscriber(&self, new_subscriber: NewSubscriber) -> Result<(), sqlx::Error>;
}
