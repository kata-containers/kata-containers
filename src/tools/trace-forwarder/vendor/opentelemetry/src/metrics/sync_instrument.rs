use crate::{
    metrics::{sdk_api, Number},
    KeyValue,
};
use std::marker;
use std::sync::Arc;

/// Measurement is used for reporting a synchronous batch of metric values.
/// Instances of this type should be created by synchronous instruments (e.g.,
/// `Counter::measurement`).
#[derive(Debug)]
pub struct Measurement {
    number: Number,
    instrument: Arc<dyn sdk_api::SyncInstrumentCore>,
}

impl Measurement {
    /// Create a new measurement for an instrument
    pub(crate) fn new(number: Number, instrument: Arc<dyn sdk_api::SyncInstrumentCore>) -> Self {
        Measurement { number, instrument }
    }

    /// The number recorded by this measurement
    pub fn number(&self) -> &Number {
        &self.number
    }

    /// Convert this measurement into the underlying number
    pub fn into_number(self) -> Number {
        self.number
    }

    /// The instrument that recorded this measurement
    pub fn instrument(&self) -> &Arc<dyn sdk_api::SyncInstrumentCore> {
        &self.instrument
    }
}

/// Wrapper around a sdk-implemented sync instrument for a given type
#[derive(Clone, Debug)]
pub(crate) struct SyncInstrument<T> {
    instrument: Arc<dyn sdk_api::SyncInstrumentCore>,
    _marker: marker::PhantomData<T>,
}

impl<T> SyncInstrument<T> {
    /// Create a new sync instrument from an sdk-implemented sync instrument
    pub(crate) fn new(instrument: Arc<dyn sdk_api::SyncInstrumentCore>) -> Self {
        SyncInstrument {
            instrument,
            _marker: marker::PhantomData,
        }
    }

    /// Create a new bound sync instrument
    pub(crate) fn bind(&self, attributes: &[KeyValue]) -> SyncBoundInstrument<T> {
        let bound_instrument = self.instrument.bind(attributes);
        SyncBoundInstrument {
            bound_instrument,
            _marker: marker::PhantomData,
        }
    }

    /// Record a value directly to the underlying instrument
    pub(crate) fn direct_record(&self, number: Number, attributes: &[KeyValue]) {
        self.instrument.record_one(number, attributes)
    }

    /// Reference to the underlying sdk-implemented instrument
    pub(crate) fn instrument(&self) -> &Arc<dyn sdk_api::SyncInstrumentCore> {
        &self.instrument
    }
}

/// Wrapper around a sdk-implemented sync bound instrument
#[derive(Clone, Debug)]
pub(crate) struct SyncBoundInstrument<T> {
    bound_instrument: Arc<dyn sdk_api::SyncBoundInstrumentCore>,
    _marker: marker::PhantomData<T>,
}

impl<T> SyncBoundInstrument<T> {
    /// Record a value directly to the underlying instrument
    pub(crate) fn direct_record(&self, number: Number) {
        self.bound_instrument.record_one(number)
    }
}
