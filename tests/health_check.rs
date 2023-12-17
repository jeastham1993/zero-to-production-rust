use once_cell::sync::Lazy;
use opentelemetry::sdk::trace::TracerProvider;
use opentelemetry::trace::noop::NoopSpanExporter;
use secrecy::ExposeSecret;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;
use zero2prod::configuration::{get_configuration, DatabaseSettings};
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

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

#[tokio::test]
async fn health_check_works() {
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health_check", test_app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_should_return_200_when_valid_post() {
    let test_app = spawn_app().await;

    let client = reqwest::Client::new();

    let body = "name=james&email=james@test.com";

    let response = client
        .post(&format!("{}/subscriptions", test_app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&test_app.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "james@test.com");
    assert_eq!(saved.name, "james");
}

#[tokio::test]
async fn subscribe_should_return_400_when_data_is_missing() {
    let test_app = spawn_app().await;
    let client = reqwest::Client::new();

    let test_cases = vec![
        ("name=james", "missing the email"),
        ("email=james@test.com", "missing the name"),
        ("", "Missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", test_app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 bad request when the payload was {}",
            error_message
        );
    }
}

async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failure binding to address");

    let port = listener.local_addr().unwrap().port();

    let mut configuration = get_configuration().expect("Failed to read configuration");
    configuration.database.database_name = Uuid::new_v4().to_string();

    let connection = configure_database(&configuration.database).await;

    let server =
        zero2prod::startup::run(listener, connection.clone()).expect("Failed to bind address");

    let _ = tokio::spawn(server);

    TestApp {
        address: format!("http://127.0.0.1:{}", port),
        db_pool: connection,
    }
}

pub async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let connection_string = config.connection_string_without_db();

    let mut connection = PgConnection::connect(&connection_string.expose_secret())
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
