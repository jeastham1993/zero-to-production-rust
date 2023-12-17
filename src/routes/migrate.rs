use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[tracing::instrument(name = "Database migration", skip(connection))]

pub async fn migrate_db(connection: web::Data<PgPool>) -> HttpResponse {
    match migrate_database(connection.get_ref()).await {
        Ok(_) => {
            tracing::info!("New subscriber saved");

            HttpResponse::Ok().finish()
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[tracing::instrument(name = "Executing migration", skip(connection))]
pub async fn migrate_database(connection: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::migrate!("./migrations")
        .run(connection)
        .await
        .expect("Failed to migrate database");

    Ok(())
}
