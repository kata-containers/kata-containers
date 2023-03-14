use crate::sdk::{
    export::metrics::{
        self, Accumulation, Aggregator, AggregatorSelector, CheckpointSet, Checkpointer,
        ExportKind, ExportKindFor, LockedProcessor, Processor, Record, Subtractor,
    },
    metrics::aggregators::SumAggregator,
    Resource,
};
use crate::{
    labels::{hash_labels, LabelSet},
    metrics::{Descriptor, MetricsError, Result},
};
use fnv::FnvHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::SystemTime;

/// Create a new basic processor
pub fn basic(
    aggregator_selector: Box<dyn AggregatorSelector + Send + Sync>,
    export_selector: Box<dyn ExportKindFor + Send + Sync>,
    memory: bool,
) -> BasicProcessor {
    BasicProcessor {
        aggregator_selector,
        export_selector,
        state: Mutex::new(BasicProcessorState::with_memory(memory)),
    }
}

/// Basic metric integration strategy
#[derive(Debug)]
pub struct BasicProcessor {
    aggregator_selector: Box<dyn AggregatorSelector + Send + Sync>,
    export_selector: Box<dyn ExportKindFor + Send + Sync>,
    state: Mutex<BasicProcessorState>,
}

impl BasicProcessor {
    /// Lock this processor to return a mutable locked processor
    pub fn lock(&self) -> Result<BasicLockedProcessor<'_>> {
        self.state
            .lock()
            .map_err(From::from)
            .map(|locked| BasicLockedProcessor {
                parent: self,
                state: locked,
            })
    }
}

impl Processor for BasicProcessor {
    fn aggregation_selector(&self) -> &dyn AggregatorSelector {
        self.aggregator_selector.as_ref()
    }
}

/// A locked representation of the processor used where mutable references are necessary.
#[derive(Debug)]
pub struct BasicLockedProcessor<'a> {
    parent: &'a BasicProcessor,
    state: MutexGuard<'a, BasicProcessorState>,
}

impl<'a> LockedProcessor for BasicLockedProcessor<'a> {
    fn process(&mut self, accumulation: Accumulation<'_>) -> Result<()> {
        if self.state.started_collection != self.state.finished_collection.wrapping_add(1) {
            return Err(MetricsError::InconsistentState);
        }

        let desc = accumulation.descriptor();
        let mut hasher = FnvHasher::default();
        desc.attribute_hash().hash(&mut hasher);
        hash_labels(&mut hasher, accumulation.labels().into_iter());
        hash_labels(&mut hasher, accumulation.resource().into_iter());
        let key = StateKey(hasher.finish());
        let agg = accumulation.aggregator();
        let finished_collection = self.state.finished_collection;
        if let Some(value) = self.state.values.get_mut(&key) {
            // Advance the update sequence number.
            let same_collection = finished_collection == value.updated;
            value.updated = finished_collection;

            // At this point in the code, we have located an existing
            // value for some stateKey.  This can be because:
            //
            // (a) stateful aggregation is being used, the entry was
            // entered during a prior collection, and this is the first
            // time processing an accumulation for this stateKey in the
            // current collection.  Since this is the first time
            // processing an accumulation for this stateKey during this
            // collection, we don't know yet whether there are multiple
            // accumulators at work.  If there are multiple accumulators,
            // they'll hit case (b) the second time through.
            //
            // (b) multiple accumulators are being used, whether stateful
            // or not.
            //
            // Case (a) occurs when the instrument and the exporter
            // require memory to work correctly, either because the
            // instrument reports a PrecomputedSum to a DeltaExporter or
            // the reverse, a non-PrecomputedSum instrument with a
            // CumulativeExporter.  This logic is encapsulated in
            // ExportKind.MemoryRequired(MetricKind).
            //
            // Case (b) occurs when the variable `sameCollection` is true,
            // indicating that the stateKey for Accumulation has already
            // been seen in the same collection.  When this happens, it
            // implies that multiple Accumulators are being used, or that
            // a single Accumulator has been configured with a label key
            // filter.

            if !same_collection {
                if !value.current_owned {
                    // This is the first Accumulation we've seen for this
                    // stateKey during this collection.  Just keep a
                    // reference to the Accumulator's Aggregator. All the other cases
                    // copy Aggregator state.
                    value.current = agg.clone();
                    return Ok(());
                }
                return agg.synchronized_move(&value.current, desc);
            }

            // If the current is not owned, take ownership of a copy
            // before merging below.
            if !value.current_owned {
                let tmp = value.current.clone();
                if let Some(current) = self.parent.aggregation_selector().aggregator_for(desc) {
                    value.current = current;
                    value.current_owned = true;
                    tmp.synchronized_move(&value.current, &desc)?;
                }
            }

            // Combine this `Accumulation` with the prior `Accumulation`.
            return value.current.merge(agg.as_ref(), desc);
        }

        let stateful = self
            .parent
            .export_selector
            .export_kind_for(&desc)
            .memory_required(desc.instrument_kind());

        let mut delta = None;
        let cumulative = if stateful {
            if desc.instrument_kind().precomputed_sum() {
                // If we know we need to compute deltas, allocate one.
                delta = self.parent.aggregation_selector().aggregator_for(desc);
            }
            // Always allocate a cumulative aggregator if stateful
            self.parent.aggregation_selector().aggregator_for(desc)
        } else {
            None
        };

        self.state.values.insert(
            key,
            StateValue {
                descriptor: desc.clone(),
                labels: accumulation.labels().clone(),
                resource: accumulation.resource().clone(),
                current_owned: false,
                current: agg.clone(),
                delta,
                cumulative,
                stateful,
                updated: finished_collection,
            },
        );

        Ok(())
    }
}

