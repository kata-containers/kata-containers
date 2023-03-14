use crate::sdk::InstrumentationLibrary;
use crate::{
    metrics::{
        sdk_api, AsyncRunner, BatchObserver, BatchObserverResult, CounterBuilder, Descriptor,
        Measurement, NumberKind, ObserverResult, Result, SumObserverBuilder, UpDownCounterBuilder,
        UpDownSumObserverBuilder, ValueObserverBuilder, ValueRecorderBuilder,
    },
    Context, KeyValue,
};
use std::fmt;
use std::sync::Arc;

/// Returns named meter instances
pub trait MeterProvider: fmt::Debug {
    /// Creates an implementation of the [`Meter`] interface. The
    /// instrumentation name must be the name of the library providing
    /// instrumentation. This name may be the same as the instrumented code only if
    /// that code provides built-in instrumentation. If the instrumentation name is
    /// empty, then a implementation defined default name will be used instead.
    ///
    fn meter(
        &self,
        instrumentation_name: &'static str,
        instrumentation_version: Option<&'static str>,
    ) -> Meter;
}

/// Meter is the OpenTelemetry metric API, based on a sdk-defined `MeterCore`
/// implementation and the `Meter` library name.
///
/// # Instruments
///
/// | **Name** | Instrument kind | Function(argument) | Default aggregation | Notes |
/// | ----------------------- | ----- | --------- | ------------- | --- |
/// | **Counter**             | Synchronous adding monotonic | Add(increment) | Sum | Per-request, part of a monotonic sum |
/// | **UpDownCounter**       | Synchronous adding | Add(increment) | Sum | Per-request, part of a non-monotonic sum |
/// | **ValueRecorder**       | Synchronous  | Record(value) | [TBD issue 636](https://github.com/open-telemetry/opentelemetry-specification/issues/636)  | Per-request, any grouping measurement |
/// | **SumObserver**         | Asynchronous adding monotonic | Observe(sum) | Sum | Per-interval, reporting a monotonic sum |
/// | **UpDownSumObserver**   | Asynchronous adding | Observe(sum) | Sum | Per-interval, reporting a non-monotonic sum |
/// | **ValueObserver**       | Asynchronous | Observe(value) | LastValue  | Per-interval, any grouping measurement |
#[derive(Debug)]
pub struct Meter {
    instrumentation_library: InstrumentationLibrary,
    core: Arc<dyn sdk_api::MeterCore + Send + Sync>,
}

