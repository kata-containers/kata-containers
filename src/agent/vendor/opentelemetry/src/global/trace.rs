use crate::global::handle_error;
use crate::trace::{NoopTracerProvider, TraceResult};
use crate::{trace, trace::TracerProvider, Context, KeyValue};
use std::borrow::Cow;
use std::fmt;
use std::mem;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

/// Wraps the [`BoxedTracer`]'s [`Span`] so it can be used generically by
/// applications without knowing the underlying type.
///
/// [`Span`]: crate::trace::Span
#[derive(Debug)]
pub struct BoxedSpan(Box<DynSpan>);

type DynSpan = dyn trace::Span + Send + Sync;

impl trace::Span for BoxedSpan {
    /// Records events at a specific time in the context of a given `Span`.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard event names and
    /// keys"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// which have prescribed semantic meanings.
    fn add_event_with_timestamp(
        &mut self,
        name: String,
        timestamp: SystemTime,
        attributes: Vec<KeyValue>,
    ) {
        self.0.add_event_with_timestamp(name, timestamp, attributes)
    }

    /// Returns the `SpanContext` for the given `Span`.
    fn span_context(&self) -> &trace::SpanContext {
        self.0.span_context()
    }

    /// Returns true if this `Span` is recording information like events with the `add_event`
    /// operation, attributes using `set_attributes`, status with `set_status`, etc.
    fn is_recording(&self) -> bool {
        self.0.is_recording()
    }

    /// Sets a single `Attribute` where the attribute properties are passed as arguments.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard
    /// attributes"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// that have prescribed semantic meanings.
    fn set_attribute(&mut self, attribute: KeyValue) {
        self.0.set_attribute(attribute)
    }

    /// Sets the status of the `Span`. If used, this will override the default `Span`
    /// status, which is `Unset`.
    fn set_status(&mut self, code: trace::StatusCode, message: String) {
        self.0.set_status(code, message)
    }

    /// Updates the `Span`'s name.
    fn update_name(&mut self, new_name: String) {
        self.0.update_name(new_name)
    }

    /// Finishes the span with given timestamp.
    fn end_with_timestamp(&mut self, timestamp: SystemTime) {
        self.0.end_with_timestamp(timestamp);
    }
}

/// Wraps the [`GlobalTracerProvider`]'s [`Tracer`] so it can be used generically by
/// applications without knowing the underlying type.
///
/// [`Tracer`]: crate::trace::Tracer
/// [`GlobalTracerProvider`]: crate::global::GlobalTracerProvider
#[derive(Debug)]
pub struct BoxedTracer(Box<dyn GenericTracer + Send + Sync>);

impl trace::Tracer for BoxedTracer {
    /// Global tracer uses `BoxedSpan`s so that it can be a global singleton,
    /// which is not possible if it takes generic type parameters.
    type Span = BoxedSpan;

    /// Returns a span with an inactive `SpanContext`. Used by functions that
    /// need to return a default span like `get_active_span` if no span is present.
    fn invalid(&self) -> Self::Span {
        BoxedSpan(self.0.invalid_boxed())
    }

    /// Starts a new `Span`.
    ///
    /// Each span has zero or one parent spans and zero or more child spans, which
    /// represent causally related operations. A tree of related spans comprises a
    /// trace. A span is said to be a _root span_ if it does not have a parent. Each
    /// trace includes a single root span, which is the shared ancestor of all other
    /// spans in the trace.
    fn start_with_context<T>(&self, name: T, cx: Context) -> Self::Span
    where
        T: Into<Cow<'static, str>>,
    {
        BoxedSpan(self.0.start_with_context_boxed(name.into(), cx))
    }

    /// Creates a span builder
    ///
    /// An ergonomic way for attributes to be configured before the `Span` is started.
    fn span_builder<T>(&self, name: T) -> trace::SpanBuilder
    where
        T: Into<Cow<'static, str>>,
    {
        trace::SpanBuilder::from_name(name)
    }

