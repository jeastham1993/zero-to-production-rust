use crate::configuration::TelemetrySettings;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{SpanExporterBuilder, WithExportConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{config, Config, TracerProvider};
use opentelemetry_sdk::{runtime, Resource};
use secrecy::ExposeSecret;
use std::collections::HashMap;

use tracing::subscriber::set_global_default;
use tracing::{level_filters::LevelFilter, Span, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

pub fn init_tracer(trace_config: &TelemetrySettings) -> TracerProvider {
    let span_exporter = match trace_config.otlp_endpoint.as_str() {
        "jaeger" => {
            return opentelemetry_jaeger::new_agent_pipeline()
                .with_endpoint("localhost:6831")
                .with_service_name(trace_config.dataset_name.clone())
                .with_trace_config(config().with_resource(Resource::new(vec![KeyValue::new(
                    opentelemetry_semantic_conventions::resource::SERVICE_NAME.to_string(),
                    trace_config.dataset_name.clone(),
                )])))
                .build_simple()
                .unwrap();
        }
        _ => opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(trace_config.otlp_endpoint.clone())
            .with_http_client(reqwest::Client::default())
            .with_headers(HashMap::from([
                (
                    "x-honeycomb-dataset".into(),
                    trace_config.dataset_name.clone(),
                ),
                (
                    "x-honeycomb-team".into(),
                    trace_config.honeycomb_api_key.expose_secret().into(),
                ),
            ]))
            .with_timeout(std::time::Duration::from_secs(2)),
    };

    TracerProvider::builder()
        .with_config(
            Config::default().with_resource(Resource::new(vec![KeyValue::new(
                opentelemetry_semantic_conventions::resource::SERVICE_NAME.to_string(),
                trace_config.dataset_name.clone(),
            )])),
        )
        .with_batch_exporter(
            SpanExporterBuilder::Http(span_exporter)
                .build_span_exporter()
                .unwrap(),
            runtime::Tokio,
        )
        .build()
}


/// Compose multiple layers into a tracing subscriber.
pub fn get_subscriber<Sink>(
    name: String,
    env_filter: String,
    sink: Sink,
    config: &TelemetrySettings,
    tracer: &opentelemetry_sdk::trace::Tracer
) -> impl Subscriber + Send + Sync
    where
        Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name, sink);

    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
        .with(tracing_subscriber::fmt::Layer::default())
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer.clone()),
        )
        .with(LevelFilter::DEBUG)
}

pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    let _ = LogTracer::init();
    global::set_text_map_propagator(TraceContextPropagator::new());

    let _ = set_global_default(subscriber);
}
