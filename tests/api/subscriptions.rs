use crate::helpers::spawn_app;

#[tokio::test]
async fn subscribe_should_return_200_when_valid_post() {
    let test_app = spawn_app().await;

    let body = "name=james&email=james@test.com";

    let response = test_app.post_subscriptions(body.into()).await;

    assert!(response.status().is_success());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions")
        .fetch_one(&test_app.db_pool)
        .await
        .expect("Failed to fetch saved subscription");

    assert_eq!(saved.email, "james@test.com");
    assert_eq!(saved.name, "james");
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
