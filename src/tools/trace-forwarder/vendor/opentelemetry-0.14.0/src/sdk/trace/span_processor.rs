//! # OpenTelemetry Span Processor Interface
//!
//! Span processor is an interface which allows hooks for span start and end method
//! invocations. The span processors are invoked only when
//! [`is_recording`] is true.
//!
//! Built-in span processors are responsible for batching and conversion of spans to
//! exportable representation and passing batches to exporters.
//!
//! Span processors can be registered directly on SDK [`TracerProvider`] and they are
//! invoked in the same order as they were registered.
//!
//! All `Tracer` instances created by a `TracerProvider` share the same span processors.
//! Changes to this collection reflect in all `Tracer` instances.
//!
//! The following diagram shows `SpanProcessor`'s relationship to other components
//! in the SDK:
//!
//! ```ascii
//!   +-----+--------------+   +-----------------------+   +-------------------+
//!   |     |              |   |                       |   |                   |
//!   |     |              |   | (Batch)SpanProcessor  |   |    SpanExporter   |
//!   |     |              +---> (Simple)SpanProcessor +--->  (JaegerExporter) |
//!   |     |              |   |                       |   |                   |
//!   | SDK | Tracer.span()|   +-----------------------+   +-------------------+
//!   |     | Span.end()   |
//!   |     |              |   +---------------------+
//!   |     |              |   |                     |
//!   |     |              +---> ZPagesProcessor     |
//!   |     |              |   |                     |
//!   +-----+--------------+   +---------------------+
//! ```
//!
//! [`is_recording`]: crate::trace::Span::is_recording()
//! [`TracerProvider`]: crate::trace::TracerProvider

use crate::global;
use crate::runtime::Runtime;
use crate::sdk::trace::Span;
use crate::{
    sdk::export::trace::{ExportResult, SpanData, SpanExporter},
    trace::{TraceError, TraceResult},
    Context,
};
use futures::{channel::mpsc, channel::oneshot, executor, future::Either, pin_mut, StreamExt};
use std::{env, fmt, str::FromStr, sync::Mutex, thread, time::Duration};

/// Delay interval between two consecutive exports.
const OTEL_BSP_SCHEDULE_DELAY: &str = "OTEL_BSP_SCHEDULE_DELAY";
/// Default delay interval between two consecutive exports.
const OTEL_BSP_SCHEDULE_DELAY_DEFAULT: u64 = 5_000;
/// Maximum queue size
const OTEL_BSP_MAX_QUEUE_SIZE: &str = "OTEL_BSP_MAX_QUEUE_SIZE";
/// Default maximum queue size
const OTEL_BSP_MAX_QUEUE_SIZE_DEFAULT: usize = 2_048;
/// Maximum batch size, must be less than or equal to OTEL_BSP_MAX_QUEUE_SIZE
const OTEL_BSP_MAX_EXPORT_BATCH_SIZE: &str = "OTEL_BSP_MAX_EXPORT_BATCH_SIZE";
/// Default maximum batch size
const OTEL_BSP_MAX_EXPORT_BATCH_SIZE_DEFAULT: usize = 512;
/// Maximum allowed time to export data.
const OTEL_BSP_EXPORT_TIMEOUT: &str = "OTEL_BSP_EXPORT_TIMEOUT";
/// Default maximum allowed time to export data.
const OTEL_BSP_EXPORT_TIMEOUT_DEFAULT: u64 = 30_000;

/// `SpanProcessor` is an interface which allows hooks for span start and end
/// method invocations. The span processors are invoked only when is_recording
/// is true.
pub trait SpanProcessor: Send + Sync + std::fmt::Debug {
    /// `on_start` is called when a `Span` is started.  This method is called
    /// synchronously on the thread that started the span, therefore it should
    /// not block or throw exceptions.
    fn on_start(&self, span: &Span, cx: &Context);
    /// `on_end` is called after a `Span` is ended (i.e., the end timestamp is
    /// already set). This method is called synchronously within the `Span::end`
    /// API, therefore it should not block or throw an exception.
    fn on_end(&self, span: SpanData);
    /// Force the spans lying in the cache to be exported.
    fn force_flush(&self) -> TraceResult<()>;
    /// Shuts down the processor. Called when SDK is shut down. This is an
    /// opportunity for processors to do any cleanup required.
    fn shutdown(&mut self) -> TraceResult<()>;
}

