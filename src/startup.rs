use crate::adapters::postgres_subscriber_repository::PostgresSubscriberRepository;
use crate::adapters::postmark_email_client::PostmarkEmailClient;
use crate::configuration::{DatabaseSettings, EmailClientSettings, Settings};
use crate::domain::email_client::EmailClient;
use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_repository::SubscriberRepository;
use crate::routes::health_check::health_check;
use crate::routes::migrate::migrate_db;
use crate::routes::subscriptions::subscribe;
use crate::routes::subscriptions_confirm::confirm;
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

pub struct ApplicationBaseUrl(pub String);

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        let connection_pool = get_connection_pool(&configuration.database);

        let listener = TcpListener::bind(format!(
            "{}:{}",
            configuration.application.host_name, configuration.application.application_port
        ))?;

        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            connection_pool,
            configuration.email_settings,
            configuration.application.base_url,
        )?;

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
    email_settings: EmailClientSettings,
    base_url: String,
) -> Result<Server, std::io::Error> {
    let repository = PostgresSubscriberRepository::new(connection);

    let email_adapter = PostmarkEmailClient::new(
        email_settings.base_url.clone(),
        SubscriberEmail::parse(email_settings.sender_email.clone()).unwrap(),
        email_settings.authorization_token.clone(),
        email_settings.timeout(),
    );

    let base_url = Data::new(ApplicationBaseUrl(base_url));

    let server = HttpServer::new(move || {
        let repo_arc: Arc<dyn SubscriberRepository> = Arc::new(repository.clone());
        let store_data: Data<dyn SubscriberRepository> = Data::from(repo_arc);

        let email_client_arc: Arc<dyn EmailClient> = Arc::new(email_adapter.clone());
        let email_client_data: Data<dyn EmailClient> = Data::from(email_client_arc.clone());

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
            .route("/subscriptions/confirm", web::get().to(confirm))
            .app_data(store_data)
            .app_data(email_client_data)
            .app_data(base_url.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new().connect_lazy_with(configuration.with_db())
}
