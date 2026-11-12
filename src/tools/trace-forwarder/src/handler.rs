// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::span_data;
use anyhow::{anyhow, Context, Result};
use byteorder::{ByteOrder, NetworkEndian};
use opentelemetry::sdk::export::trace::SpanData as AgentSpanData;
use opentelemetry_sdk::trace::SpanProcessor;
use slog::{debug, info, o, Logger};
use std::io::{ErrorKind, Write};
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt};

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

pub async fn handle_connection(
    logger: Logger,
    mut conn: impl AsyncRead + Unpin,
    processor: Option<Arc<dyn SpanProcessor>>,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "handler"));

    debug!(logger, "handling connection");

    handle_trace_data(
        logger.clone(),
        &mut conn,
        processor.as_deref(),
        &mut std::io::stdout(),
    )
    .await
    .context("failed to handle trace data")?;
    if let Some(processor) = processor {
        tokio::task::spawn_blocking(move || processor.force_flush())
            .await
            .context("trace flush worker failed")?
            .context("failed to flush trace data")?;
    }

    debug!(&logger, "handled connection");

    Ok(())
}

async fn handle_trace_data<R: AsyncRead + Unpin + ?Sized>(
    logger: Logger,
    reader: &mut R,
    processor: Option<&dyn SpanProcessor>,
    output: &mut dyn Write,
) -> Result<()> {
    loop {
        let mut header: [u8; HEADER_SIZE_BYTES as usize] = [0; HEADER_SIZE_BYTES as usize];

        info!(logger, "waiting for traces");

        let read = match reader.read(&mut header).await {
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
            .await
            .context("failed to read complete trace header")?;
        debug!(logger, "read header");

        let payload_len = NetworkEndian::read_u64(&header);
        let mut encoded_payload = Vec::new();
        (&mut *reader)
            .take(payload_len)
            .read_to_end(&mut encoded_payload)
            .await
            .with_context(|| "failed to read payload")?;
        if encoded_payload.len() as u64 != payload_len {
            return Err(anyhow!(
                "incomplete trace payload: expected {payload_len} bytes, received {}",
                encoded_payload.len()
            ));
        }

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
        // Keep shutdown responsive when another complete record is already buffered.
        tokio::task::yield_now().await;
    }

    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use opentelemetry::sdk::trace::{EvictedHashMap, EvictedQueue};
    use opentelemetry::trace::{
        Event, Link, SpanContext, SpanId, SpanKind, StatusCode, TraceFlags, TraceId, TraceState,
    };
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::error::{OTelSdkError, OTelSdkResult};
    use opentelemetry_sdk::trace::{BatchSpanProcessor, SpanData, SpanExporter};
    use std::io::Cursor;
    use std::pin::Pin;
    use std::sync::Mutex;
    use std::task::{Context as TaskContext, Poll};
    use std::time::{Duration, UNIX_EPOCH};
    use tokio::io::ReadBuf;

    #[derive(Clone, Debug, Default)]
    pub(crate) struct RecordingExporter {
        pub(crate) spans: Arc<Mutex<Vec<SpanData>>>,
        pub(crate) failure: Option<&'static str>,
    }

    impl SpanExporter for RecordingExporter {
        async fn export(&self, spans: Vec<SpanData>) -> OTelSdkResult {
            if let Some(message) = self.failure {
                return Err(OTelSdkError::InternalFailure(message.to_owned()));
            }
            self.spans.lock().unwrap().extend(spans);
            Ok(())
        }
    }

    fn logger() -> Logger {
        Logger::root(slog::Discard, o!())
    }

    pub(crate) fn agent_span() -> AgentSpanData {
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

    pub(crate) fn packet(payload: &[u8]) -> Vec<u8> {
        let mut data = vec![0; HEADER_SIZE_BYTES as usize];
        NetworkEndian::write_u64(&mut data, payload.len() as u64);
        data.extend_from_slice(payload);
        data
    }

    #[tokio::test]
    async fn agent_json_is_exported_with_its_original_span_data() {
        let agent = agent_span();
        let payload = serde_json::to_vec(&agent).unwrap();
        let exporter = RecordingExporter::default();
        let processor = Arc::new(BatchSpanProcessor::builder(exporter.clone()).build());

        handle_connection(
            logger(),
            Cursor::new(packet(&payload)),
            Some(processor.clone()),
        )
        .await
        .unwrap();
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
        assert_eq!(
            span.events[0].attributes,
            vec![opentelemetry_otlp_api::KeyValue::new(
                "event-key",
                "event-value"
            )]
        );
        assert_eq!(span.links.len(), 1);
        assert_eq!(span.links.dropped_count, 1);
        assert_eq!(span.links[0].span_context, span.span_context);
        assert_eq!(
            span.links[0].attributes,
            vec![opentelemetry_otlp_api::KeyValue::new("linked", true)]
        );
        assert_eq!(
            span.status,
            opentelemetry_otlp_api::trace::Status::error("request failed")
        );
    }

    #[tokio::test]
    async fn dump_mode_writes_each_json_record_without_an_exporter() {
        let values = [
            serde_json::json!({"name": "first", "attributes": {"answer": 42}}),
            serde_json::json!({"name": "second"}),
        ];
        let input: Vec<_> = values
            .iter()
            .flat_map(|value| packet(&serde_json::to_vec_pretty(value).unwrap()))
            .collect();
        let mut output = Vec::new();
        handle_trace_data(logger(), &mut Cursor::new(input), None, &mut output)
            .await
            .unwrap();
        let text = String::from_utf8(output).unwrap();
        let decoded: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(decoded, values);
    }

    #[tokio::test]
    async fn review_dump_preserves_full_width_trace_ids() {
        let mut agent = agent_span();
        agent.span_context = SpanContext::new(
            TraceId::from_bytes(u128::MAX.to_be_bytes()),
            SpanId::from_bytes(u64::MAX.to_be_bytes()),
            TraceFlags::SAMPLED,
            false,
            TraceState::default(),
        );
        let payload = serde_json::to_vec(&agent).unwrap();
        let mut output = Vec::new();
        handle_trace_data(
            logger(),
            &mut Cursor::new(packet(&payload)),
            None,
            &mut output,
        )
        .await
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains(&u128::MAX.to_string()), "{}", text);
        assert!(text.contains(&u64::MAX.to_string()), "{}", text);
    }

    #[tokio::test]
    async fn review_payload_length_does_not_preallocate_the_declared_size() {
        struct ObservedReader {
            header: Cursor<Vec<u8>>,
            max_requested: usize,
        }

        impl AsyncRead for ObservedReader {
            fn poll_read(
                self: Pin<&mut Self>,
                cx: &mut TaskContext<'_>,
                buffer: &mut ReadBuf<'_>,
            ) -> Poll<std::io::Result<()>> {
                let reader = self.get_mut();
                reader.max_requested = reader.max_requested.max(buffer.remaining());
                Pin::new(&mut reader.header).poll_read(cx, buffer)
            }
        }

        let mut header = vec![0; HEADER_SIZE_BYTES as usize];
        NetworkEndian::write_u64(&mut header, 16 * 1024 * 1024);
        let mut reader = ObservedReader {
            header: Cursor::new(header),
            max_requested: 0,
        };
        assert!(
            handle_trace_data(logger(), &mut reader, None, &mut Vec::new())
                .await
                .is_err()
        );
        assert!(reader.max_requested <= 8192, "{}", reader.max_requested);
    }

    #[tokio::test]
    async fn every_agent_attribute_type_is_preserved_through_export() {
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
        let processor = Arc::new(BatchSpanProcessor::builder(exporter.clone()).build());
        let mut input = Vec::new();
        let mut expected = Vec::new();
        for (value, converted) in cases {
            let mut agent = agent_span();
            agent.attributes = EvictedHashMap::new(1, 0);
            agent.attributes.insert(KeyValue::new("value", value));
            input.extend(packet(&serde_json::to_vec(&agent).unwrap()));
            expected.push(converted);
        }

        handle_connection(logger(), Cursor::new(input), Some(processor.clone()))
            .await
            .unwrap();
        processor.shutdown().unwrap();
        let spans = exporter.spans.lock().unwrap();
        assert_eq!(spans.len(), expected.len());
        for (span, expected) in spans.iter().zip(expected) {
            assert_eq!(span.attributes[0].value, expected);
        }
    }

    #[tokio::test]
    async fn invalid_span_json_is_not_exported() {
        let exporter = RecordingExporter::default();
        let processor = Arc::new(BatchSpanProcessor::builder(exporter.clone()).build());
        assert!(handle_connection(
            logger(),
            Cursor::new(packet(b"{}")),
            Some(processor.clone())
        )
        .await
        .is_err());
        processor.shutdown().unwrap();
        assert!(exporter.spans.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn fragmented_records_are_reassembled_before_export() {
        struct FragmentedReader(Cursor<Vec<u8>>);
        impl AsyncRead for FragmentedReader {
            fn poll_read(
                self: Pin<&mut Self>,
                _cx: &mut TaskContext<'_>,
                buffer: &mut ReadBuf<'_>,
            ) -> Poll<std::io::Result<()>> {
                let mut chunk = [0; 3];
                let len = buffer.remaining().min(chunk.len());
                Poll::Ready(
                    std::io::Read::read(&mut self.get_mut().0, &mut chunk[..len])
                        .map(|read| buffer.put_slice(&chunk[..read])),
                )
            }
        }

        let payload = serde_json::to_vec(&agent_span()).unwrap();
        let exporter = RecordingExporter::default();
        let processor = Arc::new(BatchSpanProcessor::builder(exporter.clone()).build());
        handle_connection(
            logger(),
            FragmentedReader(Cursor::new(packet(&payload))),
            Some(processor.clone()),
        )
        .await
        .unwrap();
        processor.shutdown().unwrap();
        assert_eq!(exporter.spans.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn malformed_or_truncated_records_are_reported() {
        for input in [
            vec![0],
            vec![u8::MAX; HEADER_SIZE_BYTES as usize],
            packet(b"not JSON"),
            packet(b""),
            packet(b"{}")[..9].to_vec(),
        ] {
            assert!(
                handle_trace_data(logger(), &mut Cursor::new(input), None, &mut Vec::new(),)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn dump_write_errors_are_reported() {
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
        .await
        .unwrap_err();
        assert!(error.to_string().contains("failed to write trace JSON"));
    }

    #[tokio::test]
    async fn exporter_failures_are_reported_when_the_connection_is_flushed() {
        let exporter = RecordingExporter {
            failure: Some("export rejected"),
            ..Default::default()
        };
        let processor = Arc::new(BatchSpanProcessor::builder(exporter).build());
        let payload = serde_json::to_vec(&agent_span()).unwrap();
        let error = handle_connection(
            logger(),
            Cursor::new(packet(&payload)),
            Some(processor.clone()),
        )
        .await
        .unwrap_err();
        assert!(format!("{error:#}").contains("export rejected"));
        processor.shutdown().unwrap();
    }
}