impl Checkpointer for BasicLockedProcessor<'_> {
    fn checkpoint_set(&mut self) -> &mut dyn CheckpointSet {
        &mut *self.state
    }

    fn start_collection(&mut self) {
        if self.state.started_collection != 0 {
            self.state.interval_start = self.state.interval_end;
        }
        self.state.started_collection = self.state.started_collection.wrapping_add(1);
    }

    fn finish_collection(&mut self) -> Result<()> {
        self.state.interval_end = crate::time::now();
        if self.state.started_collection != self.state.finished_collection.wrapping_add(1) {
            return Err(MetricsError::InconsistentState);
        }
        let finished_collection = self.state.finished_collection;
        self.state.finished_collection = self.state.finished_collection.wrapping_add(1);
        let has_memory = self.state.config.memory;

        let mut result = Ok(());

        self.state.values.retain(|_key, value| {
            // Return early if previous error
            if result.is_err() {
                return true;
            }

            let mkind = value.descriptor.instrument_kind();

            let stale = value.updated != finished_collection;
            let stateless = !value.stateful;

            // The following branch updates stateful aggregators. Skip these updates
            // if the aggregator is not stateful or if the aggregator is stale.
            if stale || stateless {
                // If this processor does not require memory, stale, stateless
                // entries can be removed. This implies that they were not updated
                // over the previous full collection interval.
                if stale && stateless && !has_memory {
                    return false;
                }
                return true;
            }

            // Update Aggregator state to support exporting either a
            // delta or a cumulative aggregation.
            if mkind.precomputed_sum() {
                if let Some(current_subtractor) =
                    value.current.as_any().downcast_ref::<SumAggregator>()
                {
                    // This line is equivalent to:
                    // value.delta = currentSubtractor - value.cumulative
                    if let (Some(cumulative), Some(delta)) =
                        (value.cumulative.as_ref(), value.delta.as_ref())
                    {
                        result = current_subtractor
                            .subtract(cumulative.as_ref(), delta.as_ref(), &value.descriptor)
                            .and_then(|_| {
                                value
                                    .current
                                    .synchronized_move(cumulative, &value.descriptor)
                            });
                    }
                } else {
                    result = Err(MetricsError::NoSubtraction);
                }
            } else {
                // This line is equivalent to:
                // value.cumulative = value.cumulative + value.delta
                if let Some(cumulative) = value.cumulative.as_ref() {
                    result = cumulative.merge(value.current.as_ref(), &value.descriptor)
                }
            }

            true
        });

        result
    }
}