    /// Create a span from a `SpanBuilder`
    fn build(&self, builder: trace::SpanBuilder) -> Self::Span {
        BoxedSpan(self.0.build_boxed(builder))
    }
}

/// Allows a specific [`Tracer`] to be used generically by [`BoxedTracer`]
/// instances by mirroring the interface and boxing the return types.
///
/// [`Tracer`]: crate::trace::Tracer
pub trait GenericTracer: fmt::Debug + 'static {
    /// Create a new invalid span for use in cases where there are no active spans.
    fn invalid_boxed(&self) -> Box<DynSpan>;

    /// Returns a trait object so the underlying implementation can be swapped
    /// out at runtime.
    fn start_with_context_boxed(&self, name: Cow<'static, str>, cx: Context) -> Box<DynSpan>;

    /// Returns a trait object so the underlying implementation can be swapped
    /// out at runtime.
    fn build_boxed(&self, builder: trace::SpanBuilder) -> Box<DynSpan>;
}

impl<S, T> GenericTracer for T
where
    S: trace::Span + Send + Sync + 'static,
    T: trace::Tracer<Span = S>,
{
    /// Create a new invalid span for use in cases where there are no active spans.
    fn invalid_boxed(&self) -> Box<DynSpan> {
        Box::new(self.invalid())
    }

    /// Returns a trait object so the underlying implementation can be swapped
    /// out at runtime.
    fn start_with_context_boxed(&self, name: Cow<'static, str>, cx: Context) -> Box<DynSpan> {
        Box::new(self.start_with_context(name, cx))
    }

    /// Returns a trait object so the underlying implementation can be swapped
    /// out at runtime.
    fn build_boxed(&self, builder: trace::SpanBuilder) -> Box<DynSpan> {
        Box::new(self.build(builder))
    }
}

/// Allows a specific [`TracerProvider`] to be used generically by the
/// [`GlobalTracerProvider`] by mirroring the interface and boxing the return types.
///
/// [`TracerProvider`]: crate::trace::TracerProvider
/// [`GlobalTracerProvider`]: crate::global::GlobalTracerProvider
pub trait GenericTracerProvider: fmt::Debug + 'static {
    /// Creates a named tracer instance that is a trait object through the underlying `TracerProvider`.
    fn get_tracer_boxed(
        &self,
        name: &'static str,
        version: Option<&'static str>,
    ) -> Box<dyn GenericTracer + Send + Sync>;

    /// Force flush all remaining spans in span processors and return results.
    fn force_flush(&self) -> Vec<TraceResult<()>>;
}

impl<S, T, P> GenericTracerProvider for P
where
    S: trace::Span + Send + Sync + 'static,
    T: trace::Tracer<Span = S> + Send + Sync,
    P: trace::TracerProvider<Tracer = T>,
{
    /// Return a boxed generic tracer
    fn get_tracer_boxed(
        &self,
        name: &'static str,
        version: Option<&'static str>,
    ) -> Box<dyn GenericTracer + Send + Sync> {
        Box::new(self.get_tracer(name, version))
    }

    fn force_flush(&self) -> Vec<TraceResult<()>> {
        self.force_flush()
    }
}

/// Represents the globally configured [`TracerProvider`] instance for this
/// application. This allows generic tracing through the returned
/// [`BoxedTracer`] instances.
///
/// [`TracerProvider`]: crate::trace::TracerProvider
#[derive(Clone, Debug)]
pub struct GlobalTracerProvider {
    provider: Arc<dyn GenericTracerProvider + Send + Sync>,
}

impl GlobalTracerProvider {
    /// Create a new GlobalTracerProvider instance from a struct that implements `TracerProvider`.
    fn new<P, T, S>(provider: P) -> Self
    where
        S: trace::Span + Send + Sync + 'static,
        T: trace::Tracer<Span = S> + Send + Sync,
        P: trace::TracerProvider<Tracer = T> + Send + Sync,
    {
        GlobalTracerProvider {
            provider: Arc::new(provider),
        }
    }
}

