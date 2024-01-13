mod configuration;
mod startup;
mod telemetry;


use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_config::meta::region::RegionProviderChain;
use aws_config::{BehaviorVersion, Region};

use aws_lambda_events::event::dynamodb::Event;
use aws_sdk_dynamodb::config::ProvideCredentials;
use aws_smithy_runtime::client::http::hyper_014::HyperClientBuilder;
use backend::adapters::postmark_email_client::PostmarkEmailClient;
use backend::configuration::{get_configuration, Settings};
use backend::domain::email_client::EmailClient;
use backend::domain::subscriber_email::SubscriberEmail;
use backend::telemetry::{get_subscriber, init_tracer, parse_context};

use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use opentelemetry::global;
use opentelemetry::trace::{
    TracerProvider,
};
use opentelemetry_sdk::propagation::TraceContextPropagator;



use tracing::subscriber::set_global_default;


use backend::handler::handle_record;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let configuration = get_configuration().expect("Failed to read configuration");

    let region = make_region_provider().region().await.unwrap();
    let credentials = DefaultCredentialsChain::builder()
        .region(region.clone())
        .build()
        .await
        .provide_credentials()
        .await
        .unwrap();

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

    let conf_builder = aws_sdk_dynamodb::Config::builder()
        .behavior_version(BehaviorVersion::v2023_11_09())
        .credentials_provider(credentials.clone())
        .http_client(hyper_client)
        .region(region.clone());

    let _conf = match configuration.database.use_local {
        true => conf_builder.endpoint_url("http://localhost:8000").build(),
        false => conf_builder.build(),
    };

    run(service_fn(|evt| {
        function_handler(
            evt,
            &configuration,
            &email_adapter,
            &configuration.base_url,
        )
    }))
    .await
}

async fn function_handler<TEmail: EmailClient>(
    event: LambdaEvent<Event>,
    configuration: &Settings,
    email_client: &TEmail,
    base_url: &str,
) -> Result<(), Error> {
    // Extract some useful information from the request
    for record in event.payload.records {
        let provider = init_tracer(&configuration.telemetry);
        let tracer = &provider.tracer("zero2prod-backend");

        let ctx = match parse_context(&record).await {
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

        match handle_record(&ctx, record, email_client, base_url).await {
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
