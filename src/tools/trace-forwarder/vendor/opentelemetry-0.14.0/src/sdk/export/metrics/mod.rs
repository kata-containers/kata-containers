//! Metrics Export
use crate::sdk::resource::Resource;
use crate::{
    labels,
    metrics::{Descriptor, InstrumentKind, Number, Result},
};
use std::any::Any;
use std::fmt;
use std::sync::Arc;
use std::time::SystemTime;

mod aggregation;
pub mod stdout;

pub use aggregation::{
    Buckets, Count, Histogram, LastValue, Max, Min, MinMaxSumCount, Points, Sum,
};
pub use stdout::stdout;

/// Processor is responsible for deciding which kind of aggregation to use (via
/// `aggregation_selector`), gathering exported results from the SDK during
/// collection, and deciding over which dimensions to group the exported data.
///
/// The SDK supports binding only one of these interfaces, as it has the sole
/// responsibility of determining which Aggregator to use for each record.
///
/// The embedded AggregatorSelector interface is called (concurrently) in
/// instrumentation context to select the appropriate Aggregator for an
/// instrument.
pub trait Processor: fmt::Debug {
    /// AggregatorSelector is responsible for selecting the
    /// concrete type of Aggregator used for a metric in the SDK.
    ///
    /// This may be a static decision based on fields of the
    /// Descriptor, or it could use an external configuration
    /// source to customize the treatment of each metric
    /// instrument.
    ///
    /// The result from AggregatorSelector.AggregatorFor should be
    /// the same type for a given Descriptor or else nil.  The same
    /// type should be returned for a given descriptor, because
    /// Aggregators only know how to Merge with their own type.  If
    /// the result is nil, the metric instrument will be disabled.
    ///
    /// Note that the SDK only calls AggregatorFor when new records
    /// require an Aggregator. This does not provide a way to
    /// disable metrics with active records.
    fn aggregation_selector(&self) -> &dyn AggregatorSelector;
}

/// A locked processor.
///
/// The `Process` method is called during collection in a single-threaded
/// context from the SDK, after the aggregator is checkpointed, allowing the
/// processor to build the set of metrics currently being exported.
pub trait LockedProcessor {
    /// Process is called by the SDK once per internal record, passing the export
    /// Accumulation (a Descriptor, the corresponding Labels, and the checkpointed
    /// Aggregator).
    ///
    /// The Context argument originates from the controller that orchestrates
    /// collection.
    fn process(&mut self, accumulation: Accumulation<'_>) -> Result<()>;
}

/// AggregatorSelector supports selecting the kind of `Aggregator` to use at
/// runtime for a specific metric instrument.
pub trait AggregatorSelector: fmt::Debug {
    /// This allocates a variable number of aggregators of a kind suitable for
    /// the requested export.
    ///
    /// When the call returns `None`, the metric instrument is explicitly disabled.
    ///
    /// This must return a consistent type to avoid confusion in later stages of
    /// the metrics export process, e.g., when merging or checkpointing
    /// aggregators for a specific instrument.
    ///
    /// This call should not block.
    fn aggregator_for(&self, descriptor: &Descriptor) -> Option<Arc<dyn Aggregator + Send + Sync>>;
}

/// The interface used by a `Controller` to coordinate the `Processor` with
/// `Accumulator`(s) and `Exporter`(s). The `start_collection` and
/// `finish_collection` methods start and finish a collection interval.
/// `Controller`s call the `Accumulator`(s) during collection to process
/// `Accumulation`s.
pub trait Checkpointer: LockedProcessor {
    /// A checkpoint of the current data set. This may be called before and after
    /// collection. The implementation is required to return the same value
    /// throughout its lifetime.
    fn checkpoint_set(&mut self) -> &mut dyn CheckpointSet;

    /// Logic to be run at the start of a collection interval.
    fn start_collection(&mut self);

