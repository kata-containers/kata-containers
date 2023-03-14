//! # OpenTelemetry Metrics API

use std::result;
use std::sync::PoisonError;
use thiserror::Error;

mod async_instrument;
mod config;
mod counter;
mod descriptor;
mod kind;
mod meter;
pub mod noop;
mod number;
mod observer;
pub mod registry;
pub mod sdk_api;
mod sync_instrument;
mod up_down_counter;
mod value_recorder;

use crate::sdk::export::ExportError;
pub use async_instrument::{AsyncRunner, BatchObserverResult, Observation, ObserverResult};
pub use config::InstrumentConfig;
pub use counter::{BoundCounter, Counter, CounterBuilder};
pub use descriptor::Descriptor;
pub use kind::InstrumentKind;
pub use meter::{Meter, MeterProvider};
pub use number::{AtomicNumber, Number, NumberKind};
pub use observer::{
    BatchObserver, SumObserver, SumObserverBuilder, UpDownSumObserver, UpDownSumObserverBuilder,
    ValueObserver, ValueObserverBuilder,
};
pub use sync_instrument::Measurement;
pub use up_down_counter::{BoundUpDownCounter, UpDownCounter, UpDownCounterBuilder};
pub use value_recorder::{BoundValueRecorder, ValueRecorder, ValueRecorderBuilder};

/// A specialized `Result` type for metric operations.
pub type Result<T> = result::Result<T, MetricsError>;

/// Errors returned by the metrics API.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum MetricsError {
    /// Other errors not covered by specific cases.
    #[error("Metrics error: {0}")]
    Other(String),
    /// Errors when requesting quantiles out of the 0-1 range.
    #[error("The requested quantile is out of range")]
    InvalidQuantile,
    /// Errors when recording nan values.
    #[error("NaN value is an invalid input")]
    NaNInput,
    /// Errors when recording negative values in monotonic sums.
    #[error("Negative value is out of range for this instrument")]
    NegativeInput,
    /// Errors when merging aggregators of incompatible types.
    #[error("Inconsistent aggregator types: {0}")]
    InconsistentAggregator(String),
    /// Errors when requesting data when no data has been collected
    #[error("No data collected by this aggregator")]
    NoDataCollected,
    /// Errors when registering to instruments with the same name and kind
    #[error("A metric was already registered by this name with another kind or number type: {0}")]
    MetricKindMismatch(String),
    /// Errors when processor logic is incorrect
    #[error("Inconsistent processor state")]
    InconsistentState,
    /// Errors when aggregator cannot subtract
    #[error("Aggregator does not subtract")]
    NoSubtraction,
    /// Fail to export metrics
    #[error("Metrics exporter {} failed with {0}", .0.exporter_name())]
    ExportErr(Box<dyn ExportError>),
}

impl<T: ExportError> From<T> for MetricsError {
    fn from(err: T) -> Self {
        MetricsError::ExportErr(Box::new(err))
    }
}

impl<T> From<PoisonError<T>> for MetricsError {
    fn from(err: PoisonError<T>) -> Self {
        MetricsError::Other(err.to_string())
    }
}