/// A [`SpanProcessor`] that exports synchronously when spans are finished.
///
/// # Examples
///
/// Note that the simple processor exports synchronously every time a span is
/// ended. If you find this limiting, consider the batch processor instead.
///
/// ```
/// use opentelemetry::{trace as apitrace, sdk::trace as sdktrace, global};
///
/// // Configure your preferred exporter
/// let exporter = apitrace::NoopSpanExporter::new();
///
/// // Then use the `with_simple_exporter` method to have the provider export when spans finish.
/// let provider = sdktrace::TracerProvider::builder()
///     .with_simple_exporter(exporter)
///     .build();
///
/// let previous_provider = global::set_tracer_provider(provider);
/// ```
#[derive(Debug)]
pub struct SimpleSpanProcessor {
    sender: crossbeam_channel::Sender<Option<SpanData>>,
    shutdown: crossbeam_channel::Receiver<()>,
}

impl SimpleSpanProcessor {
    pub(crate) fn new(mut exporter: Box<dyn SpanExporter>) -> Self {
        let (span_tx, span_rx) = crossbeam_channel::unbounded();
        let (shutdown_tx, shutdown_rx) = crossbeam_channel::bounded(0);

        let _ = thread::Builder::new()
            .name("opentelemetry-exporter".to_string())
            .spawn(move || {
                while let Ok(Some(span)) = span_rx.recv() {
                    if let Err(err) = executor::block_on(exporter.export(vec![span])) {
                        global::handle_error(err);
                    }
                }

                exporter.shutdown();

                if let Err(err) = shutdown_tx.send(()) {
                    global::handle_error(TraceError::from(format!(
                        "could not send shutdown: {:?}",
                        err
                    )));
                }
            });

        SimpleSpanProcessor {
            sender: span_tx,
            shutdown: shutdown_rx,
        }
    }
}

impl SpanProcessor for SimpleSpanProcessor {
    fn on_start(&self, _span: &Span, _cx: &Context) {
        // Ignored
    }

    fn on_end(&self, span: SpanData) {
        if let Err(err) = self.sender.send(Some(span)) {
            global::handle_error(TraceError::from(format!("error processing span {:?}", err)));
        }
    }

    fn force_flush(&self) -> TraceResult<()> {
        // Ignored since all spans in Simple Processor will be exported as they ended.
        Ok(())
    }

    fn shutdown(&mut self) -> TraceResult<()> {
        if self.sender.send(None).is_ok() {
            if let Err(err) = self.shutdown.recv() {
                global::handle_error(TraceError::from(format!(
                    "error shutting down span processor: {:?}",
                    err
                )))
            }
        }

        Ok(())
    }
}

/// A [`SpanProcessor`] that asynchronously buffers finished spans and reports
/// them at a preconfigured interval.
///
/// # Examples
///
/// This processor can be configured with an [`executor`] of your choice to
/// batch and upload spans asynchronously when they end. If you have added a
/// library like [`tokio`] or [`async-std`], you can pass in their respective
/// `spawn` and `interval` functions to have batching performed in those
/// contexts.
///
/// ```
/// # #[cfg(feature="tokio")]
/// # {
/// use futures::{stream};
/// use opentelemetry::{trace as apitrace, sdk::trace as sdktrace, global, runtime};
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() {
///     // Configure your preferred exporter
///     let exporter = apitrace::NoopSpanExporter::new();
///
///     // Then build a batch processor. You can use whichever executor you have available, for
///     // example if you are using `async-std` instead of `tokio` you can replace the spawn and
///     // interval functions with `async_std::task::spawn` and `async_std::stream::interval`.
///     let batch = sdktrace::BatchSpanProcessor::builder(exporter, runtime::Tokio)
///         .with_max_queue_size(4096)
///         .build();
///
///     // Then use the `with_batch_exporter` method to have the provider export spans in batches.
///     let provider = sdktrace::TracerProvider::builder()
///         .with_span_processor(batch)
///         .build();
///
///     let _ = global::set_tracer_provider(provider);
/// }
/// # }
/// ```
///
/// [`executor`]: https://docs.rs/futures/0.3/futures/executor/index.html
/// [`tokio`]: https://tokio.rs
/// [`async-std`]: https://async.rs
pub struct BatchSpanProcessor {
    message_sender: Mutex<mpsc::Sender<BatchMessage>>,
}

impl fmt::Debug for BatchSpanProcessor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchSpanProcessor")
            .field("message_sender", &self.message_sender)
            .finish()
    }
}

impl SpanProcessor for BatchSpanProcessor {
    fn on_start(&self, _span: &Span, _cx: &Context) {
        // Ignored
    }

