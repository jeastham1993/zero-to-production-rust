use aws_lambda_events::sqs::SqsMessage;
use opentelemetry::trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState, TracerProvider as _};
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::{SpanExporterBuilder, WithExportConfig};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::{config, Config, TracerProvider};
use opentelemetry_sdk::{runtime, Resource};
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use tracing_actix_web::{DefaultRootSpanBuilder, Level, RootSpanBuilder};
use std::collections::HashMap;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::rt::task::JoinHandle;
use tracing::subscriber::set_global_default;
use tracing::{level_filters::LevelFilter, Span, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};
use actix_web::Error;

/// Compose multiple layers into a tracing subscriber.
pub fn get_subscriber<Sink>(
    name: String,
    env_filter: String,
    sink: Sink,
    config: &TelemetrySettings,
    trace_provider: &TracerProvider,
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
                .with_tracer(trace_provider.tracer(config.dataset_name.clone())),
        )
        .with(LevelFilter::DEBUG)
}

pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    let _ = LogTracer::init();
    global::set_text_map_propagator(TraceContextPropagator::new());

    let _ = set_global_default(subscriber);
}

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
        "http://localhost:4318" => opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(trace_config.otlp_endpoint.clone())
            .with_http_client(reqwest::Client::default())
            .with_timeout(std::time::Duration::from_secs(2)),
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

pub async fn parse_context_from(record: &SqsMessage) -> Result<opentelemetry::Context, ()> {
    let message_body: Result<TracedMessage, serde_json::Error> =
        serde_json::from_str(record.body.as_ref().unwrap().as_str());

    let traced_message = match message_body {
        Ok(message) => message,
        Err(_) => return Err(()),
    };

    let trace_id = TraceId::from_hex(traced_message.trace_parent.as_str()).unwrap();
    let span_id = SpanId::from_hex(traced_message.parent_span.as_str()).unwrap();

    let span_context = SpanContext::new(
        trace_id,
        span_id,
        TraceFlags::SAMPLED,
        false,
        TraceState::NONE,
    );

    let ctx = opentelemetry::Context::new().with_remote_span_context(span_context.clone());

    Ok(ctx)
}

pub fn get_trace_and_span_id() -> Option<(String, String)> {
    // Access the current span
    let current_span = Span::current();
    // Retrieve the context from the current span
    let context = current_span.context();
    // Use OpenTelemetry's API to retrieve the TraceContext
    let span_context = context.span().span_context().clone();

    // Check if the span context is valid
    if span_context.is_valid() {
        // Retrieve traceId and spanId
        let trace_id = span_context.trace_id().to_string().clone();
        let span_id = span_context.span_id().to_string().clone();
        Some((trace_id, span_id))
    } else {
        // No valid span context found
        None
    }
}

pub struct CustomLevelRootSpanBuilder;

impl RootSpanBuilder for CustomLevelRootSpanBuilder {
    fn on_request_start(request: &ServiceRequest) -> Span {
        let paths_to_skip = ["/health_check", "/default", "/"];

        let level = if paths_to_skip.contains(&request.path()) {
            Level::TRACE
        } else {
            Level::INFO
        };

        let root_span: tracing::Span = tracing_actix_web::root_span!(level = level, request, "dd.trace_id" = tracing::field::Empty, "dd.span_id" = tracing::field::Empty, "random_rubbish" = tracing::field::Empty);

        let context = root_span.context();
        // Use OpenTelemetry's API to retrieve the TraceContext
        let span_context = context.span().span_context().clone();

        let trace_id = span_context.trace_id().to_string().clone();
        let span_id = span_context.span_id().to_string().clone();

        let dd_trace_id = u64::from_str_radix(&trace_id[16..], 16)
            .expect("Failed to convert string_trace_id to a u64.")
            .to_string();

        let dd_span_id = u64::from_str_radix(&span_id, 16)
            .expect("Failed to convert string_span_id to a u64.")
            .to_string();

        root_span.record("dd.trace_id", dd_trace_id);
        root_span.record("dd.span_id", dd_span_id);

        root_span
    }

    fn on_request_end<B: MessageBody>(span: Span, outcome: &Result<ServiceResponse<B>, Error>) {
        let _current_span = tracing::Span::current();

        DefaultRootSpanBuilder::on_request_end(span, outcome);
    }
}

pub fn spawn_blocking_with_tracing<F, R>(f: F) -> JoinHandle<R>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
{
    let current_span = tracing::Span::current();
    actix_web::rt::task::spawn_blocking(move || current_span.in_scope(f))
}

#[derive(Deserialize)]
struct TracedMessage {
    trace_parent: String,
    parent_span: String,
}

#[derive(Deserialize, Clone)]
pub struct TelemetrySettings {
    pub otlp_endpoint: String,
    pub honeycomb_api_key: Secret<String>,
    pub dataset_name: String
}
