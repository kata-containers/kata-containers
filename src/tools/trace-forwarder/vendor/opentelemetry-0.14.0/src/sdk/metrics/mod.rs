//! # OpenTelemetry Metrics SDK
use crate::global;
use crate::metrics::{
    sdk_api::{self, InstrumentCore as _, SyncBoundInstrumentCore as _},
    AsyncRunner, AtomicNumber, Descriptor, Measurement, Number, NumberKind, Observation, Result,
};
use crate::sdk::{
    export::{
        self,
        metrics::{Aggregator, LockedProcessor, Processor},
    },
    resource::Resource,
};
use crate::{
    labels::{hash_labels, LabelSet},
    Context, KeyValue,
};
use fnv::FnvHasher;
use std::any::Any;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

pub mod aggregators;
pub mod controllers;
pub mod processors;
pub mod selectors;

pub use controllers::{PullController, PushController, PushControllerWorker};

/// Creates a new accumulator builder
pub fn accumulator(processor: Arc<dyn Processor + Send + Sync>) -> AccumulatorBuilder {
    AccumulatorBuilder {
        processor,
        resource: None,
    }
}

/// Configuration for an accumulator
#[derive(Debug)]
pub struct AccumulatorBuilder {
    processor: Arc<dyn Processor + Send + Sync>,
    resource: Option<Resource>,
}

impl AccumulatorBuilder {
    /// The resource that will be applied to all records in this accumulator.
    pub fn with_resource(self, resource: Resource) -> Self {
        AccumulatorBuilder {
            resource: Some(resource),
            ..self
        }
    }

    /// Create a new accumulator from this configuration
    pub fn build(self) -> Accumulator {
        Accumulator(Arc::new(AccumulatorCore::new(
            self.processor,
            self.resource.unwrap_or_default(),
        )))
    }
}

