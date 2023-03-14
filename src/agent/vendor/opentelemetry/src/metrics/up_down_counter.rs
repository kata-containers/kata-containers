use crate::{
    metrics::{
        sync_instrument::{SyncBoundInstrument, SyncInstrument},
        Descriptor, InstrumentKind, Measurement, Meter, Number, NumberKind, Result,
    },
    KeyValue, Unit,
};
use std::marker;

/// A metric instrument that sums non-monotonic values.
#[derive(Clone, Debug)]
pub struct UpDownCounter<T>(SyncInstrument<T>);

impl<T> UpDownCounter<T>
where
    T: Into<Number>,
{
    /// Creates a bound instrument for this counter. The labels are associated with
    /// values recorded via subsequent calls to record.
    pub fn bind<'a>(&self, labels: &'a [KeyValue]) -> BoundUpDownCounter<'a, T> {
        let bound_instrument = self.0.bind(labels);

        BoundUpDownCounter {
            labels,
            bound_instrument,
        }
    }

    /// Increment this counter by a given T
    pub fn add(&self, value: T, labels: &[KeyValue]) {
        self.0.direct_record(value.into(), labels)
    }

    /// Creates a Measurement for use with batch recording.
    pub fn measurement(&self, value: T) -> Measurement {
        Measurement::new(value.into(), self.0.instrument().clone())
    }
}

/// BoundUpDownCounter is a bound instrument for counters.
#[derive(Clone, Debug)]
pub struct BoundUpDownCounter<'a, T> {
    labels: &'a [KeyValue],
    bound_instrument: SyncBoundInstrument<T>,
}

impl<'a, T> BoundUpDownCounter<'a, T>
where
    T: Into<Number>,
{
    /// Increment this counter by a given T
    pub fn add(&self, value: T) {
        self.bound_instrument.direct_record(value.into())
    }
}

/// Configuration for a new up down counter.
#[derive(Debug)]
pub struct UpDownCounterBuilder<'a, T> {
    meter: &'a Meter,
    descriptor: Descriptor,
    _marker: marker::PhantomData<T>,
}

impl<'a, T> UpDownCounterBuilder<'a, T> {
    /// Create a new counter builder
    pub(crate) fn new(meter: &'a Meter, name: String, number_kind: NumberKind) -> Self {
        UpDownCounterBuilder {
            meter,
            descriptor: Descriptor::new(
                name,
                meter.instrumentation_library().name,
                meter.instrumentation_library().version,
                InstrumentKind::UpDownCounter,
                number_kind,
            ),
            _marker: marker::PhantomData,
        }
    }

    /// Set the description for this counter
    pub fn with_description<S: Into<String>>(mut self, description: S) -> Self {
        self.descriptor.set_description(description.into());
        self
    }

    /// Set the unit for this counter.
    pub fn with_unit(mut self, unit: Unit) -> Self {
        self.descriptor.config.unit = Some(unit);
        self
    }

    /// Creates a new counter instrument.
    pub fn try_init(self) -> Result<UpDownCounter<T>> {
        let instrument = self.meter.new_sync_instrument(self.descriptor)?;
        Ok(UpDownCounter(SyncInstrument::new(instrument)))
    }

    /// Creates a new counter instrument.
    ///
    /// # Panics
    ///
    /// This function panics if the instrument cannot be created. Use try_init if you want to
    /// handle errors.
    pub fn init(self) -> UpDownCounter<T> {
        UpDownCounter(SyncInstrument::new(
            self.meter.new_sync_instrument(self.descriptor).unwrap(),
        ))
    }
}
