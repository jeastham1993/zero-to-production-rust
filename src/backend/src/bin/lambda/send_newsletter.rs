use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_config::meta::region::RegionProviderChain;
use aws_config::{BehaviorVersion, Region};

use aws_lambda_events::event::sqs::SqsEvent;
use aws_sdk_dynamodb::config::ProvideCredentials;
use aws_sdk_s3::config::SharedHttpClient;
use aws_smithy_runtime::client::http::hyper_014::HyperClientBuilder;
use backend::adapters::postmark_email_client::PostmarkEmailClient;
use backend::configuration::{get_configuration, DatabaseSettings, Settings};
use backend::domain::email_client::EmailClient;
use backend::domain::subscriber_email::SubscriberEmail;
use backend::telemetry::{parse_context_from};
use telemetry::{init_tracer, get_subscriber};

use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;

use backend::adapters::dynamodb_subscriber_repository::DynamoDbSubscriberRepository;
use backend::adapters::s3_newsletter_service::S3NewsletterMetadataStorage;
use backend::domain::newsletter_store::NewsletterStore;
use backend::domain::subscriber_repository::SubscriberRepository;
use tracing::subscriber::set_global_default;

use backend::send_newsletter_handler::handle_record;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let configuration = get_configuration().await.expect("Failed to read configuration");

    let email_adapter = PostmarkEmailClient::new(
        configuration.email_settings.base_url.clone(),
        SubscriberEmail::parse(configuration.email_settings.sender_email.clone()).unwrap(),
        configuration.email_settings.authorization_token.clone(),
        configuration.email_settings.timeout_duration(),
    );

    let https_connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_webpki_roots()
        .https_or_http()
        .enable_http1()
        .enable_http2()
        .build();

    let hyper_client = HyperClientBuilder::new().build(https_connector);

    let s3_config = configure_s3(&hyper_client, &configuration.database).await;
    let dynamo_config = configure_dynamo(&hyper_client, &configuration.database).await;

    let s3_client = aws_sdk_s3::Client::from_conf(s3_config);
    let newsletter_service = S3NewsletterMetadataStorage::new(
        s3_client,
        configuration.database.newsletter_storage_bucket.clone(),
    );

    let dynamo_client = aws_sdk_dynamodb::Client::from_conf(dynamo_config);
    let subscriber_repo = DynamoDbSubscriberRepository::new(
        dynamo_client,
        configuration.database.database_name.clone(),
    );

    run(service_fn(|evt| {
        function_handler(
            evt,
            &configuration,
            &email_adapter,
            &newsletter_service,
            &subscriber_repo,
        )
    }))
    .await
}

async fn function_handler<
    TEmail: EmailClient,
    TNewsletterStore: NewsletterStore,
    TRepo: SubscriberRepository,
>(
    event: LambdaEvent<SqsEvent>,
    configuration: &Settings,
    email_client: &TEmail,
    newsletter_store: &TNewsletterStore,
    repo: &TRepo,
) -> Result<(), Error> {
    // Extract some useful information from the request
    for record in event.payload.records {
        let provider = init_tracer(&configuration.telemetry);
        let tracer = &provider.tracer("zero2prod-backend");

        let ctx = match parse_context_from(&record).await {
            Ok(res) => res,
            Err(_) => continue,
        };

        let subscriber = get_subscriber(
            configuration.telemetry.dataset_name.clone(),
            "info".into(),
            std::io::stdout,
            &configuration.telemetry,
            tracer,
        );

        global::set_text_map_propagator(TraceContextPropagator::new());
        let _ = set_global_default(subscriber);

        match handle_record(&ctx, record, email_client, repo, newsletter_store).await {
            Ok(_) => {}
            Err(e) => {
                let error_msg = format!("Failure handling DynamoDB stream record. Error: {}", e);

                tracing::error!(error_msg);
            }
        };

        provider.force_flush();
    }

    Ok(())
}

pub fn make_region_provider() -> RegionProviderChain {
    RegionProviderChain::default_provider().or_else(Region::new("us-east-1"))
}

async fn configure_s3(
    hyper_client: &SharedHttpClient,
    db_settings: &DatabaseSettings,
) -> aws_sdk_s3::Config {
    let region = match db_settings.use_local {
        true => Region::new("eu-west-1"),
        false => RegionProviderChain::default_provider()
            .or_else(Region::new("eu-west-1"))
            .region()
            .await
            .unwrap(),
    };

    let credentials = DefaultCredentialsChain::builder()
        .region(region.clone())
        .build()
        .await
        .provide_credentials()
        .await
        .unwrap();

    aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::v2023_11_09())
        .credentials_provider(credentials.clone())
        .http_client(hyper_client.clone())
        .region(region.clone())
        .build()
}

async fn configure_dynamo(
    hyper_client: &SharedHttpClient,
    db_settings: &DatabaseSettings,
) -> aws_sdk_dynamodb::Config {
    let region = RegionProviderChain::default_provider()
        .or_else(Region::new("us-east-1"))
        .region()
        .await
        .unwrap();

    let credentials = DefaultCredentialsChain::builder()
        .region(region.clone())
        .build()
        .await
        .provide_credentials()
        .await
        .unwrap();

    let conf_builder = aws_sdk_dynamodb::Config::builder()
        .behavior_version(BehaviorVersion::v2023_11_09())
        .credentials_provider(credentials.clone())
        .http_client(hyper_client.clone())
        .region(region.clone());

    match db_settings.use_local {
        true => conf_builder.endpoint_url("http://localhost:8000").build(),
        false => conf_builder.build(),
    }
}
