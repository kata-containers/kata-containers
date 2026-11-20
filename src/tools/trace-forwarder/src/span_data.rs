// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use opentelemetry::sdk::export::trace::SpanData as AgentSpanData;
use opentelemetry::{trace as agent_trace, Array as AgentArray, Value as AgentValue};
use opentelemetry_otlp_api::trace::{
    Event, Link, SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId,
};
use opentelemetry_otlp_api::{Array, InstrumentationScope, KeyValue, Value};
use opentelemetry_sdk::trace::{SpanData, SpanEvents, SpanLinks};

fn convert_context(context: &agent_trace::SpanContext) -> Result<SpanContext> {
    Ok(SpanContext::new(
        TraceId::from_bytes(context.trace_id().to_bytes()),
        SpanId::from_bytes(context.span_id().to_bytes()),
        TraceFlags::new(context.trace_flags().to_u8()),
        context.is_remote(),
        context
            .trace_state()
            .header()
            .parse()
            .context("invalid agent trace state")?,
    ))
}

fn convert_value(value: &AgentValue) -> Value {
    match value {
        AgentValue::Bool(value) => Value::Bool(*value),
        AgentValue::I64(value) => Value::I64(*value),
        AgentValue::F64(value) => Value::F64(*value),
        AgentValue::String(value) => Value::String(value.to_string().into()),
        AgentValue::Array(value) => Value::Array(match value {
            AgentArray::Bool(values) => Array::Bool(values.clone()),
            AgentArray::I64(values) => Array::I64(values.clone()),
            AgentArray::F64(values) => Array::F64(values.clone()),
            AgentArray::String(values) => Array::String(
                values
                    .iter()
                    .map(|value| value.to_string().into())
                    .collect(),
            ),
        }),
    }
}

fn convert_attribute(key: &opentelemetry::Key, value: &AgentValue) -> KeyValue {
    KeyValue::new(key.as_str().to_owned(), convert_value(value))
}

pub fn convert_span(span: AgentSpanData) -> Result<SpanData> {
    let mut events = SpanEvents::default();
    events.dropped_count = span.events.dropped_count();
    events.events = span
        .events
        .iter()
        .map(|event| {
            Event::new(
                event.name.clone(),
                event.timestamp,
                event
                    .attributes
                    .iter()
                    .map(|attribute| convert_attribute(&attribute.key, &attribute.value))
                    .collect(),
                event.dropped_attributes_count,
            )
        })
        .collect();

    let mut links = SpanLinks::default();
    links.dropped_count = span.links.dropped_count();
    links.links = span
        .links
        .iter()
        .map(|link| {
            Ok(Link::new(
                convert_context(link.span_context())?,
                link.attributes()
                    .iter()
                    .map(|attribute| convert_attribute(&attribute.key, &attribute.value))
                    .collect(),
                link.dropped_attributes_count(),
            ))
        })
        .collect::<Result<_>>()?;

    let status = match span.status_code {
        agent_trace::StatusCode::Unset => Status::Unset,
        agent_trace::StatusCode::Ok => Status::Ok,
        agent_trace::StatusCode::Error => Status::error(span.status_message),
    };

    Ok(SpanData {
        span_context: convert_context(&span.span_context)?,
        parent_span_id: SpanId::from_bytes(span.parent_span_id.to_bytes()),
        // The agent's wire format does not include the parent's remote flag.
        parent_span_is_remote: false,
        span_kind: match span.span_kind {
            agent_trace::SpanKind::Internal => SpanKind::Internal,
            agent_trace::SpanKind::Server => SpanKind::Server,
            agent_trace::SpanKind::Client => SpanKind::Client,
            agent_trace::SpanKind::Producer => SpanKind::Producer,
            agent_trace::SpanKind::Consumer => SpanKind::Consumer,
        },
        name: span.name,
        start_time: span.start_time,
        end_time: span.end_time,
        dropped_attributes_count: span.attributes.dropped_count(),
        attributes: span
            .attributes
            .iter()
            .map(|(key, value)| convert_attribute(key, value))
            .collect(),
        events,
        links,
        // InstrumentationLibrary is omitted from the agent's serialized spans.
        instrumentation_scope: InstrumentationScope::builder("kata-trace-forwarder").build(),
        status,
    })
}
