// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

/*!
The Rust client library for [Prometheus](https://prometheus.io/).

Use of this library involves a few core concepts:

* A number of [`Counter`](type.Counter.html)s that represent metrics from your system.
* A [`Registry`](struct.Registry.html) with a number of registered [`Counter`s](type.Counter.html).
* An endpoint that calls [`gather`](fn.gather.html) which returns a
[`MetricFamily`](proto/struct.MetricFamily.html) through an [`Encoder`](trait.Encoder.html).

# Basic Example

```rust
use prometheus::{Opts, Registry, Counter, TextEncoder, Encoder};

// Create a Counter.
let counter_opts = Opts::new("test_counter", "test counter help");
let counter = Counter::with_opts(counter_opts).unwrap();

// Create a Registry and register Counter.
let r = Registry::new();
r.register(Box::new(counter.clone())).unwrap();

// Inc.
counter.inc();

// Gather the metrics.
let mut buffer = vec![];
let encoder = TextEncoder::new();
let metric_families = r.gather();
encoder.encode(&metric_families, &mut buffer).unwrap();

// Output to the standard output.
println!("{}", String::from_utf8(buffer).unwrap());
```

# Static Metrics

This crate supports staticly built metrics. You can use it with
[`lazy_static`](https://docs.rs/lazy_static/1.1.0/lazy_static/) to quickly build up and collect
some metrics.

```rust
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate prometheus;
use prometheus::{self, IntCounter, TextEncoder, Encoder};

lazy_static! {
    static ref HIGH_FIVE_COUNTER: IntCounter =
        register_int_counter!("highfives", "Number of high fives received").unwrap();
}

HIGH_FIVE_COUNTER.inc();
assert_eq!(HIGH_FIVE_COUNTER.get(), 1);
```

By default, this registers with a default registry. To make a report, you can call
[`gather`](fn.gather.html). This will return a family of metrics you can then feed through an
[`Encoder`](trait.Encoder.html) and report to Promethus.

```
# #[macro_use] extern crate lazy_static;
#[macro_use] extern crate prometheus;
# use prometheus::IntCounter;
use prometheus::{self, TextEncoder, Encoder};

// Register & measure some metrics.
# lazy_static! {
#     static ref HIGH_FIVE_COUNTER: IntCounter =
#        register_int_counter!("highfives", "Number of high fives received").unwrap();
# }
# HIGH_FIVE_COUNTER.inc();

let mut buffer = Vec::new();
let encoder = TextEncoder::new();

// Gather the metrics.
let metric_families = prometheus::gather();
// Encode them to send.
encoder.encode(&metric_families, &mut buffer).unwrap();

let output = String::from_utf8(buffer.clone()).unwrap();
const EXPECTED_OUTPUT: &'static str = "# HELP highfives Number of high fives received\n# TYPE highfives counter\nhighfives 1\n";
assert!(output.starts_with(EXPECTED_OUTPUT));
```

# Features

This library supports four features:

* `nightly`: Enable nightly only features.
* `push`: Enable push support.
* `process`: For collecting process info.

*/

#![allow(
    clippy::needless_pass_by_value,
    clippy::new_without_default,
    clippy::new_ret_no_self
)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

/// Protocol buffers format of metrics.
#[cfg(feature = "protobuf")]
#[allow(warnings)]
#[path = "../proto/proto_model.rs"]
pub mod proto;

#[cfg(feature = "protobuf")]
macro_rules! from_vec {
    ($e: expr) => {
        ::protobuf::RepeatedField::from_vec($e)
    };
}

#[cfg(not(feature = "protobuf"))]
#[path = "plain_model.rs"]
pub mod proto;

#[cfg(not(feature = "protobuf"))]
macro_rules! from_vec {
    ($e: expr) => {
        $e
    };
}

#[macro_use]
extern crate cfg_if;
#[macro_use]
extern crate lazy_static;

#[macro_use]
mod macros;
mod atomic64;
mod auto_flush;
mod counter;
mod desc;
mod encoder;
mod errors;
mod gauge;
mod histogram;
mod metrics;
#[cfg(feature = "push")]
mod push;
mod registry;
mod value;
mod vec;

// Public for generated code.
#[doc(hidden)]
pub mod timer;

#[cfg(all(feature = "process", target_os = "linux"))]
pub mod process_collector;

pub mod local {
    /*!

    Unsync local metrics, provides better performance.

    */
    pub use super::counter::{
        CounterWithValueType, LocalCounter, LocalCounterVec, LocalIntCounter, LocalIntCounterVec,
    };
    pub use super::histogram::{LocalHistogram, LocalHistogramTimer, LocalHistogramVec};
    pub use super::metrics::{LocalMetric, MayFlush};

    pub use super::auto_flush::{
        AFLocalCounter, AFLocalHistogram, CounterDelegator, HistogramDelegator,
    };
}

pub mod core {
    /*!

    Core traits and types.

    */

    pub use super::atomic64::*;
    pub use super::counter::{
        GenericCounter, GenericCounterVec, GenericLocalCounter, GenericLocalCounterVec,
    };
    pub use super::desc::{Desc, Describer};
    pub use super::gauge::{GenericGauge, GenericGaugeVec};
    pub use super::metrics::{Collector, Metric, Opts};
    pub use super::vec::{MetricVec, MetricVecBuilder};
}

pub use self::counter::{Counter, CounterVec, IntCounter, IntCounterVec};
pub use self::encoder::Encoder;
#[cfg(feature = "protobuf")]
pub use self::encoder::ProtobufEncoder;
pub use self::encoder::TextEncoder;
#[cfg(feature = "protobuf")]
pub use self::encoder::{PROTOBUF_FORMAT, TEXT_FORMAT};
pub use self::errors::{Error, Result};
pub use self::gauge::{Gauge, GaugeVec, IntGauge, IntGaugeVec};
pub use self::histogram::DEFAULT_BUCKETS;
pub use self::histogram::{exponential_buckets, linear_buckets};
pub use self::histogram::{Histogram, HistogramOpts, HistogramTimer, HistogramVec};
pub use self::metrics::Opts;
#[cfg(feature = "push")]
pub use self::push::{
    hostname_grouping_key, push_add_collector, push_add_metrics, push_collector, push_metrics,
    BasicAuthentication,
};
pub use self::registry::Registry;
pub use self::registry::{default_registry, gather, register, unregister};
