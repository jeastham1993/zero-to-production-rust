use aws_lambda_events::sqs::SqsMessage;
use opentelemetry::trace::{SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState};
use serde::Deserialize;

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

#[derive(Deserialize)]
struct TracedMessage {
    trace_parent: String,
    parent_span: String,
}
