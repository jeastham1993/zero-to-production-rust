use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::configuration::get_configuration;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subscriber, init_subscriber, init_tracer};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let configuration = get_configuration().expect("Failed to read configuration");

    let tracer = init_tracer(&configuration.telemetry);
    let subscriber = get_subscriber(
        "zero2prod".into(),
        "info".into(),
        std::io::stdout,
        &configuration.telemetry,
        &tracer,
    );

    init_subscriber(subscriber);

    let connection =
        PgPool::connect_lazy(configuration.database.connection_string().expose_secret())
            .expect("Failed to create Postgres connection pool");

    let listener = TcpListener::bind(format!(
        "{}:{}",
        configuration.application.host_name, configuration.application.application_port
    ))
    .expect("Failure binding to address");
    run(listener, connection)?.await
}
