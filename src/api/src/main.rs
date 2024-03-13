use std::env;
use std::sync::Arc;
use telemetry::{get_subscriber, init_subscriber, init_tracer, TraceFlushExtension};
use zero2prod::configuration::get_configuration;
use zero2prod::startup::Application;
use lambda_extension::{service_fn, Extension};
use tokio::sync::mpsc::unbounded_channel;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    let configuration = get_configuration()
        .await
        .expect("Failed to read configuration");

    let tracer = init_tracer(&configuration.telemetry);
    let subscriber = get_subscriber(
        configuration.telemetry.dataset_name.clone(),
        "info".into(),
        std::io::stdout,
        &configuration.telemetry,
        &tracer,
    );

    init_subscriber(subscriber);

    let (request_done_sender, request_done_receiver) = unbounded_channel::<()>();

    let cloned_tracer = tracer.clone();

    if env::var("AWS_LAMBDA_RUNTIME_API").is_ok() {
        let _ = tokio::spawn(async move {
            let flush_extension = Arc::new(TraceFlushExtension::new(request_done_receiver));
            let extension = Extension::new()
                // Internal extensions only support INVOKE events.
                .with_events(&["INVOKE"])
                .with_events_processor(service_fn(|event| {
                    let cloned_tracer = cloned_tracer.clone();

                    let flush_extension = flush_extension.clone();
                    async move { flush_extension.invoke(event, Arc::new(cloned_tracer)).await }
                }))
                // Internal extension names MUST be unique within a given Lambda function.
                .with_extension_name("internal-flush")
                // Extensions MUST be registered before calling lambda_runtime::run(), which ends the Init
                // phase and begins the Invoke phase.
                .register()
                .await;

            extension.unwrap().run().await
        });
    }

    let application = Application::build(configuration, tracer, request_done_sender).await?;

    application.run_until_stopped().await?;

    Ok(())
}
