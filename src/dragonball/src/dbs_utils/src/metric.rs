// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Defines the public components of the metric system.
//!
//! # Design
//! The main design goals of this system are:
//! * Use lockless operations, preferably ones that don't require anything other than
//!   simple reads/writes being atomic.
//! * Exploit interior mutability and atomics being Sync to allow all methods (including the ones
//!   which are effectively mutable) to be callable on a global non-mut static.
//! * Rely on `serde` to provide the actual serialization for writing the metrics.
//! * Since all metrics start at 0, we implement the `Default` trait via derive for all of them,
//!   to avoid having to initialize everything by hand.
//!
//! The system implements 2 types of metrics:
//! * Shared Incremental Metrics (SharedIncMetrics) - dedicated for the metrics which need a counter
//! (i.e the number of times an API request failed). These metrics are reset upon flush.
//! * Shared Store Metrics (SharedStoreMetrics) - are targeted at keeping a persistent value, it is not
//! intended to act as a counter (i.e for measure the process start up time for example).
//!
//! The current approach for the `SharedIncMetrics` type is to store two values (current and previous)
//! and compute the delta between them each time we do a flush (i.e by serialization). There are a number of advantages
//! to this approach, including:
//! * We don't have to introduce an additional write (to reset the value) from the thread which
//!   does to actual writing, so less synchronization effort is required.
//! * We don't have to worry at all that much about losing some data if writing fails for a while
//!   (this could be a concern, I guess).
//! If if turns out this approach is not really what we want, it's pretty easy to resort to
//! something else, while working behind the same interface.

use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Serialize, Serializer};

/// Used for defining new types of metrics that act as a counter (i.e they are continuously updated by
/// incrementing their value).
pub trait IncMetric {
    /// Adds `value` to the current counter.
    fn add(&self, value: usize);
    /// Increments by 1 unit the current counter.
    fn inc(&self) {
        self.add(1);
    }
    /// Returns current value of the counter.
    fn count(&self) -> usize;
}

/// Representation of a metric that is expected to be incremented from more than one thread, so more
/// synchronization is necessary.
// It's currently used for vCPU metrics. An alternative here would be
// to have one instance of every metric for each thread, and to
// aggregate them when writing. However this probably overkill unless we have a lot of vCPUs
// incrementing metrics very often. Still, it's there if we ever need it :-s
// We will be keeping two values for each metric for being able to reset
// counters on each metric.
// 1st member - current value being updated
// 2nd member - old value that gets the current value whenever metrics is flushed to disk
#[derive(Default)]
pub struct SharedIncMetric(AtomicUsize, AtomicUsize);

impl IncMetric for SharedIncMetric {
    // While the order specified for this operation is still Relaxed, the actual instruction will
    // be an asm "LOCK; something" and thus atomic across multiple threads, simply because of the
    // fetch_and_add (as opposed to "store(load() + 1)") implementation for atomics.
    // TODO: would a stronger ordering make a difference here?
    fn add(&self, value: usize) {
        self.0.fetch_add(value, Ordering::Relaxed);
    }

    fn count(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

impl Serialize for SharedIncMetric {
    /// Reset counters of each metrics. Here we suppose that Serialize's goal is to help with the
    /// flushing of metrics.
    /// !!! Any print of the metrics will also reset them. Use with caution !!!
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // There's no serializer.serialize_usize() for some reason :(
        let snapshot = self.0.load(Ordering::Relaxed);
        let res = serializer.serialize_u64(snapshot as u64 - self.1.load(Ordering::Relaxed) as u64);

        if res.is_ok() {
            self.1.store(snapshot, Ordering::Relaxed);
        }
        res
    }
}

/// Used for defining new types of metrics that do not need a counter and act as a persistent indicator.
pub trait StoreMetric {
    /// Returns current value of the counter.
    fn fetch(&self) -> usize;
    /// Stores `value` to the current counter.
    fn store(&self, value: usize);
}

/// Representation of a metric that is expected to hold a value that can be accessed
/// from more than one thread, so more synchronization is necessary.
#[derive(Default)]
pub struct SharedStoreMetric(AtomicUsize);

impl StoreMetric for SharedStoreMetric {
    fn fetch(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }

    fn store(&self, value: usize) {
        self.0.store(value, Ordering::Relaxed);
    }
}

impl IncMetric for SharedStoreMetric {
    fn add(&self, value: usize) {
        // This operation wraps around on overflow.
        self.0.fetch_add(value, Ordering::Relaxed);
    }

    fn count(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

impl Serialize for SharedStoreMetric {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.0.load(Ordering::Relaxed) as u64)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::fence;
    use std::sync::Arc;
    use std::thread;

    use super::*;

    #[test]
    fn test_shared_inc_metric() {
        let metric = Arc::new(SharedIncMetric::default());

        // We're going to create a number of threads that will attempt to increase this metric
        // in parallel. If everything goes fine we still can't be sure the synchronization works,
        // but if something fails, then we definitely have a problem :-s

        const NUM_THREADS_TO_SPAWN: usize = 4;
        const NUM_INCREMENTS_PER_THREAD: usize = 10_0000;
        const M2_INITIAL_COUNT: usize = 123;

        metric.add(M2_INITIAL_COUNT);

        let mut v = Vec::with_capacity(NUM_THREADS_TO_SPAWN);

        for _ in 0..NUM_THREADS_TO_SPAWN {
            let r = metric.clone();
            v.push(thread::spawn(move || {
                for _ in 0..NUM_INCREMENTS_PER_THREAD {
                    r.inc();
                }
            }));
        }

        for handle in v {
            handle.join().unwrap();
        }

        assert_eq!(
            metric.count(),
            M2_INITIAL_COUNT + NUM_THREADS_TO_SPAWN * NUM_INCREMENTS_PER_THREAD
        );
    }

    #[test]
    fn test_shared_store_metric() {
        let m1 = Arc::new(SharedStoreMetric::default());
        m1.store(1);
        fence(Ordering::SeqCst);
        assert_eq!(1, m1.fetch());
    }

    #[test]
    fn test_serialize() {
        let s = serde_json::to_string(&SharedIncMetric(
            AtomicUsize::new(123),
            AtomicUsize::new(111),
        ));
        assert!(s.is_ok());
    }

    #[test]
    fn test_wraps_around() {
        let m = SharedStoreMetric(AtomicUsize::new(usize::MAX));
        m.add(1);
        assert_eq!(m.count(), 0);
    }
}
