// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::span_data;
use anyhow::{Context, Result};
use byteorder::{ByteOrder, NetworkEndian};
use opentelemetry::sdk::export::trace::SpanData as AgentSpanData;
use opentelemetry_sdk::trace::SpanProcessor;
use slog::{debug, info, o, Logger};
use std::convert::TryFrom;
use std::io::{ErrorKind, Read, Write};

// The VSOCK "packet" protocol used comprises two elements:
//
// 1) The header (the number of bytes in the payload).
// 2) The payload bytes.
//
// This constant defines the number of bytes used to encode the header on the
// wire. In other words, the first 64-bits of the packet contain a number
// specifying how many bytes are in the remainder of the packet.
//
// Must match the value of the variable of the same name in the agents
// vsock-exporter.
const HEADER_SIZE_BYTES: u64 = std::mem::size_of::<u64>() as u64;

pub fn handle_connection(
    logger: Logger,
    mut conn: impl Read,
    processor: Option<&dyn SpanProcessor>,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "handler"));

    debug!(logger, "handling connection");

    handle_trace_data(logger.clone(), &mut conn, processor, &mut std::io::stdout())
        .context("failed to handle trace data")?;
    if let Some(processor) = processor {
        processor
            .force_flush()
            .context("failed to flush trace data")?;
    }

    debug!(&logger, "handled connection");

    Ok(())
}

