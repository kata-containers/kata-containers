//! Metrics SDK API
use crate::metrics::{AsyncRunner, Descriptor, Measurement, Number, Result};
use crate::{Context, KeyValue};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// The interface an SDK must implement to supply a Meter implementation.
pub trait MeterCore: fmt::Debug {
    /// Atomically record a batch of measurements.
    fn record_batch_with_context(
        &self,
        cx: &Context,
        attributes: &[KeyValue],
        measurements: Vec<Measurement>,
    );

    /// Create a new synchronous instrument implementation.
    fn new_sync_instrument(&self, descriptor: Descriptor) -> Result<Arc<dyn SyncInstrumentCore>>;

    /// Create a new asynchronous instrument implementation.
    ///
    /// Runner is `None` if used in batch as the batch runner is registered separately.
    fn new_async_instrument(
        &self,
        descriptor: Descriptor,
        runner: Option<AsyncRunner>,
    ) -> Result<Arc<dyn AsyncInstrumentCore>>;

    /// Register a batch observer
    fn new_batch_observer(&self, runner: AsyncRunner) -> Result<()>;
}

/// A common interface for synchronous and asynchronous instruments.
pub trait InstrumentCore: fmt::Debug + Send + Sync {
    /// Description of the instrument's descriptor
    fn descriptor(&self) -> &Descriptor;
}

/// The implementation-level interface to a generic synchronous instrument
/// (e.g., ValueRecorder and Counter instruments).
pub trait SyncInstrumentCore: InstrumentCore {
    /// Creates an implementation-level bound instrument, binding an attribute set
    /// with this instrument implementation.
    fn bind(&self, attributes: &'_ [KeyValue]) -> Arc<dyn SyncBoundInstrumentCore>;

    /// Capture a single synchronous metric event.
    fn record_one(&self, number: Number, attributes: &'_ [KeyValue]);

    /// Returns self as any
    fn as_any(&self) -> &dyn Any;
}

/// The implementation-level interface to a generic synchronous bound instrument
pub trait SyncBoundInstrumentCore: fmt::Debug + Send + Sync {
    /// Capture a single synchronous metric event.
    fn record_one(&self, number: Number);
}

/// An implementation-level interface to an asynchronous instrument (e.g.,
/// Observer instruments).
pub trait AsyncInstrumentCore: InstrumentCore {
    /// The underlying type as `Any` to support downcasting.
    fn as_any(&self) -> &dyn Any;
}
