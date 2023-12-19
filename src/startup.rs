use crate::adapters::postgres_subscriber_repository::PostgresSubscriberRepository;
use crate::adapters::postmark_email_client::PostmarkEmailClient;
use crate::configuration::{DatabaseSettings, Settings};
use crate::domain::email_client::EmailClient;
use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_repository::SubscriberRepository;
use crate::routes::health_check::health_check;
use crate::routes::migrate::migrate_db;
use crate::routes::subscriptions::subscribe;
use crate::telemetry::CustomLevelRootSpanBuilder;
use actix_web::dev::{Server, Service};
use actix_web::web::Data;
use actix_web::{web, App, HttpMessage, HttpServer};
use reqwest::header::{HeaderName, HeaderValue};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::net::TcpListener;
use std::sync::Arc;
use tracing_actix_web::{RequestId, TracingLogger};

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        let connection_pool = get_connection_pool(&configuration.database);

        let email_adapter: Arc<dyn EmailClient + Send + Sync> = Arc::new(PostmarkEmailClient::new(
            configuration.email_settings.base_url.clone(),
            SubscriberEmail::parse(configuration.email_settings.sender_email.clone()).unwrap(),
            configuration.email_settings.authorization_token.clone(),
            configuration.email_settings.timeout(),
        ));

        let listener = TcpListener::bind(format!(
            "{}:{}",
            configuration.application.host_name, configuration.application.application_port
        ))?;

        let port = listener.local_addr().unwrap().port();
        let server = run(listener, connection_pool, email_adapter)?;

        Ok(Self { server, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn run(
    listener: TcpListener,
    connection: PgPool,
    email_client: Arc<dyn EmailClient + Send + Sync>,
) -> Result<Server, std::io::Error> {
    let repository = PostgresSubscriberRepository::new(connection);

    let server = HttpServer::new(move || {
        let repo_arc: Arc<dyn SubscriberRepository> = Arc::new(repository.clone());
        let store_data: Data<dyn SubscriberRepository> = Data::from(repo_arc);
        let email_client_data: Data<dyn EmailClient + Send + Sync> =
            Data::from(email_client.clone());

        App::new()
            .route("/health_check", web::get().to(health_check))
            .wrap_fn(|req, srv| {
                let request_id = req.extensions().get::<RequestId>().copied();
                let res = srv.call(req);
                async move {
                    let mut res = res.await?;
                    if let Some(request_id) = request_id {
                        res.headers_mut().insert(
                            HeaderName::from_static("x-request-id"),
                            // this unwrap never fails, since UUIDs are valid ASCII strings
                            HeaderValue::from_str(&request_id.to_string()).unwrap(),
                        );
                    }
                    Ok(res)
                }
            })
            .wrap(TracingLogger::<CustomLevelRootSpanBuilder>::new())
            .route("/_migrate", web::get().to(migrate_db))
            .route("/subscriptions", web::post().to(subscribe))
            .app_data(store_data)
            .app_data(email_client_data)
    })
    .listen(listener)?
    .run();

    Ok(server)
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new().connect_lazy_with(configuration.with_db())
}