fn handle_trace_data(
    logger: Logger,
    reader: &mut dyn Read,
    processor: Option<&dyn SpanProcessor>,
    output: &mut dyn Write,
) -> Result<()> {
    loop {
        let mut header: [u8; HEADER_SIZE_BYTES as usize] = [0; HEADER_SIZE_BYTES as usize];

        info!(logger, "waiting for traces");

        let read = match reader.read(&mut header) {
            Ok(0) => {
                info!(logger, "agent shut down");
                break;
            }
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).context("failed to read trace header"),
        };
        reader
            .read_exact(&mut header[read..])
            .context("failed to read complete trace header")?;
        debug!(logger, "read header");

        let payload_len = usize::try_from(NetworkEndian::read_u64(&header))
            .context("trace payload length does not fit in memory")?;
        let mut encoded_payload = Vec::new();
        encoded_payload
            .try_reserve_exact(payload_len)
            .context("failed to allocate trace payload")?;
        encoded_payload.resize(payload_len, 0);

        reader
            .read_exact(&mut encoded_payload)
            .with_context(|| "failed to read payload")?;

        debug!(logger, "read payload");

        if let Some(processor) = processor {
            let span: AgentSpanData =
                serde_json::from_slice(&encoded_payload).context("failed to decode agent span")?;
            processor.on_end(span_data::convert_span(span)?);
        } else {
            let value: serde_json::Value =
                serde_json::from_slice(&encoded_payload).context("failed to decode trace JSON")?;
            serde_json::to_writer(&mut *output, &value).context("failed to write trace JSON")?;
            output
                .write_all(b"\n")
                .context("failed to write trace newline")?;
            output.flush().context("failed to flush trace output")?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::sdk::trace::{EvictedHashMap, EvictedQueue};
    use opentelemetry::trace::{
        Event, Link, SpanContext, SpanId, SpanKind, StatusCode, TraceFlags, TraceId, TraceState,
    };
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
    use opentelemetry_sdk::trace::{BatchSpanProcessor, SpanData, SpanExporter};
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, UNIX_EPOCH};

    #[derive(Clone, Debug, Default)]
    struct RecordingExporter {
        spans: Arc<Mutex<Vec<SpanData>>>,
        fail: bool,
    }

    impl SpanExporter for RecordingExporter {
        async fn export(&self, spans: Vec<SpanData>) -> OTelSdkResult {
            if self.fail {
                return Err(OTelSdkError::InternalFailure("export rejected".to_owned()));
            }
            self.spans.lock().unwrap().extend(spans);
            Ok(())
        }
    }

    fn logger() -> Logger {
        Logger::root(slog::Discard, o!())
    }

    fn agent_span() -> AgentSpanData {
        let context = SpanContext::new(
            TraceId::from_bytes(123_u128.to_be_bytes()),
            SpanId::from_bytes(456_u64.to_be_bytes()),
            TraceFlags::SAMPLED,
            false,
            TraceState::default(),
        );
        let mut attributes = EvictedHashMap::new(1, 0);
        attributes.insert(KeyValue::new("discarded", "old"));
        attributes.insert(KeyValue::new("answer", 42_i64));
        let mut events = EvictedQueue::new(1);
        events.extend([
            Event::with_name("discarded"),
            Event::new(
                "event",
                UNIX_EPOCH + Duration::from_secs(11),
                vec![KeyValue::new("event-key", "event-value")],
                2,
            ),
        ]);
        let mut links = EvictedQueue::new(1);
        links.extend([
            Link::new(context.clone(), Vec::new()),
            Link::new(context.clone(), vec![KeyValue::new("linked", true)]),
        ]);
        AgentSpanData {
            span_context: context,
            parent_span_id: SpanId::from_bytes(789_u64.to_be_bytes()),
            span_kind: SpanKind::Server,
            name: "agent-span".into(),
            start_time: UNIX_EPOCH + Duration::from_secs(10),
            end_time: UNIX_EPOCH + Duration::from_secs(12),
            attributes,
            events,
            links,
            status_code: StatusCode::Error,
            status_message: "request failed".into(),
            resource: None,
            instrumentation_lib: Default::default(),
        }
    }

    fn packet(payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0; HEADER_SIZE_BYTES as usize];
        NetworkEndian::write_u64(&mut data, payload.len() as u64);
        data.extend_from_slice(payload);
        data
    }

    #[test]
    fn agent_json_is_exported_with_its_original_span_data() {
        let agent = agent_span();
        let payload = serde_json::to_vec(&agent).unwrap();
        let exporter = RecordingExporter::default();
        let processor = BatchSpanProcessor::builder(exporter.clone()).build();

        handle_connection(logger(), Cursor::new(packet(&payload)), Some(&processor)).unwrap();
        processor.shutdown().unwrap();

        let spans = exporter.spans.lock().unwrap();
        assert_eq!(spans.len(), 1);
        let span = &spans[0];
        assert_eq!(
            span.span_context.trace_id().to_bytes(),
            agent.span_context.trace_id().to_bytes()
        );
        assert_eq!(
            span.span_context.span_id().to_bytes(),
            agent.span_context.span_id().to_bytes()
        );
        assert_eq!(
            span.parent_span_id.to_bytes(),
            agent.parent_span_id.to_bytes()
        );
        assert_eq!(span.name, agent.name);
        assert_eq!(span.start_time, agent.start_time);
        assert_eq!(span.end_time, agent.end_time);
        assert_eq!(
            span.span_kind,
            opentelemetry_otlp_api::trace::SpanKind::Server
        );
        assert_eq!(
            span.attributes,
            vec![opentelemetry_otlp_api::KeyValue::new("answer", 42_i64)]
        );
        assert_eq!(span.dropped_attributes_count, 1);
        assert_eq!(span.events.len(), 1);
        assert_eq!(span.events.dropped_count, 1);
        assert_eq!(span.events[0].name, "event");
        assert_eq!(
            span.events[0].timestamp,
            UNIX_EPOCH + Duration::from_secs(11)
        );
        assert_eq!(span.events[0].dropped_attributes_count, 2);
        assert_eq!(span.links.len(), 1);
        assert_eq!(span.links.dropped_count, 1);
        assert_eq!(span.links[0].span_context, span.span_context);
        assert_eq!(
            span.status,
            opentelemetry_otlp_api::trace::Status::error("request failed")
        );
    }

    #[test]
    fn dump_mode_writes_each_json_record_without_an_exporter() {
        let values = [
            serde_json::json!({"name": "first", "attributes": {"answer": 42}}),
            serde_json::json!({"name": "second"}),
        ];
        let input: Vec<_> = values
            .iter()
            .flat_map(|value| packet(&serde_json::to_vec_pretty(value).unwrap()))
            .collect();
        let mut output = Vec::new();
        handle_trace_data(logger(), &mut Cursor::new(input), None, &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        let decoded: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(decoded, values);
    }

    #[test]
    fn every_agent_attribute_type_is_preserved_through_export() {
        use opentelemetry::{Array as AgentArray, Value as AgentValue};
        use opentelemetry_otlp_api::{Array, Value};

        let cases = [
            (AgentValue::Bool(true), Value::Bool(true)),
            (AgentValue::I64(42), Value::I64(42)),
            (AgentValue::F64(2.5), Value::F64(2.5)),
            (
                AgentValue::String("text".into()),
                Value::String("text".into()),
            ),
            (
                AgentValue::Array(AgentArray::Bool(vec![true, false])),
                Value::Array(Array::Bool(vec![true, false])),
            ),
            (
                AgentValue::Array(AgentArray::I64(vec![1, 2])),
                Value::Array(Array::I64(vec![1, 2])),
            ),
            (
                AgentValue::Array(AgentArray::F64(vec![1.5, 2.5])),
                Value::Array(Array::F64(vec![1.5, 2.5])),
            ),
            (
                AgentValue::Array(AgentArray::String(vec!["first".into(), "second".into()])),
                Value::Array(Array::String(vec!["first".into(), "second".into()])),
            ),
        ];
        let exporter = RecordingExporter::default();
        let processor = BatchSpanProcessor::builder(exporter.clone()).build();
        let mut input = Vec::new();
        let mut expected = Vec::new();
        for (value, converted) in cases {
            let mut agent = agent_span();
            agent.attributes = EvictedHashMap::new(1, 0);
            agent.attributes.insert(KeyValue::new("value", value));
            input.extend(packet(&serde_json::to_vec(&agent).unwrap()));
            expected.push(converted);
        }

        handle_connection(logger(), Cursor::new(input), Some(&processor)).unwrap();
        processor.shutdown().unwrap();
        let spans = exporter.spans.lock().unwrap();
        assert_eq!(spans.len(), expected.len());
        for (span, expected) in spans.iter().zip(expected) {
            assert_eq!(span.attributes[0].value, expected);
        }
    }

    #[test]
    fn invalid_span_json_is_not_exported() {
        let exporter = RecordingExporter::default();
        let processor = BatchSpanProcessor::builder(exporter.clone()).build();
        assert!(handle_connection(logger(), Cursor::new(packet(b"{}")), Some(&processor)).is_err());
        processor.shutdown().unwrap();
        assert!(exporter.spans.lock().unwrap().is_empty());
    }

    #[test]
    fn fragmented_records_are_reassembled_before_export() {
        struct FragmentedReader(Cursor<Vec<u8>>);
        impl Read for FragmentedReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let len = buffer.len().min(3);
                self.0.read(&mut buffer[..len])
            }
        }

        let payload = serde_json::to_vec(&agent_span()).unwrap();
        let exporter = RecordingExporter::default();
        let processor = BatchSpanProcessor::builder(exporter.clone()).build();
        handle_connection(
            logger(),
            FragmentedReader(Cursor::new(packet(&payload))),
            Some(&processor),
        )
        .unwrap();
        processor.shutdown().unwrap();
        assert_eq!(exporter.spans.lock().unwrap().len(), 1);
    }

    #[test]
    fn malformed_or_truncated_records_are_reported() {
        for input in [
            vec![0],
            vec![u8::MAX; HEADER_SIZE_BYTES as usize],
            packet(b"not JSON"),
            packet(b""),
            packet(b"{}")[..9].to_vec(),
        ] {
            assert!(
                handle_trace_data(logger(), &mut Cursor::new(input), None, &mut Vec::new(),)
                    .is_err()
            );
        }
    }

    #[test]
    fn dump_write_errors_are_reported() {
        struct RejectWrites;
        impl Write for RejectWrites {
            fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("write rejected"))
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let error = handle_trace_data(
            logger(),
            &mut Cursor::new(packet(b"{}")),
            None,
            &mut RejectWrites,
        )
        .unwrap_err();
        assert!(error.to_string().contains("failed to write trace JSON"));
    }

    #[test]
    fn exporter_failures_are_reported_when_the_connection_is_flushed() {
        let exporter = RecordingExporter {
            fail: true,
            ..Default::default()
        };
        let processor = BatchSpanProcessor::builder(exporter).build();
        let payload = serde_json::to_vec(&agent_span()).unwrap();
        let error = handle_connection(logger(), Cursor::new(packet(&payload)), Some(&processor))
            .unwrap_err();
        assert!(format!("{error:#}").contains("export rejected"));
        processor.shutdown().unwrap();
    }
}