    /// Cleanup logic or other behavior that needs to be run after a collection
    /// interval is complete.
    fn finish_collection(&mut self) -> Result<()>;
}

/// Aggregator implements a specific aggregation behavior, i.e., a behavior to
/// track a sequence of updates to an instrument. Sum-only instruments commonly
/// use a simple Sum aggregator, but for the distribution instruments
/// (ValueRecorder, ValueObserver) there are a number of possible aggregators
/// with different cost and accuracy tradeoffs.
///
/// Note that any Aggregator may be attached to any instrument--this is the
/// result of the OpenTelemetry API/SDK separation. It is possible to attach a
/// Sum aggregator to a ValueRecorder instrument or a MinMaxSumCount aggregator
/// to a Counter instrument.
pub trait Aggregator: fmt::Debug {
    /// Update receives a new measured value and incorporates it into the
    /// aggregation. Update calls may be called concurrently.
    ///
    /// `Descriptor::number_kind` should be consulted to determine whether the
    /// provided number is an `i64`, `u64` or `f64`.
    ///
    /// The current Context could be inspected for a `Baggage` or
    /// `SpanContext`.
    fn update(&self, number: &Number, descriptor: &Descriptor) -> Result<()>;

    /// This method is called during collection to finish one period of aggregation
    /// by atomically saving the currently-updating state into the argument
    /// Aggregator.
    ///
    /// `synchronized_move` is called concurrently with `update`. These two methods
    /// must be synchronized with respect to each other, for correctness.
    ///
    /// This method will return an `InconsistentAggregator` error if this
    /// `Aggregator` cannot be copied into the destination due to an incompatible
    /// type.
    ///
    /// This call has no `Context` argument because it is expected to perform only
    /// computation.
    fn synchronized_move(
        &self,
        destination: &Arc<dyn Aggregator + Send + Sync>,
        descriptor: &Descriptor,
    ) -> Result<()>;

    /// This combines the checkpointed state from the argument `Aggregator` into this
    /// `Aggregator`. `merge` is not synchronized with respect to `update` or
    /// `synchronized_move`.
    ///
    /// The owner of an `Aggregator` being merged is responsible for synchronization
    /// of both `Aggregator` states.
    fn merge(&self, other: &(dyn Aggregator + Send + Sync), descriptor: &Descriptor) -> Result<()>;

    /// Returns the implementing aggregator as `Any` for downcasting.
    fn as_any(&self) -> &dyn Any;
}

/// An optional interface implemented by some Aggregators. An Aggregator must
/// support `subtract()` in order to be configured for a Precomputed-Sum
/// instrument (SumObserver, UpDownSumObserver) using a DeltaExporter.
pub trait Subtractor {
    /// Subtract subtracts the `operand` from this Aggregator and outputs the value
    /// in `result`.
    fn subtract(
        &self,
        operand: &(dyn Aggregator + Send + Sync),
        result: &(dyn Aggregator + Send + Sync),
        descriptor: &Descriptor,
    ) -> Result<()>;
}

/// Exporter handles presentation of the checkpoint of aggregate metrics. This
/// is the final stage of a metrics export pipeline, where metric data are
/// formatted for a specific system.
pub trait Exporter: ExportKindFor {
    /// Export is called immediately after completing a collection pass in the SDK.
    ///
    /// The CheckpointSet interface refers to the Processor that just completed
    /// collection.
    fn export(&self, checkpoint_set: &mut dyn CheckpointSet) -> Result<()>;
}

/// ExportKindSelector is a sub-interface of Exporter used to indicate
/// whether the Processor should compute Delta or Cumulative
/// Aggregations.
pub trait ExportKindFor: fmt::Debug {
    /// Determines the correct `ExportKind` that should be used when exporting data
    /// for the given metric instrument.
    fn export_kind_for(&self, descriptor: &Descriptor) -> ExportKind;
}

/// CheckpointSet allows a controller to access a complete checkpoint of
/// aggregated metrics from the Processor. This is passed to the `Exporter`
/// which may then use `try_for_each` to iterate over the collection of
/// aggregated metrics.
pub trait CheckpointSet: fmt::Debug {
    /// This iterates over aggregated checkpoints for all metrics that were updated
    /// during the last collection period. Each aggregated checkpoint returned by
    /// the function parameter may return an error.
    ///
    /// The `ExportKindSelector` argument is used to determine whether the `Record`
    /// is computed using delta or cumulative aggregation.
    ///
    /// ForEach tolerates `MetricsError::NoData` silently, as this is expected from
    /// the Meter implementation. Any other kind of error will immediately halt and
    /// return the error to the caller.
    fn try_for_each(
        &mut self,
        export_selector: &dyn ExportKindFor,
        f: &mut dyn FnMut(&Record<'_>) -> Result<()>,
    ) -> Result<()>;
}

/// Allows `Accumulator` implementations to construct new `Accumulation`s to
/// send to `Processor`s. The `Descriptor`, `Labels`, `Resource`, and
/// `Aggregator` represent aggregate metric events received over a single
/// collection period.
pub fn accumulation<'a>(
    descriptor: &'a Descriptor,
    labels: &'a labels::LabelSet,
    resource: &'a Resource,
    aggregator: &'a Arc<dyn Aggregator + Send + Sync>,
) -> Accumulation<'a> {
    Accumulation::new(descriptor, labels, resource, aggregator)
}

