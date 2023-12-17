use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let configuration = get_configuration().expect("Failed to read configuration");

    let connection = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failure connecting to Postgres database");

    let listener = TcpListener::bind("127.0.0.1:8080").expect("Failure binding to address");

    run(listener, connection)?.await
}
