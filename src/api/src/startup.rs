use crate::adapters::dynamo_db_session_store::DynamoDbSessionStore;
use crate::adapters::dynamodb_subscriber_repository::DynamoDbSubscriberRepository;
use crate::adapters::dynamodb_user_repository::DynamoDbUserRepository;
use crate::adapters::postmark_email_client::PostmarkEmailClient;
use crate::authentication::{reject_anonymous_users, UserRepository};
use crate::configuration::{DatabaseSettings, EmailClientSettings, Settings};
use crate::domain::email_client::EmailClient;
use crate::domain::subscriber_email::SubscriberEmail;
use crate::domain::subscriber_repository::SubscriberRepository;
use crate::routes::{
    admin_dashboard, change_password, change_password_form, confirm, health_check, home, log_out,
    login, login_form, migrate_db, publish_newsletter, publish_newsletter_form, subscribe,
};
use crate::telemetry::CustomLevelRootSpanBuilder;
use actix_session::storage::RedisSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::Key;
use actix_web::dev::{Server, Service};
use actix_web::web::Data;
use actix_web::{web, App, HttpMessage, HttpServer};
use actix_web_flash_messages::storage::CookieMessageStore;
use actix_web_flash_messages::FlashMessagesFramework;
use actix_web_lab::middleware::from_fn;
use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_config::meta::region::RegionProviderChain;
use aws_config::{BehaviorVersion, Region};
use aws_sdk_dynamodb::config::ProvideCredentials;
use aws_smithy_runtime::client::http::hyper_014::HyperClientBuilder;
use reqwest::header::{HeaderName, HeaderValue};
use secrecy::{ExposeSecret, Secret};
use std::net::TcpListener;
use std::sync::Arc;
use tracing_actix_web::{RequestId, TracingLogger};

pub struct ApplicationBaseUrl(pub String);

pub struct Application {
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, anyhow::Error> {
        let listener = TcpListener::bind(format!(
            "{}:{}",
            configuration.application.host_name, configuration.application.application_port
        ))?;

        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            configuration.database,
            configuration.email_settings,
            configuration.application.base_url,
            configuration.application.hmac_secret,
            configuration.redis_uri,
        )
        .await?;

        Ok(Self { server, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

async fn run(
    listener: TcpListener,
    db_settings: DatabaseSettings,
    email_settings: EmailClientSettings,
    base_url: String,
    hmac_secret: Secret<String>,
    redis_uri: Secret<String>,
) -> Result<Server, anyhow::Error> {
    let secret_key = Key::from(hmac_secret.expose_secret().as_bytes());
    let message_store = CookieMessageStore::builder(secret_key.clone()).build();
    let message_framework = FlashMessagesFramework::builder(message_store).build();

    let dynamo_db_store = DynamoDbSessionStore::builder()
        .table_name(db_settings.auth_database_name.clone())
        .key_name("PK".to_string())
        .use_dynamo_db_local(db_settings.use_local)
        .build()
        .await?;

    let email_adapter = PostmarkEmailClient::new(
        email_settings.base_url.clone(),
        SubscriberEmail::parse(email_settings.sender_email.clone()).unwrap(),
        email_settings.authorization_token.clone(),
        email_settings.timeout(),
    );

    let region = make_region_provider().region().await.unwrap();
    let credentials = DefaultCredentialsChain::builder()
        .region(region.clone())
        .build()
        .await
        .provide_credentials()
        .await
        .unwrap();

    let base_url = Data::new(ApplicationBaseUrl(base_url));

    let server = HttpServer::new(move || {
        let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
            .with_webpki_roots()
            .https_or_http()
            .enable_http1()
            .enable_http2()
            .build();
        let hyper_client = HyperClientBuilder::new().build(https_connector);

        let conf_builder = aws_sdk_dynamodb::Config::builder()
            .behavior_version(BehaviorVersion::v2023_11_09())
            .credentials_provider(credentials.clone())
            .http_client(hyper_client)
            .region(region.clone());

        let conf = match db_settings.use_local {
            true => conf_builder.endpoint_url("http://localhost:8000").build(),
            false => conf_builder.build(),
        };

        let dynamodb_client = aws_sdk_dynamodb::Client::from_conf(conf.clone());
        let dynamo_db_repo =
            DynamoDbSubscriberRepository::new(dynamodb_client, db_settings.database_name.clone());
        let user_client = aws_sdk_dynamodb::Client::from_conf(conf.clone());
        let user_repo =
            DynamoDbUserRepository::new(user_client, db_settings.auth_database_name.clone());

        let repo_arc: Arc<dyn SubscriberRepository> = Arc::new(dynamo_db_repo.clone());
        let store_data: Data<dyn SubscriberRepository> = Data::from(repo_arc);

        let user_repo_arc: Arc<dyn UserRepository> = Arc::new(user_repo.clone());
        let user_repo_data: Data<dyn UserRepository> = Data::from(user_repo_arc);

        let email_client_arc: Arc<dyn EmailClient> = Arc::new(email_adapter.clone());
        let email_client_data: Data<dyn EmailClient> = Data::from(email_client_arc.clone());

        App::new()
            .wrap(message_framework.clone())
            .wrap(SessionMiddleware::new(
                dynamo_db_store.clone(),
                secret_key.clone(),
            ))
            .route("/", web::get().to(home))
            .service(
                web::scope("/admin")
                    .wrap(from_fn(reject_anonymous_users))
                    .route("/dashboard", web::get().to(admin_dashboard))
                    .route("/newsletters", web::get().to(publish_newsletter_form))
                    .route("/newsletters", web::post().to(publish_newsletter))
                    .route("/password", web::get().to(change_password_form))
                    .route("/password", web::post().to(change_password))
                    .route("/logout", web::post().to(log_out)),
            )
            .wrap(TracingLogger::<CustomLevelRootSpanBuilder>::new())
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
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(publish_newsletter))
            .route("/util/_migrate", web::get().to(migrate_db))
            .app_data(store_data)
            .app_data(user_repo_data)
            .app_data(email_client_data)
            .app_data(base_url.clone())
            .app_data(Data::new(HmacSecret(hmac_secret.clone())))
    })
    .listen(listener)?
    .run();

    Ok(server)
}

pub fn make_region_provider() -> RegionProviderChain {
    RegionProviderChain::default_provider().or_else(Region::new("us-east-1"))
}

#[derive(Clone)]
pub struct HmacSecret(pub Secret<String>);
