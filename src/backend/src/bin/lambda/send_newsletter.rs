use std::sync::Arc;
use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_config::meta::region::RegionProviderChain;
use aws_config::{BehaviorVersion, Region};

use aws_lambda_events::event::sqs::SqsEvent;
use aws_sdk_dynamodb::config::ProvideCredentials;
use aws_sdk_s3::config::SharedHttpClient;
use aws_smithy_runtime::client::http::hyper_014::HyperClientBuilder;
use lambda_extension::Extension;
use backend::adapters::postmark_email_client::PostmarkEmailClient;
use backend::configuration::{get_configuration, DatabaseSettings};
use backend::domain::subscriber_email::SubscriberEmail;
use telemetry::{init_tracer, get_subscriber, init_subscriber, TraceFlushExtension};

use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use tokio::sync::mpsc::unbounded_channel;

use backend::adapters::dynamodb_subscriber_repository::DynamoDbSubscriberRepository;
use backend::adapters::s3_newsletter_service::S3NewsletterMetadataStorage;

use backend::send_newsletter_handler::{SendNewsletterEventHandler};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let configuration = get_configuration().await.expect("Failed to read configuration");

    let tracer = init_tracer(&configuration.telemetry);
    let subscriber = get_subscriber(
        configuration.telemetry.dataset_name.clone(),
        "info".into(),
        std::io::stdout,
        &configuration.telemetry,
        &tracer,
    );

    init_subscriber(subscriber);

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

    let (request_done_sender, request_done_receiver) = unbounded_channel::<()>();

    let flush_extension = Arc::new(TraceFlushExtension::new(request_done_receiver));

    let arc_tracer = Arc::new(tracer);
    let extension = Extension::new()
        // Internal extensions only support INVOKE events.
        .with_events(&["INVOKE"])
        .with_events_processor(service_fn(|event| {
            let cloned_tracer = arc_tracer.clone();

            let flush_extension = flush_extension.clone();
            async move { flush_extension.invoke(event, cloned_tracer).await }
        }))
        // Internal extension names MUST be unique within a given Lambda function.
        .with_extension_name("internal-flush")
        // Extensions MUST be registered before calling lambda_runtime::run(), which ends the Init
        // phase and begins the Invoke phase.
        .register()
        .await?;

    let handler = Arc::new(SendNewsletterEventHandler::new(request_done_sender));

    //https://github.com/awslabs/aws-lambda-rust-runtime/blob/main/examples/extension-internal-flush/src/main.rs
    tokio::try_join!(
        run(service_fn(|event: LambdaEvent<SqsEvent>| {
            let handler = handler.clone();
            let email_adapter = email_adapter.clone();
            let repo = subscriber_repo.clone();
            let newsletter_store = newsletter_service.clone();
            
            async move { handler.invoke(event, &email_adapter, &repo, &newsletter_store).await }
        })),
        extension.run(),
    )?;

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