    fn on_end(&self, span: SpanData) {
        let result = self
            .message_sender
            .lock()
            .map_err(|_| TraceError::Other("batch span processor mutex poisoned".into()))
            .and_then(|mut sender| {
                sender
                    .try_send(BatchMessage::ExportSpan(span))
                    .map_err(|err| TraceError::Other(err.into()))
            });

        if let Err(err) = result {
            global::handle_error(err);
        }
    }

    fn force_flush(&self) -> TraceResult<()> {
        let mut sender = self.message_sender.lock().map_err(|_| TraceError::from("When force flushing the BatchSpanProcessor, the message sender's lock has been poisoned"))?;
        let (res_sender, res_receiver) = oneshot::channel();
        sender.try_send(BatchMessage::Flush(Some(res_sender)))?;

        futures::executor::block_on(res_receiver)
            .map_err(|err| TraceError::Other(err.into()))
            .and_then(|identity| identity)
    }

    fn shutdown(&mut self) -> TraceResult<()> {
        let mut sender = self.message_sender.lock().map_err(|_| TraceError::from("When shutting down the BatchSpanProcessor, the message sender's lock has been poisoned"))?;
        let (res_sender, res_receiver) = oneshot::channel();
        sender.try_send(BatchMessage::Shutdown(res_sender))?;

        futures::executor::block_on(res_receiver)
            .map_err(|err| TraceError::Other(err.into()))
            .and_then(|identity| identity)
    }
}

#[derive(Debug)]
enum BatchMessage {
    ExportSpan(SpanData),
    Flush(Option<oneshot::Sender<ExportResult>>),
    Shutdown(oneshot::Sender<ExportResult>),
}

impl BatchSpanProcessor {
    pub(crate) fn new<R>(
        mut exporter: Box<dyn SpanExporter>,
        config: BatchConfig,
        runtime: R,
    ) -> Self
    where
        R: Runtime,
    {
        let (message_sender, message_receiver) = mpsc::channel(config.max_queue_size);
        let ticker = runtime
            .interval(config.scheduled_delay)
            .map(|_| BatchMessage::Flush(None));
        let timeout_runtime = runtime.clone();

        // Spawn worker process via user-defined spawn function.
        runtime.spawn(Box::pin(async move {
            let mut spans = Vec::new();
            let mut messages = Box::pin(futures::stream::select(message_receiver, ticker));

            while let Some(message) = messages.next().await {
                match message {
                    // Span has finished, add to buffer of pending spans.
                    BatchMessage::ExportSpan(span) => {
                        spans.push(span);

                        if spans.len() == config.max_export_batch_size {
                            let result = export_with_timeout(
                                config.max_export_timeout,
                                exporter.as_mut(),
                                &timeout_runtime,
                                spans.split_off(0),
                            )
                            .await;

                            if let Err(err) = result {
                                global::handle_error(err);
                            }
                        }
                    }
                    // Span batch interval time reached or a force flush has been invoked, export current spans.
                    BatchMessage::Flush(res_channel) => {
                        let result = export_with_timeout(
                            config.max_export_timeout,
                            exporter.as_mut(),
                            &timeout_runtime,
                            spans.split_off(0),
                        )
                        .await;

                        if let Some(channel) = res_channel {
                            if let Err(err) = channel.send(result) {
                                global::handle_error(TraceError::from(format!(
                                    "failed to send flush result: {:?}",
                                    err
                                )));
                            }
                        } else if let Err(err) = result {
                            global::handle_error(TraceError::from(format!(
                                "failed to export batch {:?}",
                                err
                            )));
                        }
                    }
                    // Stream has terminated or processor is shutdown, return to finish execution.
                    BatchMessage::Shutdown(ch) => {
                        let result = export_with_timeout(
                            config.max_export_timeout,
                            exporter.as_mut(),
                            &timeout_runtime,
                            spans.split_off(0),
                        )
                        .await;

                        exporter.shutdown();

                        if let Err(err) = ch.send(result) {
                            global::handle_error(TraceError::from(format!(
                                "failed to send batch processor shutdown result: {:?}",
                                err
                            )));
                        }

                        break;
                    }
                }
            }
        }));

        // Return batch processor with link to worker
        BatchSpanProcessor {
            message_sender: Mutex::new(message_sender),
        }
    }

