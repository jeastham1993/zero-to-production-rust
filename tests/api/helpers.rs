use async_trait::async_trait;
use once_cell::sync::Lazy;
use opentelemetry::sdk::trace::TracerProvider;
use opentelemetry::trace::noop::NoopSpanExporter;
use secrecy::ExposeSecret;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use tracing::log::info;
use uuid::Uuid;
use wiremock::MockServer;
use zero2prod::configuration::{get_configuration, DatabaseSettings};
use zero2prod::domain::email_client::EmailClient;
use zero2prod::domain::subscriber_email::SubscriberEmail;
use zero2prod::startup::{get_connection_pool, Application};
use zero2prod::telemetry::{get_subscriber, init_subscriber};

static TRACING: Lazy<()> = Lazy::new(|| {
    let mut configuration = get_configuration().expect("Failed to read configuration");
    let default_filter = "info".to_string();
    let subscriber_name = "test".to_string();
    configuration.telemetry.dataset_name = format!("test-{}", configuration.telemetry.dataset_name);

    let default_trace_provider = TracerProvider::builder()
        .with_simple_exporter(NoopSpanExporter::default())
        .build();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(
            subscriber_name,
            default_filter,
            std::io::stdout,
            &configuration.telemetry,
            &default_trace_provider,
        );
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(
            subscriber_name,
            default_filter,
            std::io::sink,
            &configuration.telemetry,
            &default_trace_provider,
        );
        init_subscriber(subscriber);
    }
});

/// Confirmation links embedded in the request to the email API.
pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain_text: reqwest::Url,
}

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub db_pool: PgPool,
    pub email_server: MockServer,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    /// Extract the confirmation links embedded in the request to the email API.
    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        // Extract the link from one of the request fields.
        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            // Let's make sure we don't call random APIs on the web
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html = get_link(body["HtmlBody"].as_str().unwrap());
        let plain_text = get_link(body["TextBody"].as_str().unwrap());
        ConfirmationLinks { html, plain_text }
    }
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    // Launch a mock server to stand in for Postmark's API
    let email_server = MockServer::start().await;

    // Randomise configuration to ensure test isolation
    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration.");
        // Use a different database for each test case
        c.database.database_name = Uuid::new_v4().to_string();
        // Use a random OS port
        c.application.application_port = 0;
        // Use the mock server as email API
        c.email_settings.base_url = email_server.uri();
        c
    };

    // Create and migrate the database
    configure_database(&configuration.database).await;

    // Launch the application as a background task
    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application.");
    let application_port = application.port();
    let _ = tokio::spawn(application.run_until_stopped());

    let test_app = TestApp {
        address: format!("http://localhost:{}", application_port),
        db_pool: get_connection_pool(&configuration.database),
        email_server,
        port: application_port,
    };

    test_app
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let connection_string = config.connection_string_without_db();

    let mut connection = PgConnection::connect(connection_string.expose_secret())
        .await
        .expect("Failed to connect to postgres");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database");

    let connection_pool = PgPool::connect(&config.connection_string().expose_secret())
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database");

    connection_pool
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
    ) -> Result<(), reqwest::Error> {
        info!("Sending email to {}", recipient.inner());
        Ok(())
    }
}
