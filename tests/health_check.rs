use sqlx::{Connection, PgConnection};
use std::net::TcpListener;
use tokio::spawn;
use zero2prod::configuration::get_configuration;

#[tokio::test]
async fn health_check_works() {
    let endpoint = spawn_app();
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health_check", endpoint))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[tokio::test]
async fn subscribe_should_return_200_when_valid_post() {
    let endpoint = spawn_app();
    let configuration = get_configuration().expect("Failed to read configuration");
    let connection_string = configuration.database.connection_string();

    let mut connection = PgConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to postgres");

    let client = reqwest::Client::new();

    let body = "name=james&email=james@test.com";

    let response = client
        .post(&format!("{}/subscriptions", endpoint))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&mut connection)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "james@test.com");
    assert_eq!(saved.name, "james");
}

#[tokio::test]
async fn subscribe_should_return_400_when_data_is_missing() {
    let endpoint = spawn_app();
    let client = reqwest::Client::new();

    let test_cases = vec![
        ("name=james", "missing the email"),
        ("email=james@test.com", "missing the name"),
        ("", "Missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(&format!("{}/subscriptions", endpoint))
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

fn spawn_app() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failure binding to address");

    let port = listener.local_addr().unwrap().port();

    let server = zero2prod::startup::run(listener).expect("Failed to bind address");

    let _ = tokio::spawn(server);

    format!("http://127.0.0.1:{}", port)
}
