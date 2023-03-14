//! Async metrics
use crate::{
    global,
    metrics::{sdk_api, MetricsError, Number},
    KeyValue,
};
use std::fmt;
use std::marker;
use std::sync::Arc;

/// Observation is used for reporting an asynchronous batch of metric values.
/// Instances of this type should be created by asynchronous instruments (e.g.,
/// [ValueObserver::observation]).
///
/// [ValueObserver::observation]: crate::metrics::ValueObserver::observation()
#[derive(Debug)]
pub struct Observation {
    number: Number,
    instrument: Arc<dyn sdk_api::AsyncInstrumentCore>,
}

impl Observation {
    /// Create a new observation for an instrument
    pub(crate) fn new(number: Number, instrument: Arc<dyn sdk_api::AsyncInstrumentCore>) -> Self {
        Observation { number, instrument }
    }

    /// The value of this observation
    pub fn number(&self) -> &Number {
        &self.number
    }
    /// The instrument used to record this observation
    pub fn instrument(&self) -> &Arc<dyn sdk_api::AsyncInstrumentCore> {
        &self.instrument
    }
}

/// A type of callback that `f64` observers run.
type F64ObserverCallback = Box<dyn Fn(ObserverResult<f64>) + Send + Sync>;

/// A type of callback that `u64` observers run.
type U64ObserverCallback = Box<dyn Fn(ObserverResult<u64>) + Send + Sync>;

/// A type of callback that `u64` observers run.
type I64ObserverCallback = Box<dyn Fn(ObserverResult<i64>) + Send + Sync>;

/// A callback argument for use with any Observer instrument that will be
/// reported as a batch of observations.
type BatchObserverCallback = Box<dyn Fn(BatchObserverResult) + Send + Sync>;

/// Data passed to an observer callback to capture observations for one
/// asynchronous metric instrument.
pub struct ObserverResult<T> {
    instrument: Arc<dyn sdk_api::AsyncInstrumentCore>,
    f: fn(&[KeyValue], &[Observation]),
    _marker: marker::PhantomData<T>,
}

impl<T> ObserverResult<T>
where
    T: Into<Number>,
{
    /// New observer result for a given metric instrument
    fn new(
        instrument: Arc<dyn sdk_api::AsyncInstrumentCore>,
        f: fn(&[KeyValue], &[Observation]),
    ) -> Self {
        ObserverResult {
            instrument,
            f,
            _marker: marker::PhantomData,
        }
    }

    /// Observe captures a single value from the associated instrument callback,
    /// with the given labels.
    pub fn observe(&self, value: T, labels: &[KeyValue]) {
        (self.f)(
            labels,
            &[Observation {
                number: value.into(),
                instrument: self.instrument.clone(),
            }],
        )
    }
}

impl<T> fmt::Debug for ObserverResult<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ObserverResult")
            .field("instrument", &self.instrument)
            .field("f", &"fn(&[KeyValue], &[Observation])")
            .finish()
    }
}

/// Passed to a batch observer callback to capture observations for multiple
/// asynchronous instruments.
pub struct BatchObserverResult {
    f: fn(&[KeyValue], &[Observation]),
}

impl fmt::Debug for BatchObserverResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BatchObserverResult")
            .field("f", &"fn(&[KeyValue], &[Observation])")
            .finish()
    }
}

impl BatchObserverResult {
    /// New observer result for a given metric instrument
    fn new(f: fn(&[KeyValue], &[Observation])) -> Self {
        BatchObserverResult { f }
    }

    /// Captures multiple observations from the associated batch instrument
    /// callback, with the given labels.
    pub fn observe(&self, labels: &[KeyValue], observations: &[Observation]) {
        (self.f)(labels, observations)
    }
}

/// Called when collecting async instruments
pub enum AsyncRunner {
    /// Callback for `f64` observed values
    F64(F64ObserverCallback),
    /// Callback for `i64` observed values
    I64(I64ObserverCallback),
    /// Callback for `u64` observed values
    U64(U64ObserverCallback),
    /// Callback for batch observed values
    Batch(BatchObserverCallback),
}

impl AsyncRunner {
    /// Run accepts a single instrument and function for capturing observations
    /// of that instrument. Each call to the function receives one captured
    /// observation. (The function accepts multiple observations so the same
    /// implementation can be used for batch runners.)
    pub fn run(
        &self,
        instrument: &Option<Arc<dyn sdk_api::AsyncInstrumentCore>>,
        f: fn(&[KeyValue], &[Observation]),
    ) {
        match (instrument, self) {
            (Some(i), AsyncRunner::F64(run)) => run(ObserverResult::new(i.clone(), f)),
            (Some(i), AsyncRunner::I64(run)) => run(ObserverResult::new(i.clone(), f)),
            (Some(i), AsyncRunner::U64(run)) => run(ObserverResult::new(i.clone(), f)),
            (None, AsyncRunner::Batch(run)) => run(BatchObserverResult::new(f)),
            _ => global::handle_error(MetricsError::Other(
                "Invalid async runner / instrument pair".into(),
            )),
        }
    }
}

impl fmt::Debug for AsyncRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsyncRunner::F64(_) => f
                .debug_struct("AsyncRunner::F64")
                .field("closure", &"Fn(ObserverResult)")
                .finish(),
            AsyncRunner::I64(_) => f
                .debug_struct("AsyncRunner::I64")
                .field("closure", &"Fn(ObserverResult)")
                .finish(),
            AsyncRunner::U64(_) => f
                .debug_struct("AsyncRunner::U64")
                .field("closure", &"Fn(ObserverResult)")
                .finish(),
            AsyncRunner::Batch(_) => f
                .debug_struct("AsyncRunner::Batch")
                .field("closure", &"Fn(BatchObserverResult)")
                .finish(),
        }
    }
}