    /// Create a new batch processor builder
    pub fn builder<E, R>(exporter: E, runtime: R) -> BatchSpanProcessorBuilder<E, R>
    where
        E: SpanExporter,
        R: Runtime,
    {
        BatchSpanProcessorBuilder {
            exporter,
            config: BatchConfig::default(),
            runtime,
        }
    }
}

async fn export_with_timeout<R, E>(
    time_out: Duration,
    exporter: &mut E,
    runtime: &R,
    batch: Vec<SpanData>,
) -> ExportResult
where
    R: Runtime,
    E: SpanExporter + ?Sized,
{
    if batch.is_empty() {
        return Ok(());
    }

    let export = exporter.export(batch);
    let timeout = runtime.delay(time_out);
    pin_mut!(export);
    pin_mut!(timeout);
    match futures::future::select(export, timeout).await {
        Either::Left((export_res, _)) => export_res,
        Either::Right((_, _)) => ExportResult::Err(TraceError::ExportTimedOut(time_out)),
    }
}

/// Batch span processor configuration
#[derive(Debug)]
pub struct BatchConfig {
    /// The maximum queue size to buffer spans for delayed processing. If the
    /// queue gets full it drops the spans. The default value of is 2048.
    max_queue_size: usize,

    /// The delay interval in milliseconds between two consecutive processing
    /// of batches. The default value is 5 seconds.
    scheduled_delay: Duration,

    /// The maximum number of spans to process in a single batch. If there are
    /// more than one batch worth of spans then it processes multiple batches
    /// of spans one batch after the other without any delay. The default value
    /// is 512.
    max_export_batch_size: usize,

    /// The maximum duration to export a batch of data.
    max_export_timeout: Duration,
}

impl Default for BatchConfig {
    fn default() -> Self {
        let mut config = BatchConfig {
            max_queue_size: OTEL_BSP_MAX_QUEUE_SIZE_DEFAULT,
            scheduled_delay: Duration::from_millis(OTEL_BSP_SCHEDULE_DELAY_DEFAULT),
            max_export_batch_size: OTEL_BSP_MAX_EXPORT_BATCH_SIZE_DEFAULT,
            max_export_timeout: Duration::from_millis(OTEL_BSP_EXPORT_TIMEOUT_DEFAULT),
        };

        if let Some(max_queue_size) = env::var(OTEL_BSP_MAX_QUEUE_SIZE)
            .ok()
            .and_then(|queue_size| usize::from_str(&queue_size).ok())
        {
            config.max_queue_size = max_queue_size;
        }

        if let Some(scheduled_delay) = env::var(OTEL_BSP_SCHEDULE_DELAY)
            .ok()
            .or_else(|| env::var("OTEL_BSP_SCHEDULE_DELAY_MILLIS").ok())
            .and_then(|delay| u64::from_str(&delay).ok())
        {
            config.scheduled_delay = Duration::from_millis(scheduled_delay);
        }

        if let Some(max_export_batch_size) = env::var(OTEL_BSP_MAX_EXPORT_BATCH_SIZE)
            .ok()
            .and_then(|batch_size| usize::from_str(&batch_size).ok())
        {
            config.max_export_batch_size = max_export_batch_size;
        }

        // max export batch size must be less or equal to max queue size.
        // we set max export batch size to max queue size if it's larger than max queue size.
        if config.max_export_batch_size > config.max_queue_size {
            config.max_export_batch_size = config.max_queue_size;
        }

        if let Some(max_export_timeout) = env::var(OTEL_BSP_EXPORT_TIMEOUT)
            .ok()
            .or_else(|| env::var("OTEL_BSP_EXPORT_TIMEOUT_MILLIS").ok())
            .and_then(|timeout| u64::from_str(&timeout).ok())
        {
            config.max_export_timeout = Duration::from_millis(max_export_timeout);
        }

        config
    }
}

/// A builder for creating [`BatchSpanProcessor`] instances.
///
#[derive(Debug)]
pub struct BatchSpanProcessorBuilder<E, R> {
    exporter: E,
    config: BatchConfig,
    runtime: R,
}