impl trace::TracerProvider for GlobalTracerProvider {
    type Tracer = BoxedTracer;

    /// Find or create a named tracer using the global provider.
    fn get_tracer(&self, name: &'static str, version: Option<&'static str>) -> Self::Tracer {
        BoxedTracer(self.provider.get_tracer_boxed(name, version))
    }

    /// Force flush all remaining spans in span processors and return results.
    fn force_flush(&self) -> Vec<TraceResult<()>> {
        self.provider.force_flush()
    }
}

lazy_static::lazy_static! {
    /// The global `Tracer` provider singleton.
    static ref GLOBAL_TRACER_PROVIDER: RwLock<GlobalTracerProvider> = RwLock::new(GlobalTracerProvider::new(trace::NoopTracerProvider::new()));
}

/// Returns an instance of the currently configured global [`TracerProvider`] through
/// [`GlobalTracerProvider`].
///
/// [`TracerProvider`]: crate::trace::TracerProvider
/// [`GlobalTracerProvider`]: crate::global::GlobalTracerProvider
pub fn tracer_provider() -> GlobalTracerProvider {
    GLOBAL_TRACER_PROVIDER
        .read()
        .expect("GLOBAL_TRACER_PROVIDER RwLock poisoned")
        .clone()
}

/// Creates a named instance of [`Tracer`] via the configured [`GlobalTracerProvider`].
///
/// If the name is an empty string, the provider will use a default name.
///
/// This is a more convenient way of expressing `global::tracer_provider().get_tracer(name, None)`.
///
/// [`Tracer`]: crate::trace::Tracer
pub fn tracer(name: &'static str) -> BoxedTracer {
    tracer_provider().get_tracer(name, None)
}

/// Creates a named instance of [`Tracer`] with version info via the configured [`GlobalTracerProvider`]
///
/// If the name is an empty string, the provider will use a default name.
/// If the version is an empty string, it will be used as part of instrumentation library information.
///
/// [`Tracer`]: crate::trace::Tracer
pub fn tracer_with_version(name: &'static str, version: &'static str) -> BoxedTracer {
    tracer_provider().get_tracer(name, Some(version))
}

/// Sets the given [`TracerProvider`] instance as the current global provider.
///
/// It returns the [`TracerProvider`] instance that was previously mounted as global provider
/// (e.g. [`NoopTracerProvider`] if a provider had not been set before).
///
/// [`TracerProvider`]: crate::trace::TracerProvider
pub fn set_tracer_provider<P, T, S>(new_provider: P) -> GlobalTracerProvider
where
    S: trace::Span + Send + Sync + 'static,
    T: trace::Tracer<Span = S> + Send + Sync,
    P: trace::TracerProvider<Tracer = T> + Send + Sync,
{
    let mut tracer_provider = GLOBAL_TRACER_PROVIDER
        .write()
        .expect("GLOBAL_TRACER_PROVIDER RwLock poisoned");
    mem::replace(
        &mut *tracer_provider,
        GlobalTracerProvider::new(new_provider),
    )
}

/// Shut down the current tracer provider. This will invoke the shutdown method on all span processors.
/// span processors should export remaining spans before return
pub fn shutdown_tracer_provider() {
    let mut tracer_provider = GLOBAL_TRACER_PROVIDER
        .write()
        .expect("GLOBAL_TRACER_PROVIDER RwLock poisoned");

    let _ = mem::replace(
        &mut *tracer_provider,
        GlobalTracerProvider::new(NoopTracerProvider::new()),
    );
}

/// Force flush all remaining spans in span processors.
///
/// Use the [`global::handle_error`] to handle errors happened during force flush.
///
/// [`global::handle_error`]: crate::global::handle_error
pub fn force_flush_tracer_provider() {
    let tracer_provider = GLOBAL_TRACER_PROVIDER
        .write()
        .expect("GLOBAL_TRACER_PROVIDER RwLock poisoned");

    let results = trace::TracerProvider::force_flush(&*tracer_provider);
    for result in results {
        if let Err(err) = result {
            handle_error(err)
        }
    }
}

