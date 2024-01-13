use std::collections::HashMap;
use std::time::Duration;
use async_trait::async_trait;


use aws_lambda_events::dynamodb::{EventRecord, StreamRecord};



use opentelemetry::Context;
use opentelemetry::trace::TracerProvider;
use secrecy::Secret;
use serde_dynamo::{AttributeValue};
use tracing::info;
use uuid::Uuid;

use wiremock::MockServer;
use backend::adapters::postmark_email_client::PostmarkEmailClient;
use backend::configuration::{get_configuration};
use backend::domain::email_client::EmailClient;
use backend::domain::subscriber_email::SubscriberEmail;
use backend::handler::{EmailSendingError, handle_record};
use backend::telemetry::{get_subscriber, init_subscriber, init_tracer};

pub struct TestApp {
    pub email_server: MockServer,
    pub email_client: PostmarkEmailClient,
    pub api_client: reqwest::Client,
    pub table_name: String,
    pub base_url: String,
}

impl TestApp {
    pub async fn process_record_with_email_address(&self, email_address: &str) -> Result<(), EmailSendingError> {
        let mut hash_map: HashMap<String, AttributeValue> = HashMap::new();
        hash_map.insert("EmailAddress".to_string(), AttributeValue::S(email_address.to_string()));
        hash_map.insert("Type".to_string(), AttributeValue::S("SubscriberToken".to_string()));

        let record = EventRecord{
            aws_region: "us-east-1".to_string(),
            change: StreamRecord {
                approximate_creation_date_time: Default::default(),
                keys: Default::default(),
                new_image: hash_map.into(),
                old_image: Default::default(),
                sequence_number: None,
                size_bytes: 0,
                stream_view_type: None,
            },
            event_id: "".to_string(),
            event_name: "".to_string(),
            event_source: None,
            event_version: None,
            event_source_arn: None,
            user_identity: None,
            record_format: None,
            table_name: None,
        };

        handle_record(&Context::new(), record, &self.email_client, &self.base_url)
            .await
    }
}

pub async fn spawn_app() -> TestApp {
    let email_server = MockServer::start().await;

    // Randomise configuration to ensure test isolation
    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration.");
        // Use a different database for each test case
        c.database.database_name = Uuid::new_v4().to_string();
        c.database.auth_database_name = Uuid::new_v4().to_string();
        c.database.use_local = true;
        // Use the mock server as email API
        c.email_settings.base_url = email_server.uri();
        c.telemetry.otlp_endpoint = "jaeger".to_string();
        c.telemetry.dataset_name = "test-zero2prod".to_string();
        c
    };

    let tracer = init_tracer(&configuration.telemetry);
    let subscriber = get_subscriber(
        configuration.telemetry.dataset_name.clone(),
        "info".into(),
        std::io::stdout,
        &configuration.telemetry,
        &tracer.tracer("zero2prod-backend-local"),
    );

    init_subscriber(subscriber);

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    TestApp {
        email_server,
        api_client: client,
        table_name: configuration.database.database_name.clone(),
        base_url: configuration.base_url.clone(),
        email_client: PostmarkEmailClient::new(configuration.email_settings.base_url.clone(), SubscriberEmail::parse(configuration.email_settings.sender_email.clone()).unwrap(), Secret::new("asecretkey".to_string()), Duration::from_secs(10)),
    }
}

pub struct TestEmailClient {}

#[async_trait]
impl EmailClient for TestEmailClient {
    async fn send_email_to(
        &self,
        recipient: &SubscriberEmail,
        _subject: &str,
        _html_content: &str,
        _text_content: &str,
    ) -> Result<(), anyhow::Error> {
        info!("Sending email to {}", recipient.inner());
        Ok(())
    }
}