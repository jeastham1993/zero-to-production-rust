use crate::domain::email_client::EmailClient;
#[allow(unused_imports)]
use crate::domain::new_subscriber::NewSubscriber;
use crate::domain::subscriber_repository::{StoreTokenError, SubscriberRepository};
use crate::startup::ApplicationBaseUrl;
use actix_web::{web, HttpResponse, ResponseError};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx::{Error, Postgres, Transaction};

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, repo, email_client, base_url),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name)
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    repo: web::Data<dyn SubscriberRepository>,
    email_client: web::Data<dyn EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> HttpResponse {
    let new_subscriber: NewSubscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    let subscriber_id = match repo.insert_subscriber(&new_subscriber).await {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let subscription_token = generate_subscription_token();

    if repo
        .store_token(subscriber_id, &subscription_token)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }

    match send_confirmation_email(
        email_client.get_ref(),
        new_subscriber,
        &subscription_token,
        &base_url.0,
    )
    .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(err) => {
            let error_message = err.to_string();
            println!("{}", error_message);

            tracing::error_span!("Error");

            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber, subscription_token, base_url)
)]
pub async fn send_confirmation_email(
    email_client: &dyn EmailClient,
    new_subscriber: NewSubscriber,
    subscription_token: &str,
    base_url: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );
    let plain_body = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    let html_body = format!(
        "Welcome to our newsletter!<br />Click <a href=\"{}\">here</a> to confirm your subscription.",
        confirmation_link
    );
    email_client
        .send_email_to(&new_subscriber.email, "Welcome!", &html_body, &plain_body)
        .await
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}
