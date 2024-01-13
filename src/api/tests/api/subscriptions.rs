use crate::helpers::spawn_app;
use aws_sdk_dynamodb::types::AttributeValue;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    // Arrange
    let app = spawn_app().await;
    let body = "name=james&email=james@test.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    // Act
    let response = app.post_subscriptions(body.into()).await;

    // Assert
    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn subscribe_persists_the_new_subscriber() {
    // Arrange
    let app = spawn_app().await;
    let body = "name=james&email=james@test.com";

    // Act
    app.post_subscriptions(body.into()).await;

    // Assert
    let saved = app
        .dynamo_db_client
        .get_item()
        .table_name(app.table_name)
        .key("PK", AttributeValue::S("james@test.com".to_string()))
        .send()
        .await
        .unwrap()
        .item
        .unwrap();

    assert_eq!(saved["PK"].as_s().unwrap(), &"james@test.com".to_string());
}

#[tokio::test]
async fn subscribe_should_return_a_400_when_fields_are_present_but_invalid() {
    let test_app = spawn_app().await;

    let test_cases = vec![
        ("name=&email=ursula_le_guin%40gmail.com", "Empty name"),
        ("name=Ursula&email=", "Empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = test_app.post_subscriptions(invalid_body.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did return a 400 when the payload was {}",
            error_message
        );
    }
}

#[tokio::test]
async fn subscribe_should_return_400_when_data_is_missing() {
    let test_app = spawn_app().await;

    let test_cases = vec![
        ("name=james", "missing the email"),
        ("email=james@test.com", "missing the name"),
        ("", "Missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = test_app.post_subscriptions(invalid_body.into()).await;

        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 bad request when the payload was {}",
            error_message
        );
    }
}
