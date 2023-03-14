use crate::metrics::{AtomicNumber, Descriptor, MetricsError, Number, NumberKind, Result};
use crate::sdk::export::metrics::{Buckets, Count, Histogram, Sum};
use crate::sdk::metrics::export::metrics::Aggregator;
use std::mem;
use std::sync::{Arc, RwLock};

/// Create a new histogram for the given descriptor with the given boundaries
pub fn histogram(desc: &Descriptor, boundaries: &[f64]) -> HistogramAggregator {
    let mut sorted_boundaries = boundaries.to_owned();
    sorted_boundaries.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let state = State::empty(&sorted_boundaries);

    HistogramAggregator {
        inner: RwLock::new(Inner {
            boundaries: sorted_boundaries,
            kind: desc.number_kind().clone(),
            state,
        }),
    }
}

/// This aggregator observes events and counts them in pre-determined buckets. It
/// also calculates the sum and count of all events.
#[derive(Debug)]
pub struct HistogramAggregator {
    inner: RwLock<Inner>,
}

#[derive(Debug)]
struct Inner {
    boundaries: Vec<f64>,
    kind: NumberKind,
    state: State,
}

#[derive(Debug)]
struct State {
    bucket_counts: Vec<f64>,
    count: AtomicNumber,
    sum: AtomicNumber,
}

impl State {
    fn empty(boundaries: &[f64]) -> Self {
        State {
            bucket_counts: vec![0.0; boundaries.len() + 1],
            count: NumberKind::U64.zero().to_atomic(),
            sum: NumberKind::U64.zero().to_atomic(),
        }
    }
}

impl Sum for HistogramAggregator {
    fn sum(&self) -> Result<Number> {
        self.inner
            .read()
            .map_err(From::from)
            .map(|inner| inner.state.sum.load())
    }
}

impl Count for HistogramAggregator {
    fn count(&self) -> Result<u64> {
        self.inner
            .read()
            .map_err(From::from)
            .map(|inner| inner.state.count.load().to_u64(&NumberKind::U64))
    }
}

impl Histogram for HistogramAggregator {
    fn histogram(&self) -> Result<Buckets> {
        self.inner
            .read()
            .map_err(From::from)
            .map(|inner| Buckets::new(inner.boundaries.clone(), inner.state.bucket_counts.clone()))
    }
}

impl Aggregator for HistogramAggregator {
    fn update(&self, number: &Number, descriptor: &Descriptor) -> Result<()> {
        self.inner.write().map_err(From::from).map(|mut inner| {
            let kind = descriptor.number_kind();
            let as_float = number.to_f64(kind);

            let mut bucket_id = inner.boundaries.len();
            for (idx, boundary) in inner.boundaries.iter().enumerate() {
                if as_float < *boundary {
                    bucket_id = idx;
                    break;
                }
            }

            inner.state.count.fetch_add(&NumberKind::U64, &1u64.into());
            inner.state.sum.fetch_add(kind, number);
            inner.state.bucket_counts[bucket_id] += 1.0;
        })
    }

    fn synchronized_move(
        &self,
        other: &Arc<dyn Aggregator + Send + Sync>,
        _descriptor: &crate::metrics::Descriptor,
    ) -> Result<()> {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            self.inner
                .write()
                .map_err(From::from)
                .and_then(|mut inner| {
                    other.inner.write().map_err(From::from).map(|mut other| {
                        let empty = State::empty(&inner.boundaries);
                        other.state = mem::replace(&mut inner.state, empty)
                    })
                })
        } else {
            Err(MetricsError::InconsistentAggregator(format!(
                "Expected {:?}, got: {:?}",
                self, other
            )))
        }
    }

    fn merge(&self, other: &(dyn Aggregator + Send + Sync), desc: &Descriptor) -> Result<()> {
        if let Some(other) = other.as_any().downcast_ref::<HistogramAggregator>() {
            self.inner
                .write()
                .map_err(From::from)
                .and_then(|mut inner| {
                    other.inner.read().map_err(From::from).map(|other| {
                        inner
                            .state
                            .sum
                            .fetch_add(desc.number_kind(), &other.state.sum.load());
                        inner
                            .state
                            .count
                            .fetch_add(&NumberKind::U64, &other.state.count.load());

                        for idx in 0..inner.state.bucket_counts.len() {
                            inner.state.bucket_counts[idx] += other.state.bucket_counts[idx];
                        }
                    })
                })
        } else {
            Err(MetricsError::InconsistentAggregator(format!(
                "Expected {:?}, got: {:?}",
                self, other
            )))
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
