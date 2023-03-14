use crate::metrics::{Descriptor, MetricsError, Number, Result};
use crate::sdk::export::metrics::{Aggregator, LastValue};
use std::any::Any;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Create a new `LastValueAggregator`
pub fn last_value() -> LastValueAggregator {
    LastValueAggregator {
        inner: Mutex::new(Inner::default()),
    }
}

/// Aggregates last value events.
#[derive(Debug)]
pub struct LastValueAggregator {
    inner: Mutex<Inner>,
}

impl Aggregator for LastValueAggregator {
    fn update(&self, number: &Number, _descriptor: &Descriptor) -> Result<()> {
        self.inner.lock().map_err(Into::into).map(|mut inner| {
            inner.state = Some(LastValueData {
                value: number.clone(),
                timestamp: crate::time::now(),
            });
        })
    }
    fn synchronized_move(
        &self,
        other: &Arc<dyn Aggregator + Send + Sync>,
        _descriptor: &Descriptor,
    ) -> Result<()> {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            self.inner.lock().map_err(From::from).and_then(|mut inner| {
                other.inner.lock().map_err(From::from).map(|mut other| {
                    other.state = inner.state.take();
                })
            })
        } else {
            Err(MetricsError::InconsistentAggregator(format!(
                "Expected {:?}, got: {:?}",
                self, other
            )))
        }
    }
    fn merge(
        &self,
        other: &(dyn Aggregator + Send + Sync),
        _descriptor: &Descriptor,
    ) -> Result<()> {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            self.inner.lock().map_err(From::from).and_then(|mut inner| {
                other.inner.lock().map_err(From::from).map(|mut other| {
                    match (&inner.state, &other.state) {
                        // Take if other timestamp is greater
                        (Some(checkpoint), Some(other_checkpoint))
                            if other_checkpoint.timestamp > checkpoint.timestamp =>
                        {
                            inner.state = other.state.take()
                        }
                        // Take if no value exists currently
                        (None, Some(_)) => inner.state = other.state.take(),
                        // Otherwise done
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

impl LastValue for LastValueAggregator {
    fn last_value(&self) -> Result<(Number, SystemTime)> {
        self.inner.lock().map_err(Into::into).and_then(|inner| {
            if let Some(checkpoint) = &inner.state {
                Ok((checkpoint.value.clone(), checkpoint.timestamp))
            } else {
                Err(MetricsError::NoDataCollected)
            }
        })
    }
}

#[derive(Debug, Default)]
struct Inner {
    state: Option<LastValueData>,
}

#[derive(Debug)]
struct LastValueData {
    value: Number,
    timestamp: SystemTime,
}
