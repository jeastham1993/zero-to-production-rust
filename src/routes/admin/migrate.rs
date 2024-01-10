use crate::authentication::UserRepository;
use actix_web::{web, HttpResponse};

#[tracing::instrument(name = "Database migration", skip(connection))]
pub async fn migrate_db(connection: web::Data<dyn UserRepository>) -> HttpResponse {
    match connection.seed().await {
        Ok(_) => {
            tracing::info!("Database seeded successfully");

            HttpResponse::Ok().finish()
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}