#[derive(Debug, Default)]
struct BasicProcessorConfig {
    /// Memory controls whether the processor remembers metric instruments and label
    /// sets that were previously reported. When Memory is true,
    /// `CheckpointSet::try_for_each` will visit metrics that were not updated in
    /// the most recent interval.
    memory: bool,
}

#[derive(Debug)]
struct BasicProcessorState {
    config: BasicProcessorConfig,
    values: HashMap<StateKey, StateValue>,
    // Note: the timestamp logic currently assumes all exports are deltas.
    process_start: SystemTime,
    interval_start: SystemTime,
    interval_end: SystemTime,
    started_collection: u64,
    finished_collection: u64,
}

impl BasicProcessorState {
    fn with_memory(memory: bool) -> Self {
        let mut state = BasicProcessorState::default();
        state.config.memory = memory;
        state
    }
}

impl Default for BasicProcessorState {
    fn default() -> Self {
        BasicProcessorState {
            config: BasicProcessorConfig::default(),
            values: HashMap::default(),
            process_start: crate::time::now(),
            interval_start: crate::time::now(),
            interval_end: crate::time::now(),
            started_collection: 0,
            finished_collection: 0,
        }
    }
}

impl CheckpointSet for BasicProcessorState {
    fn try_for_each(
        &mut self,
        exporter: &dyn ExportKindFor,
        f: &mut dyn FnMut(&Record<'_>) -> Result<()>,
    ) -> Result<()> {
        if self.started_collection != self.finished_collection {
            return Err(MetricsError::InconsistentState);
        }

        self.values.iter().try_for_each(|(_key, value)| {
            let instrument_kind = value.descriptor.instrument_kind();

            let agg;
            let start;

            // If the processor does not have memory and it was not updated in the
            // prior round, do not visit this value.
            if !self.config.memory && value.updated != self.finished_collection.wrapping_sub(1) {
                return Ok(());
            }

            match exporter.export_kind_for(&value.descriptor) {
                ExportKind::Cumulative => {
                    // If stateful, the sum has been computed.  If stateless, the
                    // input was already cumulative. Either way, use the
                    // checkpointed value:
                    if value.stateful {
                        agg = value.cumulative.as_ref();
                    } else {
                        agg = Some(&value.current);
                    }

                    start = self.process_start;
                }

                ExportKind::Delta => {
                    // Precomputed sums are a special case.
                    if instrument_kind.precomputed_sum() {
                        agg = value.delta.as_ref();
                    } else {
                        agg = Some(&value.current);
                    }

                    start = self.interval_start;
                }
            }

            let res = f(&metrics::record(
                &value.descriptor,
                &value.labels,
                &value.resource,
                agg,
                start,
                self.interval_end,
            ));
            if let Err(MetricsError::NoDataCollected) = res {
                Ok(())
            } else {
                res
            }
        })
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct StateKey(u64);

#[derive(Debug)]
struct StateValue {
    /// Instrument descriptor
    descriptor: Descriptor,

    /// Instrument labels
    labels: LabelSet,

    /// Resource that created the instrument
    resource: Resource,

    /// Indicates the last sequence number when this value had process called by an
    /// accumulator.
    updated: u64,

    /// Indicates that a cumulative aggregation is being maintained, taken from the
    /// process start time.
    stateful: bool,

    /// Indicates that "current" was allocated
    /// by the processor in order to merge results from
    /// multiple `Accumulator`s during a single collection
    /// round, which may happen either because:
    ///
    /// (1) multiple `Accumulator`s output the same `Accumulation.
    /// (2) one `Accumulator` is configured with dimensionality reduction.
    current_owned: bool,

    /// The output from a single `Accumulator` (if !current_owned) or an
    /// `Aggregator` owned by the processor used to accumulate multiple values in a
    /// single collection round.
    current: Arc<dyn Aggregator + Send + Sync>,

    /// If `Some`, refers to an `Aggregator` owned by the processor used to compute
    /// deltas between precomputed sums.
    delta: Option<Arc<dyn Aggregator + Send + Sync>>,

    /// If `Some`, refers to an `Aggregator` owned by the processor used to store
    /// the last cumulative value.
    cumulative: Option<Arc<dyn Aggregator + Send + Sync>>,
}
