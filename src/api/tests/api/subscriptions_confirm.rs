use crate::helpers::spawn_app;
use aws_sdk_dynamodb::types::AttributeValue;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    // Arrange
    let app = spawn_app().await;

    // Act
    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    // Assert
    assert_eq!(response.status().as_u16(), 400);
}

#[tokio::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber() {
    // Arrange
    let app = spawn_app().await;
    let body = "name=james&email=james@test.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let token = app.get_token_for_email("james@test.com").await;

    // Act
    app.confirm_subscription(token).await;

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
    assert_eq!(saved["GSI1PK"].as_s().unwrap(), &"confirmed".to_string());
}
