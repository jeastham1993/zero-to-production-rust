use crate::domain::subscriber_repository::SubscriberRepository;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sqlx::Error;

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(
    name = "Confirming a pending subscriber",
    skip(parameters, repo),
    fields()
)]
pub async fn confirm(
    parameters: web::Query<Parameters>,
    repo: web::Data<dyn SubscriberRepository>,
) -> HttpResponse {
    let id = match repo
        .get_subscriber_id_from_token(&parameters.subscription_token)
        .await
    {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    match id {
        None => HttpResponse::Unauthorized().finish(),
        Some(subscriber_id) => {
            if repo.confirm_subscriber(subscriber_id).await.is_err() {
                return HttpResponse::InternalServerError().finish();
            }
            HttpResponse::Ok().finish()
        }
    }
}
