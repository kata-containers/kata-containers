use crate::{
    sdk::export::{
        trace::{ExportResult, SpanData, SpanExporter},
        ExportError,
    },
    sdk::{
        trace::{Config, EvictedHashMap, EvictedQueue},
        InstrumentationLibrary,
    },
    trace::{Span, SpanContext, SpanId, SpanKind, StatusCode},
    KeyValue,
};
use async_trait::async_trait;
use std::fmt::{Display, Formatter};
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Debug)]
pub struct TestSpan(pub SpanContext);

impl Span for TestSpan {
    fn add_event_with_timestamp(
        &mut self,
        _name: String,
        _timestamp: std::time::SystemTime,
        _attributes: Vec<KeyValue>,
    ) {
    }
    fn span_context(&self) -> &SpanContext {
        &self.0
    }
    fn is_recording(&self) -> bool {
        false
    }
    fn set_attribute(&mut self, _attribute: KeyValue) {}
    fn set_status(&mut self, _code: StatusCode, _message: String) {}
    fn update_name(&mut self, _new_name: String) {}
    fn end_with_timestamp(&mut self, _timestamp: std::time::SystemTime) {}
}

pub fn new_test_export_span_data() -> SpanData {
    let config = Config::default();
    SpanData {
        span_context: SpanContext::empty_context(),
        parent_span_id: SpanId::from_u64(0),
        span_kind: SpanKind::Internal,
        name: "opentelemetry".into(),
        start_time: crate::time::now(),
        end_time: crate::time::now(),
        attributes: EvictedHashMap::new(config.span_limits.max_attributes_per_span, 0),
        events: EvictedQueue::new(config.span_limits.max_events_per_span),
        links: EvictedQueue::new(config.span_limits.max_links_per_span),
        status_code: StatusCode::Unset,
        status_message: "".into(),
        resource: config.resource,
        instrumentation_lib: InstrumentationLibrary::default(),
    }
}

#[derive(Debug)]
pub struct TestSpanExporter {
    tx_export: Sender<SpanData>,
    tx_shutdown: Sender<()>,
}

#[async_trait]
impl SpanExporter for TestSpanExporter {
    async fn export(&mut self, batch: Vec<SpanData>) -> ExportResult {
        for span_data in batch {
            self.tx_export
                .send(span_data)
                .map_err::<TestExportError, _>(Into::into)?;
        }
        Ok(())
    }

    fn shutdown(&mut self) {
        self.tx_shutdown.send(()).unwrap();
    }
}

pub fn new_test_exporter() -> (TestSpanExporter, Receiver<SpanData>, Receiver<()>) {
    let (tx_export, rx_export) = channel();
    let (tx_shutdown, rx_shutdown) = channel();
    let exporter = TestSpanExporter {
        tx_export,
        tx_shutdown,
    };
    (exporter, rx_export, rx_shutdown)
}

#[derive(Debug)]
pub struct TokioSpanExporter {
    tx_export: tokio::sync::mpsc::UnboundedSender<SpanData>,
    tx_shutdown: tokio::sync::mpsc::UnboundedSender<()>,
}

#[async_trait]
impl SpanExporter for TokioSpanExporter {
    async fn export(&mut self, batch: Vec<SpanData>) -> ExportResult {
        for span_data in batch {
            self.tx_export
                .send(span_data)
                .map_err::<TestExportError, _>(Into::into)?;
        }
        Ok(())
    }

    fn shutdown(&mut self) {
        self.tx_shutdown.send(()).unwrap();
    }
}

pub fn new_tokio_test_exporter() -> (
    TokioSpanExporter,
    tokio::sync::mpsc::UnboundedReceiver<SpanData>,
    tokio::sync::mpsc::UnboundedReceiver<()>,
) {
    let (tx_export, rx_export) = tokio::sync::mpsc::unbounded_channel();
    let (tx_shutdown, rx_shutdown) = tokio::sync::mpsc::unbounded_channel();
    let exporter = TokioSpanExporter {
        tx_export,
        tx_shutdown,
    };
    (exporter, rx_export, rx_shutdown)
}

#[derive(Debug)]
pub struct TestExportError(String);

impl std::error::Error for TestExportError {}

impl ExportError for TestExportError {
    fn exporter_name(&self) -> &'static str {
        "test"
    }
}

impl Display for TestExportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for TestExportError {
    fn from(err: tokio::sync::mpsc::error::SendError<T>) -> Self {
        TestExportError(err.to_string())
    }
}

impl<T> From<std::sync::mpsc::SendError<T>> for TestExportError {
    fn from(err: std::sync::mpsc::SendError<T>) -> Self {
        TestExportError(err.to_string())
    }
}
