mod telemetry;
mod configuration;
mod startup;

use std::future::Future;
use std::time::Duration;
use aws_lambda_events::dynamodb::EventRecord;
use aws_lambda_events::event::dynamodb::Event;use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use opentelemetry::{Context, global};
use opentelemetry::trace::{FutureExt, Span, SpanContext, SpanId, SpanKind, TraceContextExt, TraceFlags, TraceId, Tracer, TracerProvider, TraceState};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use crate::configuration::{get_configuration, Settings};
use crate::telemetry::{get_subscriber, init_subscriber, init_tracer};
use serde_dynamo::AttributeValue;
use tracing::{error, Instrument, span};
use tracing::subscriber::set_global_default;
use tracing_opentelemetry::OpenTelemetrySpanExt;

async fn function_handler(event: LambdaEvent<Event>) -> Result<(), Error> {
    // Extract some useful information from the request
    for record in event.payload.records {
        let configuration = get_configuration().expect("Failed to read configuration");

        let provider = init_tracer(&configuration.telemetry);
        let tracer = &provider.tracer("zero2prod-backend");

        let (ctx, span_ctx) = match parse_context(&record).await{
            Ok(res) => res,
            Err(_) => continue
        };

        let span = tracer.start_with_context("Processing DynamoDB record", &ctx);

        let subscriber = get_subscriber(
            configuration.telemetry.dataset_name.clone(),
            "info".into(),
            std::io::stdout,
            &configuration.telemetry,
            &tracer,
        );

        global::set_text_map_propagator(TraceContextPropagator::new());
        set_global_default(subscriber);

        do_work(&Context::new()
            .with_span(span));

        provider.force_flush();
    }

    Ok(())
}

async fn parse_context(record: &EventRecord) -> Result<(opentelemetry::Context, SpanContext), ()> {
    if !record.change.new_image.contains_key("Type") {
        return Err(());
    }

    let (_, type_value) = record.change.new_image.get_key_value("Type").unwrap();

    let parsed_type_value = match type_value {
        AttributeValue::S(val) => val,
        _ => return Err(())
    };

    if parsed_type_value != "Subscriber" {
        return Err(());
    }

    let (_, trace_parent_value) = record.change.new_image.get_key_value("TraceParent").unwrap();
    let (_, parent_span_value) = record.change.new_image.get_key_value("ParentSpan").unwrap();

    let trace_parent_value = match trace_parent_value {
        AttributeValue::S(val) => val,
        _ => return Err(())
    };

    let parent_span_value = match parent_span_value {
        AttributeValue::S(val) => val,
        _ => return Err(())
    };

    let trace_id = TraceId::from_hex(trace_parent_value).unwrap();
    let span_id = SpanId::from_hex(parent_span_value).unwrap();

    let span_context = SpanContext::new(
        trace_id,
        span_id,
        TraceFlags::SAMPLED,
        false,
        TraceState::NONE
    );

    let ctx = Context::new()
        .with_remote_span_context(span_context.clone());

    Ok((ctx, span_context))
}

#[tracing::instrument]
fn do_work(context: &Context) {
    tracing::Span::current().set_parent(context.clone());

    std::thread::sleep(Duration::from_secs(1));

    do_some_more_work();
}

#[tracing::instrument]
fn do_some_more_work() {
    std::thread::sleep(Duration::from_secs(1));
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(|evt| {
        function_handler(evt)
    })).await
}