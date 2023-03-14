//! # No-op OpenTelemetry Metrics Implementation
//!
//! This implementation is returned as the global Meter if no `Meter`
//! has been set. It is also useful for testing purposes as it is intended
//! to have minimal resource utilization and runtime impact.
use crate::{
    metrics::{
        sdk_api::{
            AsyncInstrumentCore, InstrumentCore, MeterCore, SyncBoundInstrumentCore,
            SyncInstrumentCore,
        },
        AsyncRunner, Descriptor, InstrumentKind, Measurement, Meter, MeterProvider, Number,
        NumberKind, Result,
    },
    Context, KeyValue,
};
use std::any::Any;
use std::sync::Arc;

lazy_static::lazy_static! {
    static ref NOOP_DESCRIPTOR: Descriptor = Descriptor::new(String::new(), "noop", None, InstrumentKind::Counter, NumberKind::U64);
}

/// A no-op instance of a `MetricProvider`
#[derive(Debug, Default)]
pub struct NoopMeterProvider {
    _private: (),
}

impl NoopMeterProvider {
    /// Create a new no-op meter provider.
    pub fn new() -> Self {
        NoopMeterProvider { _private: () }
    }
}

impl MeterProvider for NoopMeterProvider {
    fn meter(&self, name: &'static str, version: Option<&'static str>) -> Meter {
        Meter::new(name, version, Arc::new(NoopMeterCore::new()))
    }
}

/// A no-op instance of a `Meter`
#[derive(Debug, Default)]
pub struct NoopMeterCore {
    _private: (),
}

impl NoopMeterCore {
    /// Create a new no-op meter core.
    pub fn new() -> Self {
        NoopMeterCore { _private: () }
    }
}

impl MeterCore for NoopMeterCore {
    fn new_sync_instrument(&self, _descriptor: Descriptor) -> Result<Arc<dyn SyncInstrumentCore>> {
        Ok(Arc::new(NoopSyncInstrument::new()))
    }

    fn new_async_instrument(
        &self,
        _descriptor: Descriptor,
        _runner: Option<AsyncRunner>,
    ) -> Result<Arc<dyn AsyncInstrumentCore>> {
        Ok(Arc::new(NoopAsyncInstrument::new()))
    }

    fn record_batch_with_context(
        &self,
        _cx: &Context,
        _labels: &[KeyValue],
        _measurements: Vec<Measurement>,
    ) {
        // Ignored
    }

    fn new_batch_observer(&self, _runner: AsyncRunner) -> Result<()> {
        Ok(())
    }
}

/// A no-op sync instrument
#[derive(Debug, Default)]
pub struct NoopSyncInstrument {
    _private: (),
}

impl NoopSyncInstrument {
    /// Create a new no-op sync instrument
    pub fn new() -> Self {
        NoopSyncInstrument { _private: () }
    }
}

impl InstrumentCore for NoopSyncInstrument {
    fn descriptor(&self) -> &Descriptor {
        &NOOP_DESCRIPTOR
    }
}

impl SyncInstrumentCore for NoopSyncInstrument {
    fn bind(&self, _labels: &'_ [KeyValue]) -> Arc<dyn SyncBoundInstrumentCore> {
        Arc::new(NoopBoundSyncInstrument::new())
    }
    fn record_one(&self, _number: Number, _labels: &'_ [KeyValue]) {
        // Ignored
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A no-op bound sync instrument
#[derive(Debug, Default)]
pub struct NoopBoundSyncInstrument {
    _private: (),
}

impl NoopBoundSyncInstrument {
    /// Create a new no-op bound sync instrument
    pub fn new() -> Self {
        NoopBoundSyncInstrument { _private: () }
    }
}

impl SyncBoundInstrumentCore for NoopBoundSyncInstrument {
    fn record_one(&self, _number: Number) {
        // Ignored
    }
}

/// A no-op async instrument.
#[derive(Debug, Default)]
pub struct NoopAsyncInstrument {
    _private: (),
}

impl NoopAsyncInstrument {
    /// Create a new no-op async instrument
    pub fn new() -> Self {
        NoopAsyncInstrument { _private: () }
    }
}

impl InstrumentCore for NoopAsyncInstrument {
    fn descriptor(&self) -> &Descriptor {
        &NOOP_DESCRIPTOR
    }
}

impl AsyncInstrumentCore for NoopAsyncInstrument {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
