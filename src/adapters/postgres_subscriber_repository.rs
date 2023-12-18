use crate::domain::new_subscriber::NewSubscriber;
use crate::domain::subscriber_repository::SubscriberRepository;
use async_trait::async_trait;
use chrono::Utc;
use rand::random;
use sqlx::PgPool;
use std::time::{SystemTime, UNIX_EPOCH};
use ulid_rs::Ulid;

#[derive(Debug, Clone)]
pub struct PostgresSubscriberRepository {
    pool: PgPool,
}

impl PostgresSubscriberRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SubscriberRepository for PostgresSubscriberRepository {
    #[tracing::instrument(name = "Inserting database record", skip(new_subscriber))]
    async fn insert_subscriber(&self, new_subscriber: NewSubscriber) -> Result<(), sqlx::Error> {
        let unique_id = Ulid::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            random,
        );

        sqlx::query!(
            r#"
            INSERT INTO subscriptions (id, email, name, subscribed_at)
            VALUES($1, $2, $3, $4)
        "#,
            unique_id.to_string(),
            new_subscriber.email.as_ref(),
            new_subscriber.name.as_ref(),
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to execute query: {:?}", e);
            e
        })?;

        Ok(())
    }
}
