//! Thrift generated Jaeger client
//!
//! Definitions: https://github.com/uber/jaeger-idl/blob/master/thrift/
use std::time::{Duration, SystemTime};

use opentelemetry::trace::Event;
use opentelemetry::{Key, KeyValue, Value};

pub(crate) mod agent;
pub(crate) mod jaeger;
pub(crate) mod zipkincore;

impl From<super::Process> for jaeger::Process {
    fn from(process: super::Process) -> jaeger::Process {
        jaeger::Process::new(
            process.service_name,
            Some(process.tags.into_iter().map(Into::into).collect()),
        )
    }
}

impl From<Event> for jaeger::Log {
    fn from(event: crate::exporter::Event) -> jaeger::Log {
        let timestamp = event
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_micros() as i64;
        let mut event_set_via_attribute = false;
        let mut fields = event
            .attributes
            .into_iter()
            .map(|attr| {
                if attr.key.as_str() == "event" {
                    event_set_via_attribute = true;
                };
                attr.into()
            })
            .collect::<Vec<_>>();

        if !event_set_via_attribute {
            fields.push(Key::new("event").string(event.name).into());
        }

        if event.dropped_attributes_count != 0 {
            fields.push(
                Key::new("otel.event.dropped_attributes_count")
                    .i64(i64::from(event.dropped_attributes_count))
                    .into(),
            );
        }

        jaeger::Log::new(timestamp, fields)
    }
}

#[rustfmt::skip]
impl From<KeyValue> for jaeger::Tag {
    fn from(kv: KeyValue) -> jaeger::Tag {
        let KeyValue { key, value } = kv;
        match value {
            Value::String(s) => jaeger::Tag::new(key.into(), jaeger::TagType::String, Some(s.into()), None, None, None, None),
            Value::F64(f) => jaeger::Tag::new(key.into(), jaeger::TagType::Double, None, Some(f.into()), None, None, None),
            Value::Bool(b) => jaeger::Tag::new(key.into(), jaeger::TagType::Bool, None, None, Some(b), None, None),
            Value::I64(i) => jaeger::Tag::new(key.into(), jaeger::TagType::Long, None, None, None, Some(i), None),
            // TODO: better Array handling, jaeger thrift doesn't support arrays
            v @ Value::Array(_) => jaeger::Tag::new(key.into(), jaeger::TagType::String, Some(v.to_string()), None, None, None, None),
        }
    }
}
