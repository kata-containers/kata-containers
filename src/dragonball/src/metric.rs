// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use dbs_utils::metric::SharedIncMetric;
use lazy_static::lazy_static;
use serde::Serialize;

pub use dbs_utils::metric::IncMetric;

lazy_static! {
    /// Static instance used for handling metrics.
    pub static ref METRICS: DragonballMetrics = DragonballMetrics::default();
}

/// Metrics for the seccomp filtering.
#[derive(Default, Serialize)]
pub struct SeccompMetrics {
    /// Number of errors inside the seccomp filtering.
    pub num_faults: SharedIncMetric,
}

/// Metrics related to signals.
#[derive(Default, Serialize)]
pub struct SignalMetrics {
    /// Number of times that SIGBUS was handled.
    pub sigbus: SharedIncMetric,
    /// Number of times that SIGSEGV was handled.
    pub sigsegv: SharedIncMetric,
}

/// Structure storing all metrics while enforcing serialization support on them.
#[derive(Default, Serialize)]
pub struct DragonballMetrics {
    /// Metrics related to seccomp filtering.
    pub seccomp: SeccompMetrics,
    /// Metrics related to signals.
    pub signals: SignalMetrics,
}
