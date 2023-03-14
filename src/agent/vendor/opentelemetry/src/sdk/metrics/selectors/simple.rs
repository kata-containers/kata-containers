//! Simple Metric Selectors
use crate::metrics::{Descriptor, InstrumentKind};
use crate::sdk::export::metrics::{Aggregator, AggregatorSelector};
use crate::sdk::metrics::aggregators;
use std::sync::Arc;

/// Aggregation selection strategies.
#[derive(Debug)]
pub enum Selector {
    /// A simple aggregation selector that uses counter, ddsketch, and ddsketch
    /// aggregators for the three kinds of metric.  This selector uses more cpu
    /// and memory than the NewWithInexpensiveDistribution because it uses one
    /// DDSketch per distinct instrument and label set.
    Sketch(aggregators::DdSketchConfig),
    /// A simple aggregation selector that uses last_value, sum, and
    /// minmaxsumcount aggregators for metrics. This selector is faster and uses
    /// less memory than the others because minmaxsumcount does not aggregate
    /// quantile information.
    Inexpensive,
    /// A simple aggregation selector that uses sum and array aggregators for
    /// metrics. This selector is able to compute exact quantiles.
    Exact,
    /// A simple aggregation selector that uses sum, and histogram aggregators
    /// for metrics. This selector uses more memory than `Inexpensive` because
    /// it uses a counter per bucket.
    Histogram(Vec<f64>),
}

impl AggregatorSelector for Selector {
    fn aggregator_for(&self, descriptor: &Descriptor) -> Option<Arc<dyn Aggregator + Send + Sync>> {
        match self {
            Selector::Sketch(config) => match descriptor.instrument_kind() {
                InstrumentKind::ValueObserver => Some(Arc::new(aggregators::last_value())),
                InstrumentKind::ValueRecorder => Some(Arc::new(aggregators::ddsketch(
                    config,
                    descriptor.number_kind().clone(),
                ))),
                _ => Some(Arc::new(aggregators::sum())),
            },
            Selector::Inexpensive => match descriptor.instrument_kind() {
                InstrumentKind::ValueObserver => Some(Arc::new(aggregators::last_value())),
                InstrumentKind::ValueRecorder => {
                    Some(Arc::new(aggregators::min_max_sum_count(descriptor)))
                }
                _ => Some(Arc::new(aggregators::sum())),
            },
            Selector::Exact => match descriptor.instrument_kind() {
                InstrumentKind::ValueObserver => Some(Arc::new(aggregators::last_value())),
                InstrumentKind::ValueRecorder => Some(Arc::new(aggregators::array())),
                _ => Some(Arc::new(aggregators::sum())),
            },
            Selector::Histogram(boundaries) => match descriptor.instrument_kind() {
                InstrumentKind::ValueObserver => Some(Arc::new(aggregators::last_value())),
                InstrumentKind::ValueRecorder => {
                    Some(Arc::new(aggregators::histogram(descriptor, boundaries)))
                }
                _ => Some(Arc::new(aggregators::sum())),
            },
        }
    }
}
