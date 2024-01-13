use std::collections::HashMap;
use std::time::Duration;
use async_trait::async_trait;
use aws_config::{BehaviorVersion, Region};
use aws_config::environment::EnvironmentVariableCredentialsProvider;
use aws_lambda_events::dynamodb::{EventRecord, StreamRecord};
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::types::{AttributeDefinition, BillingMode, GlobalSecondaryIndex, KeySchemaElement, KeyType, Projection, ProjectionType};
use aws_sdk_dynamodb::types::ScalarAttributeType::S;
use opentelemetry::Context;
use opentelemetry::trace::TracerProvider;
use secrecy::Secret;
use serde_dynamo::{AttributeValue, Item};
use tracing::info;
use uuid::Uuid;
use wiremock::http::Method::Post;
use wiremock::MockServer;
use backend::adapters::dynamo_db_subscriber_repository::DynamoDbSubscriberRepository;
use backend::adapters::postmark_email_client::PostmarkEmailClient;
use backend::configuration::{DatabaseSettings, get_configuration};
use backend::domain::email_client::EmailClient;
use backend::domain::subscriber_email::SubscriberEmail;
use backend::handler::{EmailSendingError, handle_record};
use backend::telemetry::{get_subscriber, init_subscriber, init_tracer};

pub struct TestApp {
    pub email_server: MockServer,
    pub email_client: PostmarkEmailClient,
    pub repo: DynamoDbSubscriberRepository,
    pub api_client: reqwest::Client,
    pub dynamo_db_client: aws_sdk_dynamodb::Client,
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

        handle_record(&Context::new(), record, &self.email_client, &self.repo, &self.base_url)
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

    // Create and migrate the database
    let dynamo_db_client = configure_database(&configuration.database).await;

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
        dynamo_db_client: dynamo_db_client.clone(),
        table_name: configuration.database.database_name.clone(),
        base_url: configuration.base_url.clone(),
        email_client: PostmarkEmailClient::new(configuration.email_settings.base_url.clone(), SubscriberEmail::parse(configuration.email_settings.sender_email.clone()).unwrap(), Secret::new("asecretkey".to_string()), Duration::from_secs(10)),
        repo: DynamoDbSubscriberRepository::new(dynamo_db_client.clone(), configuration.database.database_name.clone())
    }
}

pub async fn configure_database(config: &DatabaseSettings) -> Client {
    let conf = aws_sdk_dynamodb::Config::builder()
        .behavior_version(BehaviorVersion::v2023_11_09())
        .credentials_provider(EnvironmentVariableCredentialsProvider::new())
        .region(Region::new("us-east-1"))
        .endpoint_url("http://localhost:8000".to_string())
        .build();

    let dynamodb_client = aws_sdk_dynamodb::Client::from_conf(conf);

    let _create_table = dynamodb_client
        .create_table()
        .table_name(&config.database_name)
        .billing_mode(BillingMode::PayPerRequest)
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("PK")
                .attribute_type(S)
                .build()
                .unwrap(),
        )
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("GSI1PK")
                .attribute_type(S)
                .build()
                .unwrap(),
        )
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("GSI1SK")
                .attribute_type(S)
                .build()
                .unwrap(),
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("PK")
                .key_type(KeyType::Hash)
                .build()
                .unwrap(),
        )
        .global_secondary_indexes(
            GlobalSecondaryIndex::builder()
                .index_name("GSI1")
                .key_schema(
                    KeySchemaElement::builder()
                        .attribute_name("GSI1PK")
                        .key_type(KeyType::Hash)
                        .build()
                        .unwrap(),
                )
                .key_schema(
                    KeySchemaElement::builder()
                        .attribute_name("GSI1SK")
                        .key_type(KeyType::Range)
                        .build()
                        .unwrap(),
                )
                .projection(
                    Projection::builder()
                        .projection_type(ProjectionType::All)
                        .build(),
                )
                .build()
                .unwrap(),
        )
        .send()
        .await
        .unwrap();

    dynamodb_client
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