/// Accumulator implements the OpenTelemetry Meter API. The Accumulator is bound
/// to a single `Processor`.
///
/// The Accumulator supports a collect API to gather and export current data.
/// `Collect` should be arranged according to the processor model. Push-based
/// processors will setup a timer to call `collect` periodically. Pull-based
/// processors will call `collect` when a pull request arrives.
#[derive(Debug, Clone)]
pub struct Accumulator(Arc<AccumulatorCore>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct MapKey {
    instrument_hash: u64,
}

type AsyncRunnerPair = (AsyncRunner, Option<Arc<dyn sdk_api::AsyncInstrumentCore>>);

#[derive(Default, Debug)]
struct AsyncInstrumentState {
    /// The set of runners in the order they were registered that will run each
    /// collection interval.
    ///
    /// Non-batch observers are entered with an instrument, batch observers are
    /// entered without an instrument, each is called once allowing both batch and
    /// individual observations to be collected.
    runners: Vec<AsyncRunnerPair>,

    /// The set of instruments in the order they were registered.
    instruments: Vec<Arc<dyn sdk_api::AsyncInstrumentCore>>,
}

fn collect_async(labels: &[KeyValue], observations: &[Observation]) {
    let labels = LabelSet::from_labels(labels.iter().cloned());

    for observation in observations {
        if let Some(instrument) = observation
            .instrument()
            .as_any()
            .downcast_ref::<AsyncInstrument>()
        {
            instrument.observe(observation.number(), &labels)
        }
    }
}

impl AsyncInstrumentState {
    /// Executes the complete set of observer callbacks.
    fn run(&self) {
        for (runner, instrument) in self.runners.iter() {
            runner.run(instrument, collect_async)
        }
    }
}

#[derive(Debug)]
struct AccumulatorCore {
    /// A concurrent map of current sync instrument state.
    current: dashmap::DashMap<MapKey, Arc<Record>>,
    /// A collection of async instrument state
    async_instruments: Mutex<AsyncInstrumentState>,

    /// The current epoch number. It is incremented in `collect`.
    current_epoch: AtomicNumber,
    /// The configured processor.
    processor: Arc<dyn Processor + Send + Sync>,
    /// The resource applied to all records in this Accumulator.
    resource: Resource,
}

impl AccumulatorCore {
    fn new(processor: Arc<dyn Processor + Send + Sync>, resource: Resource) -> Self {
        AccumulatorCore {
            current: dashmap::DashMap::new(),
            async_instruments: Mutex::new(AsyncInstrumentState::default()),
            current_epoch: NumberKind::U64.zero().to_atomic(),
            processor,
            resource,
        }
    }

    fn register(
        &self,
        instrument: Arc<dyn sdk_api::AsyncInstrumentCore>,
        runner: Option<AsyncRunner>,
    ) -> Result<()> {
        self.async_instruments
            .lock()
            .map_err(Into::into)
            .map(|mut async_instruments| {
                if let Some(runner) = runner {
                    async_instruments
                        .runners
                        .push((runner, Some(instrument.clone())));
                };
                async_instruments.instruments.push(instrument);
            })
    }

    fn register_runner(&self, runner: AsyncRunner) -> Result<()> {
        self.async_instruments
            .lock()
            .map_err(Into::into)
            .map(|mut async_instruments| async_instruments.runners.push((runner, None)))
    }

    fn collect(&self, locked_processor: &mut dyn LockedProcessor) -> usize {
        let mut checkpointed = self.observe_async_instruments(locked_processor);
        checkpointed += self.collect_sync_instruments(locked_processor);
        self.current_epoch.fetch_add(&NumberKind::U64, &1u64.into());

        checkpointed
    }

    fn observe_async_instruments(&self, locked_processor: &mut dyn LockedProcessor) -> usize {
        self.async_instruments
            .lock()
            .map_or(0, |async_instruments| {
                let mut async_collected = 0;

                async_instruments.run();

                for instrument in &async_instruments.instruments {
                    if let Some(a) = instrument.as_any().downcast_ref::<AsyncInstrument>() {
                        async_collected += self.checkpoint_async(a, locked_processor);
                    }
                }

                async_collected
            })
    }

    fn collect_sync_instruments(&self, locked_processor: &mut dyn LockedProcessor) -> usize {
        let mut checkpointed = 0;

        self.current.retain(|_key, value| {
            let mods = &value.update_count.load();
            let coll = &value.collected_count.load();

            if mods.partial_cmp(&NumberKind::U64, coll) != Some(Ordering::Equal) {
                // Updates happened in this interval,
                // checkpoint and continue.
                checkpointed += self.checkpoint_record(value, locked_processor);
                value.collected_count.store(mods);
            } else {
                // Having no updates since last collection, try to remove if
                // there are no bound handles
                if Arc::strong_count(&value) == 1 {
                    // There's a potential race between loading collected count and
                    // loading the strong count in this function.  Since this is the
                    // last we'll see of this record, checkpoint.
                    if mods.partial_cmp(&NumberKind::U64, coll) != Some(Ordering::Equal) {
                        checkpointed += self.checkpoint_record(value, locked_processor);
                    }
                    return false;
                }
            };
            true
        });

        checkpointed
    }

    fn checkpoint_record(
        &self,
        record: &Record,
        locked_processor: &mut dyn LockedProcessor,
    ) -> usize {
        if let (Some(current), Some(checkpoint)) = (&record.current, &record.checkpoint) {
            if let Err(err) = current.synchronized_move(checkpoint, record.instrument.descriptor())
            {
                global::handle_error(err);

                return 0;
            }

            let accumulation = export::metrics::accumulation(
                record.instrument.descriptor(),
                &record.labels,
                &self.resource,
                &checkpoint,
            );
            if let Err(err) = locked_processor.process(accumulation) {
                global::handle_error(err);
            }

            1
        } else {
            0
        }
    }

    fn checkpoint_async(
        &self,
        instrument: &AsyncInstrument,
        locked_processor: &mut dyn LockedProcessor,
    ) -> usize {
        instrument.recorders.lock().map_or(0, |mut recorders| {
            let mut checkpointed = 0;
            match recorders.as_mut() {
                None => return checkpointed,
                Some(recorders) => {
                    recorders.retain(|_key, label_recorder| {
                        let epoch_diff = self
                            .current_epoch
                            .load()
                            .partial_cmp(&NumberKind::U64, &label_recorder.observed_epoch.into());
                        if epoch_diff == Some(Ordering::Equal) {
                            if let Some(observed) = &label_recorder.observed {
                                let accumulation = export::metrics::accumulation(
                                    instrument.descriptor(),
                                    &label_recorder.labels,
                                    &self.resource,
                                    observed,
                                );

                                if let Err(err) = locked_processor.process(accumulation) {
                                    global::handle_error(err);
                                }
                                checkpointed += 1;
                            }
                        }

                        // Retain if this is not second collection cycle with no
                        // observations for this labelset.
                        epoch_diff == Some(Ordering::Greater)
                    });
                }
            }
            if recorders.as_ref().map_or(false, |map| map.is_empty()) {
                *recorders = None;
            }

            checkpointed
        })
    }
}

#[derive(Debug, Clone)]
struct SyncInstrument {
    instrument: Arc<Instrument>,
}

impl SyncInstrument {
    fn acquire_handle(&self, labels: &[KeyValue]) -> Arc<Record> {
        let mut hasher = FnvHasher::default();
        self.instrument
            .descriptor
            .attribute_hash()
            .hash(&mut hasher);

        hash_labels(&mut hasher, labels.iter().map(|kv| (&kv.key, &kv.value)));

        let map_key = MapKey {
            instrument_hash: hasher.finish(),
        };
        let current = &self.instrument.meter.0.current;
        if let Some(existing_record) = current.get(&map_key) {
            return existing_record.value().clone();
        }

        let record = Arc::new(Record {
            update_count: NumberKind::U64.zero().to_atomic(),
            collected_count: NumberKind::U64.zero().to_atomic(),
            labels: LabelSet::from_labels(labels.iter().cloned()),
            instrument: self.clone(),
            current: self
                .instrument
                .meter
                .0
                .processor
                .aggregation_selector()
                .aggregator_for(&self.instrument.descriptor),
            checkpoint: self
                .instrument
                .meter
                .0
                .processor
                .aggregation_selector()
                .aggregator_for(&self.instrument.descriptor),
        });
        current.insert(map_key, record.clone());

        record
    }
}

impl sdk_api::InstrumentCore for SyncInstrument {
    fn descriptor(&self) -> &Descriptor {
        self.instrument.descriptor()
    }
}

impl sdk_api::SyncInstrumentCore for SyncInstrument {
    fn bind(&self, labels: &'_ [KeyValue]) -> Arc<dyn sdk_api::SyncBoundInstrumentCore> {
        self.acquire_handle(labels)
    }
    fn record_one(&self, number: Number, labels: &'_ [KeyValue]) {
        let handle = self.acquire_handle(labels);
        handle.record_one(number)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct LabeledRecorder {
    observed_epoch: u64,
    labels: LabelSet,
    observed: Option<Arc<dyn Aggregator + Send + Sync>>,
}

#[derive(Debug, Clone)]
struct AsyncInstrument {
    instrument: Arc<Instrument>,
    recorders: Arc<Mutex<Option<HashMap<u64, LabeledRecorder>>>>,
}

impl AsyncInstrument {
    fn observe(&self, number: &Number, labels: &LabelSet) {
        if let Err(err) = aggregators::range_test(number, &self.instrument.descriptor) {
            global::handle_error(err);
        }
        if let Some(recorder) = self.get_recorder(labels) {
            if let Err(err) = recorder.update(number, &self.instrument.descriptor) {
                global::handle_error(err)
            }
        }
    }

    fn get_recorder(&self, labels: &LabelSet) -> Option<Arc<dyn Aggregator + Send + Sync>> {
        self.recorders.lock().map_or(None, |mut recorders| {
            let mut hasher = FnvHasher::default();
            hash_labels(&mut hasher, labels.into_iter());
            let label_hash = hasher.finish();
            if let Some(recorder) = recorders.as_mut().and_then(|rec| rec.get_mut(&label_hash)) {
                let current_epoch = self
                    .instrument
                    .meter
                    .0
                    .current_epoch
                    .load()
                    .to_u64(&NumberKind::U64);
                if recorder.observed_epoch == current_epoch {
                    // last value wins for Observers, so if we see the same labels
                    // in the current epoch, we replace the old recorder
                    return self
                        .instrument
                        .meter
                        .0
                        .processor
                        .aggregation_selector()
                        .aggregator_for(&self.instrument.descriptor);
                } else {
                    recorder.observed_epoch = current_epoch;
                }
                return recorder.observed.clone();
            }

            let recorder = self
                .instrument
                .meter
                .0
                .processor
                .aggregation_selector()
                .aggregator_for(&self.instrument.descriptor);
            if recorders.is_none() {
                *recorders = Some(HashMap::new());
            }
            // This may store a recorder with no aggregator in the map, thus disabling the
            // async_instrument for the LabelSet for good. This is intentional, but will be
            // revisited later.
            let observed_epoch = self
                .instrument
                .meter
                .0
                .current_epoch
                .load()
                .to_u64(&NumberKind::U64);
            recorders.as_mut().unwrap().insert(
                label_hash,
                LabeledRecorder {
                    observed: recorder.clone(),
                    labels: labels.clone(),
                    observed_epoch,
                },
            );

            recorder
        })
    }
}

impl sdk_api::InstrumentCore for AsyncInstrument {
    fn descriptor(&self) -> &Descriptor {
        self.instrument.descriptor()
    }
}

impl sdk_api::AsyncInstrumentCore for AsyncInstrument {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// record maintains the state of one metric instrument.  Due
/// the use of lock-free algorithms, there may be more than one
/// `record` in existence at a time, although at most one can
/// be referenced from the `Accumulator.current` map.
#[derive(Debug)]
struct Record {
    /// Incremented on every call to `update`.
    update_count: AtomicNumber,

    /// Set to `update_count` on collection, supports checking for no updates during
    /// a round.
    collected_count: AtomicNumber,

    /// The processed label set for this record.
    ///
    /// TODO: look at perf here.
    labels: LabelSet,

    /// The corresponding instrument.
    instrument: SyncInstrument,

    /// current implements the actual `record_one` API, depending on the type of
    /// aggregation. If `None`, the metric was disabled by the exporter.
    current: Option<Arc<dyn Aggregator + Send + Sync>>,
    checkpoint: Option<Arc<dyn Aggregator + Send + Sync>>,
}

impl sdk_api::SyncBoundInstrumentCore for Record {
    fn record_one<'a>(&self, number: Number) {
        // check if the instrument is disabled according to the AggregatorSelector.
        if let Some(recorder) = &self.current {
            if let Err(err) =
                aggregators::range_test(&number, &self.instrument.instrument.descriptor)
                    .and_then(|_| recorder.update(&number, &self.instrument.instrument.descriptor))
            {
                global::handle_error(err);
                return;
            }

            // Record was modified, inform the collect() that things need
            // to be collected while the record is still mapped.
            self.update_count.fetch_add(&NumberKind::U64, &1u64.into());
        }
    }
}

#[derive(Debug)]
struct Instrument {
    descriptor: Descriptor,
    meter: Accumulator,
}

impl sdk_api::InstrumentCore for Instrument {
    fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
}

impl sdk_api::MeterCore for Accumulator {
    fn new_sync_instrument(
        &self,
        descriptor: Descriptor,
    ) -> Result<Arc<dyn sdk_api::SyncInstrumentCore>> {
        Ok(Arc::new(SyncInstrument {
            instrument: Arc::new(Instrument {
                descriptor,
                meter: self.clone(),
            }),
        }))
    }

    fn record_batch_with_context(
        &self,
        _cx: &Context,
        labels: &[KeyValue],
        measurements: Vec<Measurement>,
    ) {
        for measure in measurements.into_iter() {
            if let Some(instrument) = measure
                .instrument()
                .as_any()
                .downcast_ref::<SyncInstrument>()
            {
                let handle = instrument.acquire_handle(labels);

                handle.record_one(measure.into_number());
            }
        }
    }

    fn new_async_instrument(
        &self,
        descriptor: Descriptor,
        runner: Option<AsyncRunner>,
    ) -> Result<Arc<dyn sdk_api::AsyncInstrumentCore>> {
        let instrument = Arc::new(AsyncInstrument {
            instrument: Arc::new(Instrument {
                descriptor,
                meter: self.clone(),
            }),
            recorders: Arc::new(Mutex::new(None)),
        });

        self.0.register(instrument.clone(), runner)?;

        Ok(instrument)
    }

    fn new_batch_observer(&self, runner: AsyncRunner) -> Result<()> {
        self.0.register_runner(runner)
    }
}