/// Allows `Processor` implementations to construct export records. The
/// `Descriptor`, `Labels`, and `Aggregator` represent aggregate metric events
/// received over a single collection period.
pub fn record<'a>(
    descriptor: &'a Descriptor,
    labels: &'a labels::LabelSet,
    resource: &'a Resource,
    aggregator: Option<&'a Arc<dyn Aggregator + Send + Sync>>,
    start: SystemTime,
    end: SystemTime,
) -> Record<'a> {
    Record {
        metadata: Metadata::new(descriptor, labels, resource),
        aggregator,
        start,
        end,
    }
}

impl Record<'_> {
    /// The aggregator for this metric
    pub fn aggregator(&self) -> Option<&Arc<dyn Aggregator + Send + Sync>> {
        self.aggregator
    }
}

/// A container for the common elements for exported metric data that are shared
/// by the `Accumulator`->`Processor` and `Processor`->`Exporter` steps.
#[derive(Debug)]
pub struct Metadata<'a> {
    descriptor: &'a Descriptor,
    labels: &'a labels::LabelSet,
    resource: &'a Resource,
}

impl<'a> Metadata<'a> {
    /// Create a new `Metadata` instance.
    pub fn new(
        descriptor: &'a Descriptor,
        labels: &'a labels::LabelSet,
        resource: &'a Resource,
    ) -> Self {
        {
            Metadata {
                descriptor,
                labels,
                resource,
            }
        }
    }

    /// A description of the metric instrument being exported.
    pub fn descriptor(&self) -> &Descriptor {
        self.descriptor
    }

    /// The labels associated with the instrument and the aggregated data.
    pub fn labels(&self) -> &labels::LabelSet {
        self.labels
    }

    /// Common attributes that apply to this metric event.
    pub fn resource(&self) -> &Resource {
        self.resource
    }
}

/// A container for the exported data for a single metric instrument and label
/// set, as prepared by the `Processor` for the `Exporter`. This includes the
/// effective start and end time for the aggregation.
#[derive(Debug)]
pub struct Record<'a> {
    metadata: Metadata<'a>,
    aggregator: Option<&'a Arc<dyn Aggregator + Send + Sync>>,
    start: SystemTime,
    end: SystemTime,
}

