// Copyright 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

extern crate procfs;

use anyhow::{anyhow, Context, Result};

use prometheus::{Encoder, Gauge, IntCounter, Registry, TextEncoder};
use std::sync::Mutex;

const NAMESPACE_KATA_MONITOR: &str = "kata_ctl_monitor";

lazy_static! {

    static ref REGISTERED: Mutex<bool> = Mutex::new(false);

    // custom registry
    static ref REGISTRY: Registry = Registry::new();

    // monitor metrics
    static ref MONITOR_SCRAPE_COUNT: IntCounter =
    IntCounter::new(format!("{}_{}", NAMESPACE_KATA_MONITOR, "scrape_count"), "Monitor scrape count").unwrap();

    static ref MONITOR_MAX_FDS: Gauge = Gauge::new(format!("{}_{}", NAMESPACE_KATA_MONITOR, "process_max_fds"), "Open FDs for monitor").unwrap();

    static ref MONITOR_OPEN_FDS: Gauge = Gauge::new(format!("{}_{}", NAMESPACE_KATA_MONITOR, "process_open_fds"), "Open FDs for monitor").unwrap();

    static ref MONITOR_RESIDENT_MEMORY: Gauge = Gauge::new(format!("{}_{}", NAMESPACE_KATA_MONITOR, "process_resident_memory_bytes"), "Resident memory size in bytes for monitor").unwrap();
}

/// get monitor metrics
pub fn get_monitor_metrics() -> Result<String> {
    let mut registered = REGISTERED
        .lock()
        .map_err(|e| anyhow!("failed to check monitor metrics register status {:?}", e))?;

    if !(*registered) {
        register_monitor_metrics().context("failed to register monitor metrics")?;
        *registered = true;
    }

    update_monitor_metrics().context("failed to update monitor metrics")?;

    // gather all metrics and return as a String
    let metric_families = REGISTRY.gather();

    let mut buffer = Vec::new();
    TextEncoder::new()
        .encode(&metric_families, &mut buffer)
        .context("failed to encode gathered metrics")?;

    Ok(String::from_utf8(buffer)?)
}

fn register_monitor_metrics() -> Result<()> {
    REGISTRY.register(Box::new(MONITOR_SCRAPE_COUNT.clone()))?;
    REGISTRY.register(Box::new(MONITOR_MAX_FDS.clone()))?;
    REGISTRY.register(Box::new(MONITOR_OPEN_FDS.clone()))?;
    REGISTRY.register(Box::new(MONITOR_RESIDENT_MEMORY.clone()))?;

    Ok(())
}

fn update_monitor_metrics() -> Result<()> {
    MONITOR_SCRAPE_COUNT.inc();

    let me = match procfs::process::Process::myself() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("failed to create process instance: {:?}", e);

            return Ok(());
        }
    };

    if let Ok(fds) = procfs::sys::fs::file_max() {
        MONITOR_MAX_FDS.set(fds as f64);
    }

    if let Ok(fds) = me.fd_count() {
        MONITOR_OPEN_FDS.set(fds as f64);
    }

    if let Ok(statm) = me.statm() {
        MONITOR_RESIDENT_MEMORY.set(statm.resident as f64);
    }

    Ok(())
}