impl<E, R> BatchSpanProcessorBuilder<E, R>
where
    E: SpanExporter + 'static,
    R: Runtime,
{
    /// Set max queue size for batches
    pub fn with_max_queue_size(self, size: usize) -> Self {
        let mut config = self.config;
        config.max_queue_size = size;

        BatchSpanProcessorBuilder { config, ..self }
    }

    /// Set scheduled delay for batches
    pub fn with_scheduled_delay(self, delay: Duration) -> Self {
        let mut config = self.config;
        config.scheduled_delay = delay;

        BatchSpanProcessorBuilder { config, ..self }
    }

    /// Set max timeout for exporting.
    pub fn with_max_timeout(self, timeout: Duration) -> Self {
        let mut config = self.config;
        config.max_export_timeout = timeout;

        BatchSpanProcessorBuilder { config, ..self }
    }

    /// Set max export size for batches, should always less than or equals to max queue size.
    ///
    /// If input is larger than max queue size, will lower it to be equal to max queue size
    pub fn with_max_export_batch_size(self, size: usize) -> Self {
        let mut config = self.config;
        if size > config.max_queue_size {
            config.max_export_batch_size = config.max_queue_size;
        } else {
            config.max_export_batch_size = size;
        }

        BatchSpanProcessorBuilder { config, ..self }
    }

    /// Build a batch processor
    pub fn build(self) -> BatchSpanProcessor {
        BatchSpanProcessor::new(Box::new(self.exporter), self.config, self.runtime)
    }
}

#[cfg(all(test, feature = "testing", feature = "trace"))]
mod tests {
    use super::{
        BatchSpanProcessor, SimpleSpanProcessor, SpanProcessor, OTEL_BSP_EXPORT_TIMEOUT,
        OTEL_BSP_MAX_EXPORT_BATCH_SIZE, OTEL_BSP_MAX_QUEUE_SIZE, OTEL_BSP_MAX_QUEUE_SIZE_DEFAULT,
        OTEL_BSP_SCHEDULE_DELAY, OTEL_BSP_SCHEDULE_DELAY_DEFAULT,
    };
    use crate::runtime;
    use crate::sdk::export::trace::{stdout, ExportResult, SpanData, SpanExporter};
    use crate::sdk::trace::BatchConfig;
    use crate::testing::trace::{
        new_test_export_span_data, new_test_exporter, new_tokio_test_exporter,
    };
    use async_trait::async_trait;
    use futures::Future;
    use std::fmt::Debug;
    use std::time::Duration;

    #[test]
    fn simple_span_processor_on_end_calls_export() {
        let (exporter, rx_export, _rx_shutdown) = new_test_exporter();
        let mut processor = SimpleSpanProcessor::new(Box::new(exporter));
        processor.on_end(new_test_export_span_data());
        assert!(rx_export.recv().is_ok());
        let _result = processor.shutdown();
    }

    #[test]
    fn simple_span_processor_shutdown_calls_shutdown() {
        let (exporter, _rx_export, rx_shutdown) = new_test_exporter();
        let mut processor = SimpleSpanProcessor::new(Box::new(exporter));
        let _result = processor.shutdown();
        assert!(rx_shutdown.try_recv().is_ok());
    }

    #[test]
    fn test_build_batch_span_processor_builder() {
        std::env::set_var(OTEL_BSP_MAX_EXPORT_BATCH_SIZE, "500");
        std::env::set_var(OTEL_BSP_EXPORT_TIMEOUT, "2046");
        std::env::set_var(OTEL_BSP_SCHEDULE_DELAY, "I am not number");

        let mut builder = BatchSpanProcessor::builder(
            stdout::Exporter::new(std::io::stdout(), true),
            runtime::Tokio,
        );
        // export batch size cannot exceed max queue size
        assert_eq!(builder.config.max_export_batch_size, 500);
        assert_eq!(
            builder.config.scheduled_delay,
            Duration::from_millis(OTEL_BSP_SCHEDULE_DELAY_DEFAULT)
        );
        assert_eq!(
            builder.config.max_queue_size,
            OTEL_BSP_MAX_QUEUE_SIZE_DEFAULT
        );
        assert_eq!(
            builder.config.max_export_timeout,
            Duration::from_millis(2046)
        );

        std::env::set_var(OTEL_BSP_MAX_QUEUE_SIZE, "120");
        builder = BatchSpanProcessor::builder(
            stdout::Exporter::new(std::io::stdout(), true),
            runtime::Tokio,
        );

        assert_eq!(builder.config.max_export_batch_size, 120);
        assert_eq!(builder.config.max_queue_size, 120);
    }

