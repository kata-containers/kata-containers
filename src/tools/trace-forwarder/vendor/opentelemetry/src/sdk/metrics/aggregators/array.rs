use crate::metrics::{AtomicNumber, Descriptor, MetricsError, Number, NumberKind, Result};
use crate::sdk::{
    export::metrics::{Count, Points},
    metrics::Aggregator,
};
use std::any::Any;
use std::mem;
use std::sync::{Arc, Mutex};

/// Create a new default `ArrayAggregator`
pub fn array() -> ArrayAggregator {
    ArrayAggregator::default()
}

/// An aggregator which stores metrics in an array.
#[derive(Debug, Default)]
pub struct ArrayAggregator {
    inner: Mutex<Inner>,
}

impl Count for ArrayAggregator {
    fn count(&self) -> Result<u64> {
        self.inner
            .lock()
            .map_err(Into::into)
            .map(|inner| inner.points.as_ref().map_or(0, |p| p.len() as u64))
    }
}

impl Points for ArrayAggregator {
    fn points(&self) -> Result<Vec<Number>> {
        self.inner
            .lock()
            .map_err(Into::into)
            .map(|inner| inner.points.as_ref().map_or_else(Vec::new, |p| p.0.clone()))
    }
}

impl Aggregator for ArrayAggregator {
    fn update(&self, number: &Number, descriptor: &Descriptor) -> Result<()> {
        self.inner.lock().map_err(Into::into).map(|mut inner| {
            if let Some(points) = inner.points.as_mut() {
                points.push(number.clone());
            } else {
                inner.points = Some(PointsData::with_number(number.clone()));
            }
            inner.sum.fetch_add(descriptor.number_kind(), number)
        })
    }

    fn synchronized_move(
        &self,
        other: &Arc<dyn Aggregator + Send + Sync>,
        descriptor: &Descriptor,
    ) -> Result<()> {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            other
                .inner
                .lock()
                .map_err(Into::into)
                .and_then(|mut other| {
                    self.inner.lock().map_err(Into::into).map(|mut inner| {
                        other.points = mem::take(&mut inner.points);
                        other.sum = mem::replace(
                            &mut inner.sum,
                            descriptor.number_kind().zero().to_atomic(),
                        );

                        // TODO: This sort should be done lazily, only when quantiles are
                        // requested. The SDK specification says you can use this aggregator to
                        // simply list values in the order they were received as an alternative to
                        // requesting quantile information.
                        if let Some(points) = &mut other.points {
                            points.sort(descriptor.number_kind());
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
    fn merge(&self, other: &(dyn Aggregator + Send + Sync), desc: &Descriptor) -> Result<()> {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            self.inner.lock().map_err(Into::into).and_then(|mut inner| {
                other.inner.lock().map_err(From::from).map(|other_inner| {
                    // Note: Current assumption is that `o` was checkpointed,
                    // therefore is already sorted.  See the TODO above, since
                    // this is an open question.
                    inner
                        .sum
                        .fetch_add(desc.number_kind(), &other_inner.sum.load());
                    match (inner.points.as_mut(), other_inner.points.as_ref()) {
                        (Some(points), Some(other_points)) => {
                            points.combine(desc.number_kind(), other_points)
                        }
                        (None, Some(other_points)) => inner.points = Some(other_points.clone()),
                        _ => (),
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

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug, Default)]
struct Inner {
    sum: AtomicNumber,
    points: Option<PointsData>,
}

#[derive(Clone, Debug, Default)]
struct PointsData(Vec<Number>);

impl PointsData {
    fn with_number(number: Number) -> Self {
        PointsData(vec![number])
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn push(&mut self, number: Number) {
        self.0.push(number)
    }

    fn sort(&mut self, kind: &NumberKind) {
        match kind {
            NumberKind::I64 => self.0.sort_by_key(|a| a.to_u64(kind)),
            NumberKind::F64 => self.0.sort_by(|a, b| {
                a.to_f64(kind)
                    .partial_cmp(&b.to_f64(kind))
                    .expect("nan values should be rejected. This is a bug.")
            }),
            NumberKind::U64 => self.0.sort_by_key(|a| a.to_u64(kind)),
        }
    }
    fn combine(&mut self, kind: &NumberKind, other: &PointsData) {
        self.0.append(&mut other.0.clone());
        self.sort(kind)
    }
}
