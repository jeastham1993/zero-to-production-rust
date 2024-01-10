use crate::domain::new_subscriber::{ConfirmedSubscriber, NewSubscriber};
use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_repository::{DatabaseError, StoreTokenError, SubscriberRepository};

use async_trait::async_trait;
use chrono::Utc;
use rand::random;

use sqlx::{Error, PgPool};
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
    async fn insert_subscriber(
        &self,
        new_subscriber: &NewSubscriber,
    ) -> Result<String, anyhow::Error> {
        let unique_id = Ulid::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            random,
        );

        sqlx::query!(
            r#"
            INSERT INTO subscriptions (id, email, name, subscribed_at, status)
            VALUES($1, $2, $3, $4, 'pending_confirmation')
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

        Ok(unique_id.to_string())
    }

    #[tracing::instrument(
        name = "Store subscription token",
        skip(subscriber_id, subscription_token)
    )]
    async fn store_token(
        &self,
        subscriber_id: String,
        subscription_token: &str,
    ) -> Result<(), anyhow::Error> {
        sqlx::query!(
            r#"
    INSERT INTO subscription_tokens (subscription_token, subscriber_id)
    VALUES ($1, $2)
        "#,
            subscription_token,
            subscriber_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to execute query: {:?}", e);
            StoreTokenError(e)
        })?;

        Ok(())
    }

    #[tracing::instrument(name = "Get subscriber ID from token", skip(subscription_token))]
    async fn get_subscriber_id_from_token(
        &self,
        subscription_token: &str,
    ) -> Result<Option<String>, anyhow::Error> {
        let result = sqlx::query!(
            r#"SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1"#,
            subscription_token,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(result.map(|r| r.subscriber_id))
    }

    #[tracing::instrument(name = "Mark subscriber as confirmed", skip(subscriber_id))]
    async fn confirm_subscriber(&self, subscriber_id: String) -> Result<(), anyhow::Error> {
        sqlx::query!(
            r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
            subscriber_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[tracing::instrument(name = "Get confirmed subscribers", skip())]
    async fn get_confirmed_subscribers(
        &self,
    ) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
        struct Row {
            email: String,
        }

        let rows = sqlx::query_as!(
            Row,
            r#"
            SELECT email
            FROM subscriptions
            WHERE status = 'confirmed'
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let confirmed_subscribers = rows
            .into_iter()
            .map(|r| match SubscriberEmail::parse(r.email) {
                Ok(email) => Ok(ConfirmedSubscriber { email }),
                Err(error) => Err(anyhow::anyhow!(error)),
            })
            .collect();

        Ok(confirmed_subscribers)
    }

    async fn apply_migrations(&self) -> Result<(), anyhow::Error> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .expect("Failed to migrate database");

        Ok(())
    }
}