#[cfg(test)]
// Note that all tests here should be marked as ignore so that it won't be picked up by default We
// need to run those tests one by one as the GlobalTracerProvider is a shared object between
// threads Use cargo test -- --ignored --test-threads=1 to run those tests.
mod tests {
    use super::*;
    use crate::{
        runtime::{self, Runtime},
        trace::{NoopTracer, Tracer},
    };
    use std::{
        fmt::Debug,
        io::Write,
        sync::Mutex,
        thread::{self, sleep},
        time::Duration,
    };

    #[derive(Debug)]
    struct AssertWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl AssertWriter {
        fn new() -> AssertWriter {
            AssertWriter {
                buf: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn len(&self) -> usize {
            self.buf
                .lock()
                .expect("cannot acquire the lock of assert writer")
                .len()
        }
    }

    impl Write for AssertWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut buffer = self
                .buf
                .lock()
                .expect("cannot acquire the lock of assert writer");
            buffer.write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            let mut buffer = self
                .buf
                .lock()
                .expect("cannot acquire the lock of assert writer");
            buffer.flush()
        }
    }

    impl Clone for AssertWriter {
        fn clone(&self) -> Self {
            AssertWriter {
                buf: self.buf.clone(),
            }
        }
    }

    #[derive(Debug)]
    struct TestTracerProvider {
        debug_msg: &'static str,
    }

    impl Default for TestTracerProvider {
        fn default() -> Self {
            TestTracerProvider { debug_msg: "" }
        }
    }

    impl TestTracerProvider {
        fn new(debug_msg: &'static str) -> Self {
            TestTracerProvider { debug_msg }
        }
    }

    impl TracerProvider for TestTracerProvider {
        type Tracer = NoopTracer;

        fn get_tracer(&self, _name: &'static str, _version: Option<&'static str>) -> Self::Tracer {
            NoopTracer::default()
        }

        fn force_flush(&self) -> Vec<TraceResult<()>> {
            Vec::new()
        }
    }

    #[test]
    #[ignore = "requires --test-threads=1"]
    fn test_set_tracer_provider() {
        let _ = set_tracer_provider(TestTracerProvider::new("global one"));

        {
            let _ = set_tracer_provider(TestTracerProvider::new("inner one"));
            assert!(format!("{:?}", tracer_provider()).contains("inner one"));
        }

        assert!(format!("{:?}", tracer_provider()).contains("inner one"));
    }

    #[test]
    #[ignore = "requires --test-threads=1"]
    fn test_set_tracer_provider_in_another_thread() {
        let _ = set_tracer_provider(TestTracerProvider::new("global one"));

        let handle = thread::spawn(move || {
            assert!(format!("{:?}", tracer_provider()).contains("global one"));
        });

        println!("{:?}", tracer_provider());

        let _ = handle.join();
    }

    #[test]
    #[ignore = "requires --test-threads=1"]
    fn test_set_tracer_provider_in_another_function() {
        let setup = || {
            let _ = set_tracer_provider(TestTracerProvider::new("global one"));
            assert!(format!("{:?}", tracer_provider()).contains("global one"))
        };

        setup();

        assert!(format!("{:?}", tracer_provider()).contains("global one"))
    }

    #[test]
    #[ignore = "requires --test-threads=1"]
    fn test_set_two_provider_in_two_thread() {
        let (sender, recv) = std::sync::mpsc::channel();
        let (sender1, sender2) = (sender.clone(), sender);
        let _handle1 = thread::spawn(move || {
            sleep(Duration::from_secs(1));
            let _previous = set_tracer_provider(TestTracerProvider::new("thread 1"));
            sleep(Duration::from_secs(2));
            let _ = sender1.send(format!("thread 1: {:?}", tracer_provider()));
        });
        let _handle2 = thread::spawn(move || {
            sleep(Duration::from_secs(2));
            let _previous = set_tracer_provider(TestTracerProvider::new("thread 2"));
            sleep(Duration::from_secs(1));
            let _ = sender2.send(format!("thread 2 :{:?}", tracer_provider()));
        });

        let first_resp = recv.recv().unwrap();
        let second_resp = recv.recv().unwrap();
        assert!(first_resp.contains("thread 2"));
        assert!(second_resp.contains("thread 2"));
    }

