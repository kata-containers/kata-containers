// Copyright 2021-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

extern crate procfs;

use std::sync::Mutex;

use anyhow::{anyhow, Result};
use dbs_utils::metric::IncMetric;
use prometheus::{Encoder, IntCounter, IntGaugeVec, Opts, Registry, TextEncoder};

use crate::metric::METRICS;

const NAMESPACE_KATA_HYPERVISOR: &str = "kata_hypervisor";

lazy_static! {
    static ref REGISTERED: Mutex<bool> = Mutex::new(false);

    // custom registry
    static ref REGISTRY: Registry = Registry::new();

    // hypervisor metrics
    static ref HYPERVISOR_SCRAPE_COUNT: IntCounter =
    IntCounter::new(format!("{}_{}",NAMESPACE_KATA_HYPERVISOR,"scrape_count"), "Hypervisor metrics scrape count.").unwrap();

    static ref HYPERVISOR_VCPU: IntGaugeVec =
    IntGaugeVec::new(Opts::new(format!("{}_{}",NAMESPACE_KATA_HYPERVISOR,"vcpu"), "Hypervisor metrics specific to VCPUs' mode of functioning."), &["cpu_id", "item"]).unwrap();

    static ref HYPERVISOR_SECCOMP: IntGaugeVec =
    IntGaugeVec::new(Opts::new(format!("{}_{}",NAMESPACE_KATA_HYPERVISOR,"seccomp"), "Hypervisor metrics for the seccomp filtering."), &["item"]).unwrap();

    static ref HYPERVISOR_SIGNALS: IntGaugeVec =
    IntGaugeVec::new(Opts::new(format!("{}_{}",NAMESPACE_KATA_HYPERVISOR,"signals"), "Hypervisor metrics related to signals."), &["item"]).unwrap();
}

/// get prometheus metrics
pub fn get_hypervisor_metrics() -> Result<String> {
    let mut registered = REGISTERED
        .lock()
        .map_err(|e| anyhow!("failed to check hypervisor metrics register status {:?}", e))?;

    if !(*registered) {
        register_hypervisor_metrics()?;
        *registered = true;
    }

    update_hypervisor_metrics()?;

    // gather all metrics and return as a String
    let metric_families = REGISTRY.gather();

    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer)?;

    Ok(String::from_utf8(buffer)?)
}

fn register_hypervisor_metrics() -> Result<()> {
    REGISTRY.register(Box::new(HYPERVISOR_SCRAPE_COUNT.clone()))?;
    REGISTRY.register(Box::new(HYPERVISOR_VCPU.clone()))?;
    REGISTRY.register(Box::new(HYPERVISOR_SECCOMP.clone()))?;
    REGISTRY.register(Box::new(HYPERVISOR_SIGNALS.clone()))?;

    Ok(())
}

fn update_hypervisor_metrics() -> Result<()> {
    HYPERVISOR_SCRAPE_COUNT.inc();

    set_intgauge_vec_vcpu(&HYPERVISOR_VCPU);
    set_intgauge_vec_seccomp(&HYPERVISOR_SECCOMP);
    set_intgauge_vec_signals(&HYPERVISOR_SIGNALS);

    Ok(())
}

fn set_intgauge_vec_vcpu(icv: &prometheus::IntGaugeVec) {
    let metric_guard = METRICS.read().unwrap();
    for (cpu_id, metrics) in metric_guard.vcpu.iter() {
        icv.with_label_values(&[cpu_id.to_string().as_str(), "exit_io_in"])
            .set(metrics.exit_io_in.count() as i64);
        icv.with_label_values(&[cpu_id.to_string().as_str(), "exit_io_out"])
            .set(metrics.exit_io_out.count() as i64);
        icv.with_label_values(&[cpu_id.to_string().as_str(), "exit_mmio_read"])
            .set(metrics.exit_mmio_read.count() as i64);
        icv.with_label_values(&[cpu_id.to_string().as_str(), "exit_mmio_write"])
            .set(metrics.exit_mmio_write.count() as i64);
        icv.with_label_values(&[cpu_id.to_string().as_str(), "failures"])
            .set(metrics.failures.count() as i64);
        icv.with_label_values(&[cpu_id.to_string().as_str(), "filter_cpuid"])
            .set(metrics.filter_cpuid.count() as i64);
    }
}

fn set_intgauge_vec_seccomp(icv: &prometheus::IntGaugeVec) {
    let metric_guard = METRICS.read().unwrap();
    icv.with_label_values(&["num_faults"])
        .set(metric_guard.seccomp.num_faults.count() as i64);
}

fn set_intgauge_vec_signals(icv: &prometheus::IntGaugeVec) {
    let metric_guard = METRICS.read().unwrap();
    icv.with_label_values(&["sigbus"])
        .set(metric_guard.signals.sigbus.count() as i64);
    icv.with_label_values(&["sigsegv"])
        .set(metric_guard.signals.sigsegv.count() as i64);
}
