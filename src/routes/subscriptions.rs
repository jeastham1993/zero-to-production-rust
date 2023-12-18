#[allow(unused_imports)]
use crate::domain::new_subscriber::NewSubscriber;
use crate::domain::subscriber_repository::SubscriberRepository;
use actix_web::{web, HttpResponse};

#[derive(serde::Deserialize)]
pub struct FormData {
    pub email: String,
    pub name: String,
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, repo),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name)
)]

pub async fn subscribe(
    form: web::Form<FormData>,
    repo: web::Data<dyn SubscriberRepository>,
) -> HttpResponse {
    let new_subscriber: NewSubscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    match repo.insert_subscriber(new_subscriber).await {
        Ok(_) => {
            tracing::info!("New subscriber saved");

            HttpResponse::Ok().finish()
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
