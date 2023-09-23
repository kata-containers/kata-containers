// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, RwLock};

use dbs_utils::metric::{IncMetric, SharedIncMetric};
use lazy_static::lazy_static;
use serde::Serialize;

lazy_static! {
    /// Static instance used for handling metrics.
    pub static ref METRICS: RwLock<DragonballMetrics> = RwLock::new(DragonballMetrics::default());
}

/// Metrics specific to VCPUs' mode of functioning.
#[derive(Default, Serialize)]
pub struct VcpuMetrics {
    /// Number of KVM exits for handling input IO.
    pub exit_io_in: SharedIncMetric,
    /// Number of KVM exits for handling output IO.
    pub exit_io_out: SharedIncMetric,
    /// Number of KVM exits for handling MMIO reads.
    pub exit_mmio_read: SharedIncMetric,
    /// Number of KVM exits for handling MMIO writes.
    pub exit_mmio_write: SharedIncMetric,
    /// Number of errors during this VCPU's run.
    pub failures: SharedIncMetric,
    /// Failures in configuring the CPUID.
    pub filter_cpuid: SharedIncMetric,
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
    /// Metrics related to a vcpu's functioning.
    pub vcpu: VcpuMetrics,
    /// Metrics related to seccomp filtering.
    pub seccomp: SeccompMetrics,
    /// Metrics related to signals.
    pub signals: SignalMetrics,
}
