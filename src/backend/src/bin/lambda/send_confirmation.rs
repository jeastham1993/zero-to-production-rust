use anyhow::anyhow;
use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_config::meta::region::RegionProviderChain;
use aws_config::{BehaviorVersion, Region};

use aws_lambda_events::event::sqs::SqsEvent;
use aws_sdk_dynamodb::config::ProvideCredentials;
use aws_smithy_runtime::client::http::hyper_014::HyperClientBuilder;
use lambda_extension::{service_fn, Error, Extension, NextEvent};
use backend::adapters::postmark_email_client::PostmarkEmailClient;
use backend::configuration::{get_configuration};
use backend::domain::subscriber_email::SubscriberEmail;
use telemetry::{init_tracer, get_subscriber, init_subscriber};

use lambda_runtime::{run, LambdaEvent};
use opentelemetry_sdk::trace::{config, Config, TracerProvider};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio::sync::Mutex;

use std::sync::Arc;
use backend::send_confirmation_handler::{SendConfirmationEventHandler};

/// Implements an internal Lambda extension to flush logs/telemetry after each request.
struct TraceFlushExtension {
    request_done_receiver: Mutex<UnboundedReceiver<()>>,
}

impl TraceFlushExtension {
    pub fn new(request_done_receiver: UnboundedReceiver<()>) -> Self {
        Self {
            request_done_receiver: Mutex::new(request_done_receiver),
        }
    }

    pub async fn invoke(&self, event: lambda_extension::LambdaEvent, tracer_provider: Arc<TracerProvider>) -> Result<(), Error> {
        match event.next {
            // NB: Internal extensions only support the INVOKE event.
            NextEvent::Shutdown(shutdown) => {
                return Err(anyhow!("extension received unexpected SHUTDOWN event: {:?}", shutdown).into());
            }
            NextEvent::Invoke(_e) => {}
        }

        eprintln!("[extension] waiting for event to be processed");

        // Wait for runtime to finish processing event.
        self.request_done_receiver
            .lock()
            .await
            .recv()
            .await
            .ok_or_else(|| anyhow!("channel is closed"))?;

        eprintln!("[extension] flushing logs and telemetry");

        tracer_provider.force_flush();

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let configuration = get_configuration().await.expect("Failed to read configuration");

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

    let (request_done_sender, request_done_receiver) = unbounded_channel::<()>();
    
    let tracer= init_tracer(&configuration.telemetry);
    let subscriber = get_subscriber(
        configuration.telemetry.dataset_name.clone(),
        "info".into(),
        std::io::stdout,
        &configuration.telemetry,
        &tracer,
    );

    init_subscriber(subscriber);

    let arc_tracer = Arc::new(tracer);

    let flush_extension = Arc::new(TraceFlushExtension::new(request_done_receiver));
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

    let handler = Arc::new(SendConfirmationEventHandler::new(request_done_sender));

    //https://github.com/awslabs/aws-lambda-rust-runtime/blob/main/examples/extension-internal-flush/src/main.rs
    tokio::try_join!(
        run(service_fn(|event: LambdaEvent<SqsEvent>| {
            let handler = handler.clone();
            let config = configuration.clone();
            let email_adapter = email_adapter.clone();
            
            async move { handler.invoke(event, &config, &email_adapter).await }
        })),
        extension.run(),
    )?;

    Ok(())
}

pub fn make_region_provider() -> RegionProviderChain {
    RegionProviderChain::default_provider().or_else(Region::new("us-east-1"))
}
