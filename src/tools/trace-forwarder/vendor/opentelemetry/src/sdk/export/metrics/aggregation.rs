//! Metrics SDK Aggregator export API
use crate::metrics::{Number, Result};
use std::time::SystemTime;

/// Sum returns an aggregated sum.
pub trait Sum {
    /// The sum of the currently aggregated metrics
    fn sum(&self) -> Result<Number>;
}

/// Count returns the number of values that were aggregated.
pub trait Count {
    /// The count of the currently aggregated metrics
    fn count(&self) -> Result<u64>;
}

/// Min returns the minimum value over the set of values that were aggregated.
pub trait Min {
    /// The min of the currently aggregated metrics
    fn min(&self) -> Result<Number>;
}

/// Max returns the maximum value over the set of values that were aggregated.
pub trait Max {
    /// The max of the currently aggregated metrics
    fn max(&self) -> Result<Number>;
}

/// LastValue returns the latest value that was aggregated.
pub trait LastValue {
    /// The last value of the currently aggregated metrics
    fn last_value(&self) -> Result<(Number, SystemTime)>;
}

/// Points return the raw set of values that were aggregated.
pub trait Points {
    /// The raw set of points currently aggregated
    fn points(&self) -> Result<Vec<Number>>;
}

/// Buckets represent histogram buckets boundaries and counts.
///
/// For a Histogram with N defined boundaries, e.g, [x, y, z].
/// There are N+1 counts: [-inf, x), [x, y), [y, z), [z, +inf]
#[derive(Debug)]
pub struct Buckets {
    /// Boundaries are floating point numbers, even when
    /// aggregating integers.
    boundaries: Vec<f64>,

    /// Counts are floating point numbers to account for
    /// the possibility of sampling which allows for
    /// non-integer count values.
    counts: Vec<f64>,
}

impl Buckets {
    /// Create new buckets
    pub fn new(boundaries: Vec<f64>, counts: Vec<f64>) -> Self {
        Buckets { boundaries, counts }
    }

    /// Boundaries of the histogram buckets
    pub fn boundaries(&self) -> &Vec<f64> {
        &self.boundaries
    }

    /// Counts of the histogram buckets
    pub fn counts(&self) -> &Vec<f64> {
        &self.counts
    }
}

/// Histogram returns the count of events in pre-determined buckets.
pub trait Histogram: Sum + Count {
    /// Buckets for this histogram.
    fn histogram(&self) -> Result<Buckets>;
}

/// MinMaxSumCount supports the Min, Max, Sum, and Count interfaces.
pub trait MinMaxSumCount: Min + Max + Sum + Count {}