impl Record<'_> {
    /// A description of the metric instrument being exported.
    pub fn descriptor(&self) -> &Descriptor {
        self.metadata.descriptor
    }

    /// The labels associated with the instrument and the aggregated data.
    pub fn labels(&self) -> &labels::LabelSet {
        self.metadata.labels
    }

    /// Common attributes that apply to this metric event.
    pub fn resource(&self) -> &Resource {
        self.metadata.resource
    }

    /// The start time of the interval covered by this aggregation.
    pub fn start_time(&self) -> &SystemTime {
        &self.start
    }

    /// The end time of the interval covered by this aggregation.
    pub fn end_time(&self) -> &SystemTime {
        &self.end
    }
}

/// A container for the exported data for a single metric instrument and label
/// set, as prepared by an `Accumulator` for the `Processor`.
#[derive(Debug)]
pub struct Accumulation<'a> {
    metadata: Metadata<'a>,
    aggregator: &'a Arc<dyn Aggregator + Send + Sync>,
}

impl<'a> Accumulation<'a> {
    /// Create a new `Record` instance.
    pub fn new(
        descriptor: &'a Descriptor,
        labels: &'a labels::LabelSet,
        resource: &'a Resource,
        aggregator: &'a Arc<dyn Aggregator + Send + Sync>,
    ) -> Self {
        Accumulation {
            metadata: Metadata::new(descriptor, labels, resource),
            aggregator,
        }
    }

    /// A description of the metric instrument being exported.
    pub fn descriptor(&self) -> &Descriptor {
        self.metadata.descriptor
    }

    /// The labels associated with the instrument and the aggregated data.
    pub fn labels(&self) -> &labels::LabelSet {
        self.metadata.labels
    }

    /// Common attributes that apply to this metric event.
    pub fn resource(&self) -> &Resource {
        self.metadata.resource
    }

    /// The checkpointed aggregator for this metric.
    pub fn aggregator(&self) -> &Arc<dyn Aggregator + Send + Sync> {
        self.aggregator
    }
}

/// Indicates the kind of data exported by an exporter.
/// These bits may be OR-d together when multiple exporters are in use.
#[derive(Clone, Debug)]
pub enum ExportKind {
    /// Indicates that the `Exporter` expects a cumulative `Aggregation`.
    Cumulative = 1,

    /// Indicates that the `Exporter` expects a delta `Aggregation`.
    Delta = 2,
}

/// Strategies for selecting which export kind is used for an instrument.
#[derive(Debug, Clone)]
pub enum ExportKindSelector {
    /// A selector that always returns [`ExportKind::Cumulative`].
    Cumulative,
    /// A selector that always returns [`ExportKind::Delta`].
    Delta,
    /// A selector that returns cumulative or delta based on a given instrument
    /// kind.
    Stateless,
}

impl ExportKind {
    /// Tests whether `kind` includes a specific kind of exporter.
    pub fn includes(&self, has: &ExportKind) -> bool {
        (self.clone() as u32) & (has.clone() as u32) != 0
    }

    /// Returns whether an exporter of this kind requires memory to export correctly.
    pub fn memory_required(&self, kind: &InstrumentKind) -> bool {
        match kind {
            InstrumentKind::ValueRecorder
            | InstrumentKind::ValueObserver
            | InstrumentKind::Counter
            | InstrumentKind::UpDownCounter => {
                // Delta-oriented instruments:
                self.includes(&ExportKind::Cumulative)
            }

            InstrumentKind::SumObserver | InstrumentKind::UpDownSumObserver => {
                // Cumulative-oriented instruments:
                self.includes(&ExportKind::Delta)
            }
        }
    }
}

impl ExportKindFor for ExportKindSelector {
    fn export_kind_for(&self, descriptor: &Descriptor) -> ExportKind {
        match self {
            ExportKindSelector::Cumulative => ExportKind::Cumulative,
            ExportKindSelector::Delta => ExportKind::Delta,
            ExportKindSelector::Stateless => {
                if descriptor.instrument_kind().precomputed_sum() {
                    ExportKind::Cumulative
                } else {
                    ExportKind::Delta
                }
            }
        }
    }
}
