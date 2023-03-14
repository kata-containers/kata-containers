//! DDSketch quantile sketch with relative-error guarantees.
//! DDSketch is a fast and fully-mergeable quantile sketch with relative-error guarantees.
//!
//! The main difference between this approach and previous art is DDSKetch employ a new method to
//! compute the error. Traditionally, the error rate of one sketch is evaluated by rank accuracy,
//! which can still generate a relative large variance if the dataset has long tail.
//!
//! DDSKetch, on the contrary, employs relative error rate that could work well on long tail dataset.
//!
//! The detail of this algorithm can be found in https://arxiv.org/pdf/1908.10693

use std::{
    any::Any,
    cmp::Ordering,
    mem,
    ops::AddAssign,
    sync::{Arc, RwLock},
};

use crate::{
    metrics::{Descriptor, MetricsError, Number, NumberKind, Result},
    sdk::export::metrics::{Aggregator, Count, Max, Min, MinMaxSumCount, Sum},
};

const INITIAL_NUM_BINS: usize = 128;
const GROW_LEFT_BY: i64 = 128;

const DEFAULT_MAX_NUM_BINS: i64 = 2048;
const DEFAULT_ALPHA: f64 = 0.01;
const DEFAULT_MIN_BOUNDARY: f64 = 1.0e-9;

/// An aggregator to calculate quantile
pub fn ddsketch(config: &DdSketchConfig, kind: NumberKind) -> DdSketchAggregator {
    DdSketchAggregator::new(config, kind)
}

/// DDSKetch quantile sketch algorithm
///
/// It can give q-quantiles with α-accurate for any 0<=q<=1.
///
/// Here the accurate is calculated based on relative-error rate. Thus, the error guarantee adapts the scale of the output data. With relative error guarantee, the histogram can be more accurate in the area of low data density. For example, the long tail of response time data.
///
/// For example, if the actual percentile is 1 second, and relative-error guarantee
/// is 2%, then the value should within the range of 0.98 to 1.02
/// second. But if the actual percentile is 1 millisecond, with the same relative-error
/// guarantee, the value returned should within the range of 0.98 to 1.02 millisecond.
///
/// In order to support both negative and positive inputs, DDSketchAggregator has two DDSketch store within itself to store the negative and positive inputs.
#[derive(Debug)]
pub struct DdSketchAggregator {
    inner: RwLock<Inner>,
}

impl DdSketchAggregator {
    /// Create a new DDSKetchAggregator that would yield a quantile with relative error rate less
    /// than `alpha`
    ///
    /// The input should have a granularity larger than `key_epsilon`
    pub fn new(config: &DdSketchConfig, kind: NumberKind) -> DdSketchAggregator {
        DdSketchAggregator {
            inner: RwLock::new(Inner::new(config, kind)),
        }
    }
}

impl Default for DdSketchAggregator {
    fn default() -> Self {
        DdSketchAggregator::new(
            &DdSketchConfig::new(DEFAULT_ALPHA, DEFAULT_MAX_NUM_BINS, DEFAULT_MIN_BOUNDARY),
            NumberKind::F64,
        )
    }
}

impl Sum for DdSketchAggregator {
    fn sum(&self) -> Result<Number> {
        self.inner
            .read()
            .map_err(From::from)
            .map(|inner| inner.sum.clone())
    }
}

impl Min for DdSketchAggregator {
    fn min(&self) -> Result<Number> {
        self.inner
            .read()
            .map_err(From::from)
            .map(|inner| inner.min_value.clone())
    }
}

impl Max for DdSketchAggregator {
    fn max(&self) -> Result<Number> {
        self.inner
            .read()
            .map_err(From::from)
            .map(|inner| inner.max_value.clone())
    }
}

impl Count for DdSketchAggregator {
    fn count(&self) -> Result<u64> {
        self.inner
            .read()
            .map_err(From::from)
            .map(|inner| inner.count())
    }
}

impl MinMaxSumCount for DdSketchAggregator {}

impl Aggregator for DdSketchAggregator {
    fn update(&self, number: &Number, descriptor: &Descriptor) -> Result<()> {
        self.inner
            .write()
            .map_err(From::from)
            .map(|mut inner| inner.add(number, descriptor.number_kind()))
    }

