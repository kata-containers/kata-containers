//! # Stdout Span Exporter
//!
//! The stdout [`SpanExporter`] writes debug printed [`Span`]s to its configured
//! [`Write`] instance. By default it will write to [`Stdout`].
//!
//! [`SpanExporter`]: super::SpanExporter
//! [`Span`]: crate::trace::Span
//! [`Write`]: std::io::Write
//! [`Stdout`]: std::io::Stdout
//!
//! # Examples
//!
//! ```no_run
//! use opentelemetry::trace::Tracer;
//! use opentelemetry::sdk::export::trace::stdout;
//! use opentelemetry::global::shutdown_tracer_provider;
//!
//! fn main() {
//!     let tracer = stdout::new_pipeline()
//!         .with_pretty_print(true)
//!         .install_simple();
//!
//!     tracer.in_span("doing_work", |cx| {
//!         // Traced app logic here...
//!     });
//!
//!     shutdown_tracer_provider(); // sending remaining spans
//! }
//! ```
use crate::{
    global, sdk,
    sdk::export::{
        trace::{ExportResult, SpanData, SpanExporter},
        ExportError,
    },
    trace::TracerProvider,
};
use async_trait::async_trait;
use std::fmt::Debug;
use std::io::{stdout, Stdout, Write};

/// Pipeline builder
#[derive(Debug)]
pub struct PipelineBuilder<W: Write> {
    pretty_print: bool,
    trace_config: Option<sdk::trace::Config>,
    writer: W,
}

/// Create a new stdout exporter pipeline builder.
pub fn new_pipeline() -> PipelineBuilder<Stdout> {
    PipelineBuilder::default()
}

impl Default for PipelineBuilder<Stdout> {
    /// Return the default pipeline builder.
    fn default() -> Self {
        Self {
            pretty_print: false,
            trace_config: None,
            writer: stdout(),
        }
    }
}

impl<W: Write> PipelineBuilder<W> {
    /// Specify the pretty print setting.
    pub fn with_pretty_print(mut self, pretty_print: bool) -> Self {
        self.pretty_print = pretty_print;
        self
    }

    /// Assign the SDK trace configuration.
    pub fn with_trace_config(mut self, config: sdk::trace::Config) -> Self {
        self.trace_config = Some(config);
        self
    }

    /// Specify the writer to use.
    pub fn with_writer<T: Write>(self, writer: T) -> PipelineBuilder<T> {
        PipelineBuilder {
            pretty_print: self.pretty_print,
            trace_config: self.trace_config,
            writer,
        }
    }
}

impl<W> PipelineBuilder<W>
where
    W: Write + Debug + Send + 'static,
{
    /// Install the stdout exporter pipeline with the recommended defaults.
    pub fn install_simple(mut self) -> sdk::trace::Tracer {
        let exporter = Exporter::new(self.writer, self.pretty_print);

        let mut provider_builder =
            sdk::trace::TracerProvider::builder().with_simple_exporter(exporter);
        if let Some(config) = self.trace_config.take() {
            provider_builder = provider_builder.with_config(config);
        }
        let provider = provider_builder.build();
        let tracer = provider.get_tracer("opentelemetry", Some(env!("CARGO_PKG_VERSION")));
        let _ = global::set_tracer_provider(provider);

        tracer
    }
}

/// A [`SpanExporter`] that writes to [`Stdout`] or other configured [`Write`].
///
/// [`SpanExporter`]: super::SpanExporter
/// [`Write`]: std::io::Write
/// [`Stdout`]: std::io::Stdout
#[derive(Debug)]
pub struct Exporter<W: Write> {
    writer: W,
    pretty_print: bool,
}

impl<W: Write> Exporter<W> {
    /// Create a new stdout `Exporter`.
    pub fn new(writer: W, pretty_print: bool) -> Self {
        Self {
            writer,
            pretty_print,
        }
    }
}

#[async_trait]
impl<W> SpanExporter for Exporter<W>
where
    W: Write + Debug + Send + 'static,
{
    /// Export spans to stdout
    async fn export(&mut self, batch: Vec<SpanData>) -> ExportResult {
        for span in batch {
            if self.pretty_print {
                self.writer
                    .write_all(format!("{:#?}\n", span).as_bytes())
                    .map_err::<Error, _>(Into::into)?;
            } else {
                self.writer
                    .write_all(format!("{:?}\n", span).as_bytes())
                    .map_err::<Error, _>(Into::into)?;
            }
        }

        Ok(())
    }
}

/// Stdout exporter's error
#[derive(thiserror::Error, Debug)]
#[error(transparent)]
struct Error(#[from] std::io::Error);

impl ExportError for Error {
    fn exporter_name(&self) -> &'static str {
        "stdout"
    }
}
