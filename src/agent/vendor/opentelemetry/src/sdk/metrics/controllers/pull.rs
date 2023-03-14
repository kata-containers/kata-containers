use crate::metrics::{registry, Result};
use crate::sdk::{
    export::metrics::{AggregatorSelector, CheckpointSet, Checkpointer, ExportKindFor, Record},
    metrics::{
        accumulator,
        processors::{self, BasicProcessor},
        Accumulator,
    },
    Resource,
};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

const DEFAULT_CACHE_DURATION: Duration = Duration::from_secs(10);

/// Returns a builder for creating a `PullController` with the configured and options.
pub fn pull(
    aggregator_selector: Box<dyn AggregatorSelector + Send + Sync>,
    export_selector: Box<dyn ExportKindFor + Send + Sync>,
) -> PullControllerBuilder {
    PullControllerBuilder::with_selectors(aggregator_selector, export_selector)
}

/// Pull controllers are typically used in an environment where there are
/// multiple readers. It is common, therefore, when configuring a
/// `BasicProcessor` for use with this controller, to use a
/// `ExportKind::Cumulative` strategy and the `with_memory(true)` builder
/// option, which ensures that every `CheckpointSet` includes full state.
#[derive(Debug)]
pub struct PullController {
    accumulator: Accumulator,
    processor: Arc<BasicProcessor>,
    provider: registry::RegistryMeterProvider,
    period: Duration,
    last_collect: SystemTime,
}

impl PullController {
    /// The provider for this controller
    pub fn provider(&self) -> registry::RegistryMeterProvider {
        self.provider.clone()
    }

    /// Collects all metrics if the last collected at time is past the current period
    pub fn collect(&mut self) -> Result<()> {
        if self
            .last_collect
            .elapsed()
            .map_or(true, |elapsed| elapsed > self.period)
        {
            self.last_collect = crate::time::now();
            self.processor.lock().and_then(|mut checkpointer| {
                checkpointer.start_collection();
                self.accumulator.0.collect(&mut checkpointer);
                checkpointer.finish_collection()
            })
        } else {
            Ok(())
        }
    }
}

impl CheckpointSet for PullController {
    fn try_for_each(
        &mut self,
        export_selector: &dyn ExportKindFor,
        f: &mut dyn FnMut(&Record<'_>) -> Result<()>,
    ) -> Result<()> {
        self.processor.lock().and_then(|mut locked_processor| {
            locked_processor
                .checkpoint_set()
                .try_for_each(export_selector, f)
        })
    }
}

/// Configuration for a `PullController`.
#[derive(Debug)]
pub struct PullControllerBuilder {
    /// The aggregator selector used by the controller
    aggregator_selector: Box<dyn AggregatorSelector + Send + Sync>,

    /// The export kind selector used by this controller
    export_selector: Box<dyn ExportKindFor + Send + Sync>,

    /// Resource is the OpenTelemetry resource associated with all Meters created by
    /// the controller.
    resource: Option<Resource>,

    /// CachePeriod is the period which a recently-computed result will be returned
    /// without gathering metric data again.
    ///
    /// If the period is zero, caching of the result is disabled. The default value
    /// is 10 seconds.
    cache_period: Option<Duration>,

    /// Memory controls whether the controller's processor remembers metric
    /// instruments and label sets that were previously reported. When memory is
    /// `true`, `CheckpointSet::try_for_each` will visit metrics that were not
    /// updated in the most recent interval. Default true.
    memory: bool,
}

impl PullControllerBuilder {
    /// Configure the sectors for this controller
    pub fn with_selectors(
        aggregator_selector: Box<dyn AggregatorSelector + Send + Sync>,
        export_selector: Box<dyn ExportKindFor + Send + Sync>,
    ) -> Self {
        PullControllerBuilder {
            aggregator_selector,
            export_selector,
            resource: None,
            cache_period: None,
            memory: true,
        }
    }

    /// Configure the resource for this controller
    pub fn with_resource(self, resource: Resource) -> Self {
        PullControllerBuilder {
            resource: Some(resource),
            ..self
        }
    }

    /// Configure the cache period for this controller
    pub fn with_cache_period(self, period: Duration) -> Self {
        PullControllerBuilder {
            cache_period: Some(period),
            ..self
        }
    }

    /// Sets the memory behavior of the controller's `Processor`.  If this is
    /// `true`, the processor will report metric instruments and label sets that
    /// were previously reported but not updated in the most recent interval.
    pub fn with_memory(self, memory: bool) -> Self {
        PullControllerBuilder { memory, ..self }
    }

    /// Build a new `PullController` from the current configuration.
    pub fn build(self) -> PullController {
        let processor = Arc::new(processors::basic(
            self.aggregator_selector,
            self.export_selector,
            self.memory,
        ));

        let accumulator = accumulator(processor.clone())
            .with_resource(self.resource.unwrap_or_default())
            .build();
        let provider = registry::meter_provider(Arc::new(accumulator.clone()));

        PullController {
            accumulator,
            processor,
            provider,
            period: self.cache_period.unwrap_or(DEFAULT_CACHE_DURATION),
            last_collect: crate::time::now(),
        }
    }
}
