use actix_web::{web, HttpResponse};

use crate::domain::subscriber_repository::SubscriberRepository;

#[tracing::instrument(name = "Database migration", skip(connection))]
pub async fn migrate_db(connection: web::Data<dyn SubscriberRepository>) -> HttpResponse {
    match connection.apply_migrations().await {
        Ok(_) => {
            tracing::info!("New subscriber saved");

            HttpResponse::Ok().finish()
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