    fn synchronized_move(
        &self,
        destination: &Arc<(dyn Aggregator + Send + Sync)>,
        descriptor: &Descriptor,
    ) -> Result<()> {
        if let Some(other) = destination.as_any().downcast_ref::<Self>() {
            other
                .inner
                .write()
                .map_err(From::from)
                .and_then(|mut other| {
                    self.inner.write().map_err(From::from).map(|mut inner| {
                        let kind = descriptor.number_kind();
                        other.max_value = mem::replace(&mut inner.max_value, kind.zero());
                        other.min_value = mem::replace(&mut inner.min_value, kind.zero());
                        other.key_epsilon = mem::take(&mut inner.key_epsilon);
                        other.offset = mem::take(&mut inner.offset);
                        other.gamma = mem::take(&mut inner.gamma);
                        other.gamma_ln = mem::take(&mut inner.gamma_ln);
                        other.positive_store = mem::take(&mut inner.positive_store);
                        other.negative_store = mem::take(&mut inner.negative_store);
                        other.sum = mem::replace(&mut inner.sum, kind.zero());
                    })
                })
        } else {
            Err(MetricsError::InconsistentAggregator(format!(
                "Expected {:?}, got: {:?}",
                self, destination
            )))
        }
    }

    fn merge(
        &self,
        other: &(dyn Aggregator + Send + Sync),
        _descriptor: &Descriptor,
    ) -> Result<()> {
        if let Some(other) = other.as_any().downcast_ref::<DdSketchAggregator>() {
            self.inner.write()
                .map_err(From::from)
                .and_then(|mut inner| {
                    other.inner.read()
                        .map_err(From::from)
                        .and_then(|other| {
                            // assert that it can merge
                            if inner.positive_store.max_num_bins != other.positive_store.max_num_bins {
                                return Err(MetricsError::InconsistentAggregator(format!(
                                    "When merging two DDSKetchAggregators, their max number of bins must be the same. Expect max number of bins to be {:?}, but get {:?}", inner.positive_store.max_num_bins, other.positive_store.max_num_bins
                                )));
                            }
                            if inner.negative_store.max_num_bins != other.negative_store.max_num_bins {
                                return Err(MetricsError::InconsistentAggregator(format!(
                                    "When merging two DDSKetchAggregators, their max number of bins must be the same. Expect max number of bins to be {:?}, but get {:?}", inner.negative_store.max_num_bins, other.negative_store.max_num_bins
                                )));
                            }


                            if (inner.gamma - other.gamma).abs() > std::f64::EPSILON {
                                return Err(MetricsError::InconsistentAggregator(format!(
                                    "When merging two DDSKetchAggregators, their gamma must be the same. Expect max number of bins to be {:?}, but get {:?}", inner.gamma, other.gamma
                                )));
                            }

                            if other.count() == 0 {
                                return Ok(());
                            }

                            if inner.count() == 0 {
                                inner.positive_store.merge(&other.positive_store);
                                inner.negative_store.merge(&other.negative_store);
                                inner.sum = other.sum.clone();
                                inner.min_value = other.min_value.clone();
                                inner.max_value = other.max_value.clone();
                                return Ok(());
                            }

                            inner.positive_store.merge(&other.positive_store);
                            inner.negative_store.merge(&other.negative_store);

                            inner.sum = match inner.kind {
                                NumberKind::F64 =>
                                    Number::from(inner.sum.to_f64(&inner.kind) + other.sum.to_f64(&other.kind)),
                                NumberKind::U64 => Number::from(inner.sum.to_u64(&inner.kind) + other.sum.to_u64(&other.kind)),
                                NumberKind::I64 => Number::from(inner.sum.to_i64(&inner.kind) + other.sum.to_i64(&other.kind))
                            };

                            if inner.min_value.partial_cmp(&inner.kind, &other.min_value) == Some(Ordering::Greater) {
                                inner.min_value = other.min_value.clone();
                            };

                            if inner.max_value.partial_cmp(&inner.kind, &other.max_value) == Some(Ordering::Less) {
                                inner.max_value = other.max_value.clone();
                            }

                            Ok(())
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

/// DDSKetch Configuration.
#[derive(Debug)]
pub struct DdSketchConfig {
    alpha: f64,
    max_num_bins: i64,
    key_epsilon: f64,
}

impl DdSketchConfig {
    /// Create a new DDSKetch config
    pub fn new(alpha: f64, max_num_bins: i64, key_epsilon: f64) -> Self {
        DdSketchConfig {
            alpha,
            max_num_bins,
            key_epsilon,
        }
    }
}

/// DDSKetch implementation.
///
/// Note that Inner is not thread-safe. All operation should be protected by a lock or other
/// synchronization.
///
/// Inner will also convert all Number into actual primitive type and back.
///
/// According to the paper, the DDSKetch only support positive number. Inner support
/// either positive or negative number. But cannot yield actual result when input has
/// both positive and negative number.
#[derive(Debug)]
struct Inner {
    positive_store: Store,
    negative_store: Store,
    kind: NumberKind,
    // sum of all value within store
    sum: Number,
    // γ = (1 + α)/(1 - α)
    gamma: f64,
    // ln(γ)
    gamma_ln: f64,
    // The epsilon when map value to bin key. Any value between [-key_epsilon, key_epsilon] will
    // be mapped to bin key 0. Must be a positive number.
    key_epsilon: f64,
    // offset is here to ensure that keys for positive numbers that are larger than min_value are
    // greater than or equal to 1 while the keys for negative numbers are less than or equal to -1.
    offset: i64,

    // minimum number that in store.
    min_value: Number,
    // maximum number that in store.
    max_value: Number,
}

impl Inner {
    fn new(config: &DdSketchConfig, kind: NumberKind) -> Inner {
        let gamma: f64 = 1.0 + 2.0 * config.alpha / (1.0 - config.alpha);
        let mut inner = Inner {
            positive_store: Store::new(config.max_num_bins / 2),
            negative_store: Store::new(config.max_num_bins / 2),
            min_value: kind.max(),
            max_value: kind.min(),
            sum: kind.zero(),
            gamma,
            gamma_ln: gamma.ln(),
            key_epsilon: config.key_epsilon,
            offset: 0,
            kind,
        };
        // reset offset based on key_epsilon
        inner.offset = -(inner.log_gamma(inner.key_epsilon)).ceil() as i64 + 1i64;
        inner
    }

    fn add(&mut self, v: &Number, kind: &NumberKind) {
        let key = self.key(v, kind);
        match v.partial_cmp(kind, &Number::from(0.0)) {
            Some(Ordering::Greater) | Some(Ordering::Equal) => {
                self.positive_store.add(key);
            }
            Some(Ordering::Less) => {
                self.negative_store.add(key);
            }
            _ => {
                // if return none. Do nothing and return
                return;
            }
        }

        // update min and max
        if self.min_value.partial_cmp(&self.kind, v) == Some(Ordering::Greater) {
            self.min_value = v.clone();
        }

        if self.max_value.partial_cmp(&self.kind, v) == Some(Ordering::Less) {
            self.max_value = v.clone();
        }

        match &self.kind {
            NumberKind::I64 => {
                self.sum = Number::from(self.sum.to_i64(&self.kind) + v.to_i64(kind));
            }
            NumberKind::U64 => {
                self.sum = Number::from(self.sum.to_u64(&self.kind) + v.to_u64(kind));
            }
            NumberKind::F64 => {
                self.sum = Number::from(self.sum.to_f64(&self.kind) + v.to_f64(kind));
            }
        }
    }

    fn key(&self, num: &Number, kind: &NumberKind) -> i64 {
        if num.to_f64(kind) < -self.key_epsilon {
            let positive_num = match kind {
                NumberKind::F64 => Number::from(-num.to_f64(kind)),
                NumberKind::U64 => Number::from(num.to_u64(kind)),
                NumberKind::I64 => Number::from(-num.to_i64(kind)),
            };
            (-self.log_gamma(positive_num.to_f64(kind)).ceil()) as i64 - self.offset
        } else if num.to_f64(kind) > self.key_epsilon {
            self.log_gamma(num.to_f64(&kind)).ceil() as i64 + self.offset
        } else {
            0i64
        }
    }

    /// get the index of the bucket based on num
    fn log_gamma(&self, num: f64) -> f64 {
        num.ln() / self.gamma_ln
    }

    fn count(&self) -> u64 {
        self.negative_store.count + self.positive_store.count
    }
}

#[derive(Debug)]
struct Store {
    bins: Vec<u64>,
    count: u64,
    min_key: i64,
    max_key: i64,
    // maximum number of bins Store can have.
    // In the worst case, the bucket can grow as large as the number of the elements inserted into.
    // max_num_bins helps control the number of bins.
    max_num_bins: i64,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            bins: vec![0; INITIAL_NUM_BINS],
            count: 0,
            min_key: 0,
            max_key: 0,
            max_num_bins: DEFAULT_MAX_NUM_BINS,
        }
    }
}

/// DDSKetchInner stores the data
impl Store {
    fn new(max_num_bins: i64) -> Store {
        Store {
            bins: vec![
                0;
                if max_num_bins as usize > INITIAL_NUM_BINS {
                    INITIAL_NUM_BINS
                } else {
                    max_num_bins as usize
                }
            ],
            count: 0u64,
            min_key: 0i64,
            max_key: 0i64,
            max_num_bins,
        }
    }

    /// Add count based on key.
    ///
    /// If key is not in [min_key, max_key], we will expand to left or right
    ///
    ///
    /// The bins are essentially working in a round-robin fashion where we can use all space in bins
    /// to represent any continuous space within length. That's why we need to offset the key
    /// with `min_key` so that we get the actual bin index.
    fn add(&mut self, key: i64) {
        if self.count == 0 {
            self.max_key = key;
            self.min_key = key - self.bins.len() as i64 + 1
        }

        if key < self.min_key {
            self.grow_left(key)
        } else if key > self.max_key {
            self.grow_right(key)
        }
        let idx = if key - self.min_key < 0 {
            0
        } else {
            key - self.min_key
        };
        // we unwrap here because grow_left or grow_right will make sure the idx is less than vector size
        let bin_count = self.bins.get_mut(idx as usize).unwrap();
        *bin_count += 1;
        self.count += 1;
    }

    fn grow_left(&mut self, key: i64) {
        if self.min_key < key || self.bins.len() >= self.max_num_bins as usize {
            return;
        }

        let min_key = if self.max_key - key >= self.max_num_bins {
            self.max_key - self.max_num_bins + 1
        } else {
            let mut min_key = self.min_key;
            while min_key > key {
                min_key -= GROW_LEFT_BY;
            }
            min_key
        };

        // The new vector will contain three parts.
        // First part is all 0, which is the part expended
        // Second part is from existing bins.
        // Third part is what's left.
        let expected_len = (self.max_key - min_key + 1) as usize;
        let mut new_bins = vec![0u64; expected_len];
        let old_bin_slice = &mut new_bins[(self.min_key - min_key) as usize..];
        old_bin_slice.copy_from_slice(&self.bins);

        self.bins = new_bins;
        self.min_key = min_key;
    }

    fn grow_right(&mut self, key: i64) {
        if self.max_key > key {
            return;
        }

        if key - self.max_key >= self.max_num_bins {
            // if currently key minus currently max key is larger than maximum number of bins.
            // Move all elements in current bins into the first bin
            self.bins = vec![0; self.max_num_bins as usize];
            self.max_key = key;
            self.min_key = key - self.max_num_bins + 1;
            self.bins.get_mut(0).unwrap().add_assign(self.count);
        } else if key - self.min_key >= self.max_num_bins {
            let min_key = key - self.max_num_bins + 1;
            let upper_bound = if min_key < self.max_key + 1 {
                min_key
            } else {
                self.max_key + 1
            } - self.min_key;
            let n = self.bins.iter().take(upper_bound as usize).sum::<u64>();

            if self.bins.len() < self.max_num_bins as usize {
                let mut new_bins = vec![0; self.max_num_bins as usize];
                new_bins[0..self.bins.len() - (min_key - self.min_key) as usize]
                    .as_mut()
                    .copy_from_slice(&self.bins[(min_key - self.min_key) as usize..]);
                self.bins = new_bins;
            } else {
                // bins length is equal to max number of bins
                self.bins.drain(0..(min_key - self.min_key) as usize);
                if self.max_num_bins > self.max_key - min_key + 1 {
                    self.bins.resize(
                        self.bins.len()
                            + (self.max_num_bins - (self.max_key - min_key + 1)) as usize,
                        0,
                    )
                }
            }
            self.max_key = key;
            self.min_key = min_key;
            self.bins.get_mut(0).unwrap().add_assign(n);
        } else {
            let mut new_bin = vec![0; (key - self.min_key + 1) as usize];
            new_bin[0..self.bins.len()]
                .as_mut()
                .copy_from_slice(&self.bins);
            self.bins = new_bin;
            self.max_key = key;
        }
    }

    /// Merge two stores
    fn merge(&mut self, other: &Store) {
        if self.count == 0 {
            return;
        }
        if other.count == 0 {
            self.bins = other.bins.clone();
            self.min_key = other.min_key;
            self.max_key = other.max_key;
            self.count = other.count;
        }

        if self.max_key > other.max_key {
            if other.min_key < self.min_key {
                self.grow_left(other.min_key);
            }
            let start = if other.min_key > self.min_key {
                other.min_key
            } else {
                self.min_key
            } as usize;
            for i in start..other.max_key as usize {
                self.bins[i - self.min_key as usize] = other.bins[i - other.min_key as usize];
            }
            let mut n = 0;
            for i in other.min_key as usize..self.min_key as usize {
                n += other.bins[i - other.min_key as usize]
            }
            self.bins[0] += n;
        } else if other.min_key < self.min_key {
            let mut tmp_bins = vec![0u64; other.bins.len()];
            tmp_bins.as_mut_slice().copy_from_slice(&other.bins);

            for i in self.min_key as usize..self.max_key as usize {
                tmp_bins[i - other.min_key as usize] += self.bins[i - self.min_key as usize];
            }

            self.bins = tmp_bins;
            self.max_key = other.max_key;
            self.min_key = other.min_key;
        } else {
            self.grow_right(other.max_key);
            for i in other.min_key as usize..(other.max_key + 1) as usize {
                self.bins[i - self.min_key as usize] += other.bins[i - other.min_key as usize];
            }
        }

        self.count += other.count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::{Descriptor, InstrumentKind, Number, NumberKind};
    use crate::sdk::export::metrics::{Aggregator, Count, Max, Min, Sum};
    use rand_distr::{Distribution, Exp, LogNormal, Normal};
    use std::cmp::Ordering;
    use std::sync::Arc;

    const TEST_MAX_BINS: i64 = 1024;
    const TEST_ALPHA: f64 = 0.01;
    const TEST_KEY_EPSILON: f64 = 1.0e-9;

    // Test utils

    struct Dataset {
        data: Vec<Number>,
        kind: NumberKind,
    }

    impl Dataset {
        fn from_f64_vec(data: Vec<f64>) -> Dataset {
            Dataset {
                data: data.into_iter().map(Number::from).collect::<Vec<Number>>(),
                kind: NumberKind::F64,
            }
        }

        fn from_u64_vec(data: Vec<u64>) -> Dataset {
            Dataset {
                data: data.into_iter().map(Number::from).collect::<Vec<Number>>(),
                kind: NumberKind::U64,
            }
        }

        fn from_i64_vec(data: Vec<i64>) -> Dataset {
            Dataset {
                data: data.into_iter().map(Number::from).collect::<Vec<Number>>(),
                kind: NumberKind::I64,
            }
        }

        fn sum(&self) -> Number {
            match self.kind {
                NumberKind::F64 => {
                    Number::from(self.data.iter().map(|e| e.to_f64(&self.kind)).sum::<f64>())
                }
                NumberKind::U64 => {
                    Number::from(self.data.iter().map(|e| e.to_u64(&self.kind)).sum::<u64>())
                }
                NumberKind::I64 => {
                    Number::from(self.data.iter().map(|e| e.to_i64(&self.kind)).sum::<i64>())
                }
            }
        }
    }

    fn generate_linear_dataset_f64(start: f64, step: f64, num: usize) -> Vec<f64> {
        let mut vec = Vec::with_capacity(num);
        for i in 0..num {
            vec.push((start + i as f64 * step) as f64);
        }
        vec
    }

    fn generate_linear_dataset_u64(start: u64, step: u64, num: usize) -> Vec<u64> {
        let mut vec = Vec::with_capacity(num);
        for i in 0..num {
            vec.push(start + i as u64 * step);
        }
        vec
    }

    fn generate_linear_dataset_i64(start: i64, step: i64, num: usize) -> Vec<i64> {
        let mut vec = Vec::with_capacity(num);
        for i in 0..num {
            vec.push(start + i as i64 * step);
        }
        vec
    }

    /// generate a dataset with normal distribution. Return sorted dataset.
    fn generate_normal_dataset(mean: f64, stddev: f64, num: usize) -> Vec<f64> {
        let normal = Normal::new(mean, stddev).unwrap();
        let mut data = Vec::with_capacity(num);
        for _ in 0..num {
            data.push(normal.sample(&mut rand::thread_rng()));
        }
        data.as_mut_slice()
            .sort_by(|a, b| a.partial_cmp(b).unwrap());
        data
    }

    /// generate a dataset with log normal distribution. Return sorted dataset.
    fn generate_log_normal_dataset(mean: f64, stddev: f64, num: usize) -> Vec<f64> {
        let normal = LogNormal::new(mean, stddev).unwrap();
        let mut data = Vec::with_capacity(num);
        for _ in 0..num {
            data.push(normal.sample(&mut rand::thread_rng()));
        }
        data.as_mut_slice()
            .sort_by(|a, b| a.partial_cmp(b).unwrap());
        data
    }

    fn generate_exponential_dataset(rate: f64, num: usize) -> Vec<f64> {
        let exponential = Exp::new(rate).unwrap();
        let mut data = Vec::with_capacity(num);
        for _ in 0..num {
            data.push(exponential.sample(&mut rand::thread_rng()));
        }
        data.as_mut_slice()
            .sort_by(|a, b| a.partial_cmp(b).unwrap());
        data
    }

    /// Insert all element of data into ddsketch and assert the quantile result is within the error range.
    /// Note that data must be sorted.
    fn evaluate_sketch(dataset: Dataset) {
        let kind = &dataset.kind;
        let ddsketch = DdSketchAggregator::new(
            &DdSketchConfig::new(TEST_ALPHA, TEST_MAX_BINS, TEST_KEY_EPSILON),
            kind.clone(),
        );
        let descriptor = Descriptor::new(
            "test".to_string(),
            "test",
            None,
            InstrumentKind::ValueRecorder,
            kind.clone(),
        );

        for i in &dataset.data {
            let _ = ddsketch.update(i, &descriptor);
        }

        assert_eq!(
            ddsketch
                .min()
                .unwrap()
                .partial_cmp(kind, dataset.data.get(0).unwrap()),
            Some(Ordering::Equal)
        );
        assert_eq!(
            ddsketch
                .max()
                .unwrap()
                .partial_cmp(kind, dataset.data.last().unwrap()),
            Some(Ordering::Equal)
        );
        assert_eq!(
            ddsketch.sum().unwrap().partial_cmp(kind, &dataset.sum()),
            Some(Ordering::Equal)
        );
        assert_eq!(ddsketch.count().unwrap(), dataset.data.len() as u64);
    }

    // Test basic operation of Store

    /// First set max_num_bins < number of keys, test to see if the store will collapse to left
    /// most bin instead of expending beyond the max_num_bins
    #[test]
    fn test_insert_into_store() {
        let mut store = Store::new(200);
        for i in -100..1300 {
            store.add(i)
        }
        assert_eq!(store.count, 1400);
        assert_eq!(store.bins.len(), 200);
    }

    /// Test to see if copy_from_slice will panic because the range size is different in left and right
    #[test]
    fn test_grow_right() {
        let mut store = Store::new(150);
        for i in &[-100, -50, 150, -20, 10] {
            store.add(*i)
        }
        assert_eq!(store.count, 5);
    }

    /// Test to see if copy_from_slice will panic because the range size is different in left and right
    #[test]
    fn test_grow_left() {
        let mut store = Store::new(150);
        for i in &[500, 150, 10] {
            store.add(*i)
        }
        assert_eq!(store.count, 3);
    }

    /// Before merge, store1 should hold 300 bins that looks like [201,1,1,1,...],
    /// store 2 should hold 200 bins looks like [301,1,1,...]
    /// After merge, store 1 should still hold 300 bins with following distribution
    ///
    /// index [0,0] -> 201
    ///
    /// index [1,99] -> 1
    ///
    /// index [100, 100] -> 302
    ///
    /// index [101, 299] -> 2
    #[test]
    fn test_merge_stores() {
        let mut store1 = Store::new(300);
        let mut store2 = Store::new(200);
        for i in 500..1000 {
            store1.add(i);
            store2.add(i);
        }
        store1.merge(&store2);
        assert_eq!(store1.bins.get(0), Some(&201));
        assert_eq!(&store1.bins[1..100], vec![1u64; 99].as_slice());
        assert_eq!(store1.bins[100], 302);
        assert_eq!(&store1.bins[101..], vec![2u64; 199].as_slice());
        assert_eq!(store1.count, 1000);
    }

    // Test ddsketch with different distribution

    #[test]
    fn test_linear_distribution() {
        // test u64
        let mut dataset = Dataset::from_u64_vec(generate_linear_dataset_u64(12, 3, 5000));
        evaluate_sketch(dataset);

        // test i64
        dataset = Dataset::from_i64_vec(generate_linear_dataset_i64(-12, 3, 5000));
        evaluate_sketch(dataset);

        // test f64
        dataset = Dataset::from_f64_vec(generate_linear_dataset_f64(-12.0, 3.0, 5000));
        evaluate_sketch(dataset);
    }

    #[test]
    fn test_normal_distribution() {
        let mut dataset = Dataset::from_f64_vec(generate_normal_dataset(150.0, 1.2, 100));
        evaluate_sketch(dataset);

        dataset = Dataset::from_f64_vec(generate_normal_dataset(-30.0, 4.4, 100));
        evaluate_sketch(dataset);
    }

    #[test]
    fn test_log_normal_distribution() {
        let dataset = Dataset::from_f64_vec(generate_log_normal_dataset(120.0, 0.5, 100));
        evaluate_sketch(dataset);
    }

    #[test]
    fn test_exponential_distribution() {
        let dataset = Dataset::from_f64_vec(generate_exponential_dataset(2.0, 500));
        evaluate_sketch(dataset);
    }

    // Test Aggregator operation of DDSketch
    #[test]
    fn test_synchronized_move() {
        let dataset = Dataset::from_f64_vec(generate_normal_dataset(1.0, 3.5, 100));
        let kind = &dataset.kind;
        let ddsketch = DdSketchAggregator::new(
            &DdSketchConfig::new(TEST_ALPHA, TEST_MAX_BINS, TEST_KEY_EPSILON),
            kind.clone(),
        );
        let descriptor = Descriptor::new(
            "test".to_string(),
            "test",
            None,
            InstrumentKind::ValueRecorder,
            kind.clone(),
        );
        for i in &dataset.data {
            let _ = ddsketch.update(i, &descriptor);
        }
        let expected_sum = ddsketch.sum().unwrap().to_f64(&NumberKind::F64);
        let expected_count = ddsketch.count().unwrap();
        let expected_min = ddsketch.min().unwrap().to_f64(&NumberKind::F64);
        let expected_max = ddsketch.max().unwrap().to_f64(&NumberKind::F64);

        let moved_ddsketch: Arc<(dyn Aggregator + Send + Sync)> =
            Arc::new(DdSketchAggregator::new(
                &DdSketchConfig::new(TEST_ALPHA, TEST_MAX_BINS, TEST_KEY_EPSILON),
                NumberKind::F64,
            ));
        let _ = ddsketch
            .synchronized_move(&moved_ddsketch, &descriptor)
            .expect("Fail to sync move");
        let moved_ddsketch = moved_ddsketch
            .as_any()
            .downcast_ref::<DdSketchAggregator>()
            .expect("Fail to cast dyn Aggregator down to DDSketchAggregator");

        // assert sum, max, min and count
        assert!(
            (moved_ddsketch.max().unwrap().to_f64(&NumberKind::F64) - expected_max).abs()
                < std::f64::EPSILON
        );
        assert!(
            (moved_ddsketch.min().unwrap().to_f64(&NumberKind::F64) - expected_min).abs()
                < std::f64::EPSILON
        );
        assert!(
            (moved_ddsketch.sum().unwrap().to_f64(&NumberKind::F64) - expected_sum).abs()
                < std::f64::EPSILON
        );
        assert_eq!(moved_ddsketch.count().unwrap(), expected_count);
    }
}