impl Meter {
    /// Create a new named meter from a sdk implemented core
    pub fn new<T: Into<&'static str>>(
        instrumentation_name: T,
        instrumentation_version: Option<T>,
        core: Arc<dyn sdk_api::MeterCore + Send + Sync>,
    ) -> Self {
        Meter {
            instrumentation_library: InstrumentationLibrary::new(
                instrumentation_name.into(),
                instrumentation_version.map(Into::into),
            ),
            core,
        }
    }

    pub(crate) fn instrumentation_library(&self) -> InstrumentationLibrary {
        self.instrumentation_library
    }

    /// Creates a new integer `CounterBuilder` for `u64` values with the given name.
    pub fn u64_counter<T>(&self, name: T) -> CounterBuilder<'_, u64>
    where
        T: Into<String>,
    {
        CounterBuilder::new(self, name.into(), NumberKind::U64)
    }

    /// Creates a new floating point `CounterBuilder` for `f64` values with the given name.
    pub fn f64_counter<T>(&self, name: T) -> CounterBuilder<'_, f64>
    where
        T: Into<String>,
    {
        CounterBuilder::new(self, name.into(), NumberKind::F64)
    }

    /// Creates a new integer `UpDownCounterBuilder` for an `i64` up down counter with the given name.
    pub fn i64_up_down_counter<T>(&self, name: T) -> UpDownCounterBuilder<'_, i64>
    where
        T: Into<String>,
    {
        UpDownCounterBuilder::new(self, name.into(), NumberKind::I64)
    }

    /// Creates a new floating point `UpDownCounterBuilder` for an `f64` up down counter with the given name.
    pub fn f64_up_down_counter<T>(&self, name: T) -> UpDownCounterBuilder<'_, f64>
    where
        T: Into<String>,
    {
        UpDownCounterBuilder::new(self, name.into(), NumberKind::F64)
    }

    /// Creates a new `ValueRecorderBuilder` for `i64` values with the given name.
    pub fn i64_value_recorder<T>(&self, name: T) -> ValueRecorderBuilder<'_, i64>
    where
        T: Into<String>,
    {
        ValueRecorderBuilder::new(self, name.into(), NumberKind::I64)
    }

    /// Creates a new `ValueRecorderBuilder` for `u64` values with the given name.
    pub fn u64_value_recorder<T>(&self, name: T) -> ValueRecorderBuilder<'_, u64>
    where
        T: Into<String>,
    {
        ValueRecorderBuilder::new(self, name.into(), NumberKind::U64)
    }

    /// Creates a new `ValueRecorderBuilder` for `f64` values with the given name.
    pub fn f64_value_recorder<T>(&self, name: T) -> ValueRecorderBuilder<'_, f64>
    where
        T: Into<String>,
    {
        ValueRecorderBuilder::new(self, name.into(), NumberKind::F64)
    }

    /// Creates a new integer `SumObserverBuilder` for `u64` values with the given
    /// name and callback
    pub fn u64_sum_observer<T, F>(&self, name: T, callback: F) -> SumObserverBuilder<'_, u64>
    where
        T: Into<String>,
        F: Fn(ObserverResult<u64>) + Send + Sync + 'static,
    {
        SumObserverBuilder::new(
            self,
            name.into(),
            Some(AsyncRunner::U64(Box::new(callback))),
            NumberKind::U64,
        )
    }

    /// Creates a new floating point `SumObserverBuilder` for `f64` values with the
    /// given name and callback
    pub fn f64_sum_observer<T, F>(&self, name: T, callback: F) -> SumObserverBuilder<'_, f64>
    where
        T: Into<String>,
        F: Fn(ObserverResult<f64>) + Send + Sync + 'static,
    {
        SumObserverBuilder::new(
            self,
            name.into(),
            Some(AsyncRunner::F64(Box::new(callback))),
            NumberKind::F64,
        )
    }

    /// Creates a new integer `UpDownSumObserverBuilder` for `i64` values with the
    /// given name and callback.
    pub fn i64_up_down_sum_observer<T, F>(
        &self,
        name: T,
        callback: F,
    ) -> UpDownSumObserverBuilder<'_, i64>
    where
        T: Into<String>,
        F: Fn(ObserverResult<i64>) + Send + Sync + 'static,
    {
        UpDownSumObserverBuilder::new(
            self,
            name.into(),
            Some(AsyncRunner::I64(Box::new(callback))),
            NumberKind::I64,
        )
    }

    /// Creates a new floating point `UpDownSumObserverBuilder` for `f64` values
    /// with the given name and callback
    pub fn f64_up_down_sum_observer<T, F>(
        &self,
        name: T,
        callback: F,
    ) -> UpDownSumObserverBuilder<'_, f64>
    where
        T: Into<String>,
        F: Fn(ObserverResult<f64>) + Send + Sync + 'static,
    {
        UpDownSumObserverBuilder::new(
            self,
            name.into(),
            Some(AsyncRunner::F64(Box::new(callback))),
            NumberKind::F64,
        )
    }

    /// Creates a new integer `ValueObserverBuilder` for `u64` values with the given
    /// name and callback
    pub fn u64_value_observer<T, F>(&self, name: T, callback: F) -> ValueObserverBuilder<'_, u64>
    where
        T: Into<String>,
        F: Fn(ObserverResult<u64>) + Send + Sync + 'static,
    {
        ValueObserverBuilder::new(
            self,
            name.into(),
            Some(AsyncRunner::U64(Box::new(callback))),
            NumberKind::U64,
        )
    }

    /// Creates a new integer `ValueObserverBuilder` for `i64` values with the given
    /// name and callback
    pub fn i64_value_observer<T, F>(&self, name: T, callback: F) -> ValueObserverBuilder<'_, i64>
    where
        T: Into<String>,
        F: Fn(ObserverResult<i64>) + Send + Sync + 'static,
    {
        ValueObserverBuilder::new(
            self,
            name.into(),
            Some(AsyncRunner::I64(Box::new(callback))),
            NumberKind::I64,
        )
    }

    /// Creates a new floating point `ValueObserverBuilder` for `f64` values with
    /// the given name and callback
    pub fn f64_value_observer<T, F>(&self, name: T, callback: F) -> ValueObserverBuilder<'_, f64>
    where
        T: Into<String>,
        F: Fn(ObserverResult<f64>) + Send + Sync + 'static,
    {
        ValueObserverBuilder::new(
            self,
            name.into(),
            Some(AsyncRunner::F64(Box::new(callback))),
            NumberKind::F64,
        )
    }

    /// Creates a new `BatchObserver` that supports making batches of observations
    /// for multiple instruments or returns an error if instrument initialization
    /// fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{global, metrics::BatchObserverResult, KeyValue};
    ///
    /// # fn init_observer() -> opentelemetry::metrics::Result<()> {
    /// let meter = global::meter("test");
    ///
    /// meter.build_batch_observer(|batch| {
    ///   let instrument = batch.u64_value_observer("test_instrument").try_init()?;
    ///
    ///   Ok(move |result: BatchObserverResult| {
    ///     result.observe(&[KeyValue::new("my-key", "my-value")], &[instrument.observation(1)]);
    ///   })
    /// })?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build_batch_observer<B, F>(&self, builder: B) -> Result<()>
    where
        B: Fn(BatchObserver<'_>) -> Result<F>,
        F: Fn(BatchObserverResult) + Send + Sync + 'static,
    {
        let observer = builder(BatchObserver::new(self))?;
        self.core
            .new_batch_observer(AsyncRunner::Batch(Box::new(observer)))
    }

    /// Creates a new `BatchObserver` that supports making batches of observations
    /// for multiple instruments.
    ///
    /// # Panics
    ///
    /// Panics if instrument initialization or observer registration returns an
    /// error.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{global, metrics::BatchObserverResult, KeyValue};
    ///
    /// let meter = global::meter("test");
    ///
    /// meter.batch_observer(|batch| {
    ///   let instrument = batch.u64_value_observer("test_instrument").init();
    ///
    ///   move |result: BatchObserverResult| {
    ///     result.observe(&[KeyValue::new("my-key", "my-value")], &[instrument.observation(1)]);
    ///   }
    /// });
    /// ```
    pub fn batch_observer<B, F>(&self, builder: B)
    where
        B: Fn(BatchObserver<'_>) -> F,
        F: Fn(BatchObserverResult) + Send + Sync + 'static,
    {
        let observer = builder(BatchObserver::new(self));
        self.core
            .new_batch_observer(AsyncRunner::Batch(Box::new(observer)))
            .unwrap()
    }

    /// Atomically record a batch of measurements.
    pub fn record_batch<T: IntoIterator<Item = Measurement>>(
        &self,
        labels: &[KeyValue],
        measurements: T,
    ) {
        self.record_batch_with_context(&Context::current(), labels, measurements)
    }

    /// Atomically record a batch of measurements with a given context
    pub fn record_batch_with_context<T: IntoIterator<Item = Measurement>>(
        &self,
        cx: &Context,
        labels: &[KeyValue],
        measurements: T,
    ) {
        self.core
            .record_batch_with_context(cx, labels, measurements.into_iter().collect())
    }

    pub(crate) fn new_sync_instrument(
        &self,
        descriptor: Descriptor,
    ) -> Result<Arc<dyn sdk_api::SyncInstrumentCore>> {
        self.core.new_sync_instrument(descriptor)
    }

    pub(crate) fn new_async_instrument(
        &self,
        descriptor: Descriptor,
        runner: Option<AsyncRunner>,
    ) -> Result<Arc<dyn sdk_api::AsyncInstrumentCore>> {
        self.core.new_async_instrument(descriptor, runner)
    }
}