    #[tokio::test]
    async fn test_batch_span_processor() {
        let (exporter, mut export_receiver, _shutdown_receiver) = new_tokio_test_exporter();
        let config = BatchConfig {
            scheduled_delay: Duration::from_secs(60 * 60 * 24), // set the tick to 24 hours so we know the span must be exported via force_flush
            ..Default::default()
        };
        let mut processor =
            BatchSpanProcessor::new(Box::new(exporter), config, runtime::TokioCurrentThread);
        let handle = tokio::spawn(async move {
            loop {
                if let Some(span) = export_receiver.recv().await {
                    assert_eq!(span.span_context, new_test_export_span_data().span_context);
                    break;
                }
            }
        });
        tokio::time::sleep(Duration::from_secs(1)).await; // skip the first
        processor.on_end(new_test_export_span_data());
        let flush_res = processor.force_flush();
        assert!(flush_res.is_ok());
        let _shutdown_result = processor.shutdown();

        assert!(
            tokio::time::timeout(Duration::from_secs(5), handle)
                .await
                .is_ok(),
            "timed out in 5 seconds. force_flush may not export any data when called"
        );
    }

    struct BlockingExporter<D> {
        delay_for: Duration,
        delay_fn: D,
    }

    impl<D, DS> Debug for BlockingExporter<D>
    where
        D: Fn(Duration) -> DS + 'static + Send + Sync,
        DS: Future<Output = ()> + Send + Sync + 'static,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("blocking exporter for testing")
        }
    }

    #[async_trait]
    impl<D, DS> SpanExporter for BlockingExporter<D>
    where
        D: Fn(Duration) -> DS + 'static + Send + Sync,
        DS: Future<Output = ()> + Send + Sync + 'static,
    {
        async fn export(&mut self, _batch: Vec<SpanData>) -> ExportResult {
            (self.delay_fn)(self.delay_for).await;
            Ok(())
        }
    }

    #[test]
    fn test_timeout_tokio_timeout() {
        // If time_out is true, then we ask exporter to block for 60s and set timeout to 5s.
        // If time_out is false, then we ask the exporter to block for 5s and set timeout to 60s.
        // Either way, the test should be finished within 5s.
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(timeout_test_tokio(true));
    }

    #[test]
    fn test_timeout_tokio_not_timeout() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(timeout_test_tokio(false));
    }

    #[test]
    #[cfg(feature = "rt-async-std")]
    fn test_timeout_async_std_timeout() {
        async_std::task::block_on(timeout_test_std_async(true));
    }

    #[test]
    #[cfg(feature = "rt-async-std")]
    fn test_timeout_async_std_not_timeout() {
        async_std::task::block_on(timeout_test_std_async(false));
    }

    // If the time_out is true, then the result suppose to ended with timeout.
    // otherwise the exporter should be able to export within time out duration.
    #[cfg(feature = "rt-async-std")]
    async fn timeout_test_std_async(time_out: bool) {
        let config = BatchConfig {
            max_export_timeout: Duration::from_millis(if time_out { 5 } else { 60 }),
            scheduled_delay: Duration::from_secs(60 * 60 * 24), // set the tick to 24 hours so we know the span must be exported via force_flush
            ..Default::default()
        };
        let exporter = BlockingExporter {
            delay_for: Duration::from_millis(if !time_out { 5 } else { 60 }),
            delay_fn: async_std::task::sleep,
        };
        let mut processor = BatchSpanProcessor::new(Box::new(exporter), config, runtime::AsyncStd);
        processor.on_end(new_test_export_span_data());
        let flush_res = processor.force_flush();
        if time_out {
            assert!(flush_res.is_err());
        } else {
            assert!(flush_res.is_ok());
        }
        let shutdown_res = processor.shutdown();
        assert!(shutdown_res.is_ok());
    }

    // If the time_out is true, then the result suppose to ended with timeout.
    // otherwise the exporter should be able to export within time out duration.
    async fn timeout_test_tokio(time_out: bool) {
        let config = BatchConfig {
            max_export_timeout: Duration::from_millis(if time_out { 5 } else { 60 }),
            scheduled_delay: Duration::from_secs(60 * 60 * 24), // set the tick to 24 hours so we know the span must be exported via force_flush,
            ..Default::default()
        };
        let exporter = BlockingExporter {
            delay_for: Duration::from_millis(if !time_out { 5 } else { 60 }),
            delay_fn: tokio::time::sleep,
        };
        let mut processor =
            BatchSpanProcessor::new(Box::new(exporter), config, runtime::TokioCurrentThread);
        tokio::time::sleep(Duration::from_secs(1)).await; // skip the first
        processor.on_end(new_test_export_span_data());
        let flush_res = processor.force_flush();
        if time_out {
            assert!(flush_res.is_err());
        } else {
            assert!(flush_res.is_ok());
        }
        let shutdown_res = processor.shutdown();
        assert!(shutdown_res.is_ok());
    }
}
