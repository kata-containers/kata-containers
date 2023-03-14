//! Metrics Registry API
use crate::{
    metrics::{
        sdk_api::{AsyncInstrumentCore, MeterCore, SyncInstrumentCore},
        Meter, MeterProvider,
    },
    metrics::{AsyncRunner, Descriptor, Measurement, MetricsError, Result},
    Context, KeyValue,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Create a new `RegistryMeterProvider` from a `MeterCore`.
pub fn meter_provider(core: Arc<dyn MeterCore + Send + Sync>) -> RegistryMeterProvider {
    RegistryMeterProvider(Arc::new(UniqueInstrumentMeterCore::wrap(core)))
}

/// A standard `MeterProvider` for wrapping a `MeterCore`.
#[derive(Debug, Clone)]
pub struct RegistryMeterProvider(Arc<dyn MeterCore + Send + Sync>);

impl MeterProvider for RegistryMeterProvider {
    fn meter(&self, name: &'static str, version: Option<&'static str>) -> Meter {
        Meter::new(name, version, self.0.clone())
    }
}

#[derive(Debug)]
struct UniqueInstrumentMeterCore {
    inner: Arc<dyn MeterCore + Send + Sync>,
    sync_state: Mutex<HashMap<UniqueInstrumentKey, UniqueSyncInstrument>>,
    async_state: Mutex<HashMap<UniqueInstrumentKey, UniqueAsyncInstrument>>,
}

impl UniqueInstrumentMeterCore {
    fn wrap(inner: Arc<dyn MeterCore + Send + Sync>) -> Self {
        UniqueInstrumentMeterCore {
            inner,
            sync_state: Mutex::new(HashMap::default()),
            async_state: Mutex::new(HashMap::default()),
        }
    }
}

impl MeterCore for UniqueInstrumentMeterCore {
    fn record_batch_with_context(
        &self,
        cx: &Context,
        attributes: &[KeyValue],
        measurements: Vec<Measurement>,
    ) {
        self.inner
            .record_batch_with_context(cx, attributes, measurements)
    }

    fn new_sync_instrument(&self, descriptor: Descriptor) -> Result<UniqueSyncInstrument> {
        self.sync_state
            .lock()
            .map_err(Into::into)
            .and_then(|mut state| {
                let key = UniqueInstrumentKey::from(&descriptor);
                check_sync_uniqueness(&state, &descriptor, &key).and_then(|instrument| {
                    match instrument {
                        Some(instrument) => Ok(instrument),
                        None => {
                            let instrument = self.inner.new_sync_instrument(descriptor)?;
                            state.insert(key, instrument.clone());

                            Ok(instrument)
                        }
                    }
                })
            })
    }

    fn new_async_instrument(
        &self,
        descriptor: Descriptor,
        runner: Option<AsyncRunner>,
    ) -> super::Result<UniqueAsyncInstrument> {
        self.async_state
            .lock()
            .map_err(Into::into)
            .and_then(|mut state| {
                let key = UniqueInstrumentKey::from(&descriptor);
                check_async_uniqueness(&state, &descriptor, &key).and_then(|instrument| {
                    match instrument {
                        Some(instrument) => Ok(instrument),
                        None => {
                            let instrument = self.inner.new_async_instrument(descriptor, runner)?;
                            state.insert(key, instrument.clone());

                            Ok(instrument)
                        }
                    }
                })
            })
    }

    fn new_batch_observer(&self, runner: AsyncRunner) -> Result<()> {
        self.inner.new_batch_observer(runner)
    }
}

fn check_sync_uniqueness(
    instruments: &HashMap<UniqueInstrumentKey, UniqueSyncInstrument>,
    desc: &Descriptor,
    key: &UniqueInstrumentKey,
) -> Result<Option<UniqueSyncInstrument>> {
    if let Some(instrument) = instruments.get(key) {
        if is_equal(instrument.descriptor(), desc) {
            Ok(Some(instrument.clone()))
        } else {
            Err(MetricsError::MetricKindMismatch(format!(
                "metric was {} ({}), registered as a {:?} {:?}",
                desc.name(),
                desc.instrumentation_name(),
                desc.number_kind(),
                desc.instrument_kind()
            )))
        }
    } else {
        Ok(None)
    }
}

fn check_async_uniqueness(
    instruments: &HashMap<UniqueInstrumentKey, UniqueAsyncInstrument>,
    desc: &Descriptor,
    key: &UniqueInstrumentKey,
) -> Result<Option<UniqueAsyncInstrument>> {
    if let Some(instrument) = instruments.get(key) {
        if is_equal(instrument.descriptor(), desc) {
            Ok(Some(instrument.clone()))
        } else {
            Err(MetricsError::MetricKindMismatch(format!(
                "metric was {} ({}), registered as a {:?} {:?}",
                desc.name(),
                desc.instrumentation_name(),
                desc.number_kind(),
                desc.instrument_kind()
            )))
        }
    } else {
        Ok(None)
    }
}

fn is_equal(a: &Descriptor, b: &Descriptor) -> bool {
    a.instrument_kind() == b.instrument_kind() && a.number_kind() == b.number_kind()
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct UniqueInstrumentKey {
    instrument_name: String,
    instrumentation_name: String,
}

impl From<&Descriptor> for UniqueInstrumentKey {
    fn from(desc: &Descriptor) -> Self {
        UniqueInstrumentKey {
            instrument_name: desc.name().to_string(),
            instrumentation_name: desc.instrumentation_name().to_string(),
        }
    }
}

type UniqueSyncInstrument = Arc<dyn SyncInstrumentCore>;
type UniqueAsyncInstrument = Arc<dyn AsyncInstrumentCore>;