    fn build_batch_tracer_provider<R: Runtime>(
        assert_writer: AssertWriter,
        runtime: R,
    ) -> crate::sdk::trace::TracerProvider {
        use crate::sdk::trace::TracerProvider;
        let exporter = crate::sdk::export::trace::stdout::Exporter::new(assert_writer, true);
        TracerProvider::builder()
            .with_batch_exporter(exporter, runtime)
            .build()
    }

    fn build_simple_tracer_provider(
        assert_writer: AssertWriter,
    ) -> crate::sdk::trace::TracerProvider {
        use crate::sdk::trace::TracerProvider;
        let exporter = crate::sdk::export::trace::stdout::Exporter::new(assert_writer, true);
        TracerProvider::builder()
            .with_simple_exporter(exporter)
            .build()
    }

    async fn test_set_provider_in_tokio<R: Runtime>(runtime: R) -> AssertWriter {
        let buffer = AssertWriter::new();
        let _ = set_tracer_provider(build_batch_tracer_provider(buffer.clone(), runtime));
        let tracer = tracer("opentelemetery");

        tracer.in_span("test", |_cx| {});

        buffer
    }

    // When using `tokio::spawn` to spawn the worker task in batch processor
    //
    // multiple -> no shut down -> not export
    // multiple -> shut down -> export
    // single -> no shutdown -> not export
    // single -> shutdown -> hang forever

    // When using |fut| tokio::task::spawn_blocking(|| futures::executor::block_on(fut))
    // to spawn the worker task in batch processor
    //
    // multiple -> no shutdown -> hang forever
    // multiple -> shut down -> export
    // single -> shut down -> export
    // single -> no shutdown -> hang forever

    // Test if the multiple thread tokio runtime could exit successfully when not force flushing spans
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires --test-threads=1"]
    async fn test_set_provider_multiple_thread_tokio() {
        let assert_writer = test_set_provider_in_tokio(runtime::Tokio).await;
        assert_eq!(assert_writer.len(), 0);
    }

    // Test if the multiple thread tokio runtime could exit successfully when force flushing spans
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires --test-threads=1"]
    async fn test_set_provider_multiple_thread_tokio_shutdown() {
        let assert_writer = test_set_provider_in_tokio(runtime::Tokio).await;
        shutdown_tracer_provider();
        assert!(assert_writer.len() > 0);
    }

    // Test use simple processor in single thread tokio runtime.
    // Expected to see the spans being exported to buffer
    #[tokio::test]
    #[ignore = "requires --test-threads=1"]
    async fn test_set_provider_single_thread_tokio_with_simple_processor() {
        let assert_writer = AssertWriter::new();
        let _ = set_tracer_provider(build_simple_tracer_provider(assert_writer.clone()));
        let tracer = tracer("opentelemetry");

        tracer.in_span("test", |_cx| {});

        shutdown_tracer_provider();

        assert!(assert_writer.len() > 0);
    }

    // Test if the single thread tokio runtime could exit successfully when not force flushing spans
    #[tokio::test]
    #[ignore = "requires --test-threads=1"]
    async fn test_set_provider_single_thread_tokio() {
        let assert_writer = test_set_provider_in_tokio(runtime::TokioCurrentThread).await;
        assert_eq!(assert_writer.len(), 0)
    }

    // Test if the single thread tokio runtime could exit successfully when force flushing spans.
    #[tokio::test]
    #[ignore = "requires --test-threads=1"]
    async fn test_set_provider_single_thread_tokio_shutdown() {
        let assert_writer = test_set_provider_in_tokio(runtime::TokioCurrentThread).await;
        shutdown_tracer_provider();
        assert!(assert_writer.len() > 0);
    }
}
