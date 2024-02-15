// Copyright 2021-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

extern crate procfs;

use anyhow::{anyhow, Result};
use prometheus::{Encoder, Gauge, GaugeVec, Opts, Registry, TextEncoder};
use slog::warn;
use std::sync::Mutex;

const NAMESPACE_KATA_SHIM: &str = "kata_shim";

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "metrics"))
    };
}

lazy_static! {
    static ref REGISTERED: Mutex<bool> = Mutex::new(false);

    // custom registry
    static ref REGISTRY: Registry = Registry::new();

    // shim metrics
    static ref SHIM_THREADS: Gauge = Gauge::new(format!("{}_{}", NAMESPACE_KATA_SHIM, "threads"),"Kata containerd shim v2 process threads.").unwrap();

    static ref SHIM_PROC_STATUS: GaugeVec =
        GaugeVec::new(Opts::new(format!("{}_{}",NAMESPACE_KATA_SHIM,"proc_status"), "Kata containerd shim v2 process status."), &["item"]).unwrap();

    static ref SHIM_PROC_STAT: GaugeVec = GaugeVec::new(Opts::new(format!("{}_{}",NAMESPACE_KATA_SHIM,"proc_stat"), "Kata containerd shim v2 process statistics."), &["item"]).unwrap();

    static ref SHIM_IO_STAT: GaugeVec = GaugeVec::new(Opts::new(format!("{}_{}",NAMESPACE_KATA_SHIM,"io_stat"), "Kata containerd shim v2 process IO statistics."), &["item"]).unwrap();

    static ref SHIM_OPEN_FDS: Gauge = Gauge::new(format!("{}_{}", NAMESPACE_KATA_SHIM, "fds"), "Kata containerd shim v2 open FDs.").unwrap();
}

pub fn get_shim_metrics() -> Result<String> {
    let mut registered = REGISTERED
        .lock()
        .map_err(|e| anyhow!("failed to check shim metrics register status {:?}", e))?;

    if !(*registered) {
        register_shim_metrics()?;
        *registered = true;
    }

    update_shim_metrics()?;

    // gather all metrics and return as a String
    let metric_families = REGISTRY.gather();

    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    encoder.encode(&metric_families, &mut buffer)?;

    Ok(String::from_utf8(buffer)?)
}

fn register_shim_metrics() -> Result<()> {
    REGISTRY.register(Box::new(SHIM_THREADS.clone()))?;
    REGISTRY.register(Box::new(SHIM_PROC_STATUS.clone()))?;
    REGISTRY.register(Box::new(SHIM_PROC_STAT.clone()))?;
    REGISTRY.register(Box::new(SHIM_IO_STAT.clone()))?;
    REGISTRY.register(Box::new(SHIM_OPEN_FDS.clone()))?;

    // TODO:
    // REGISTRY.register(Box::new(RPC_DURATIONS_HISTOGRAM.clone()))?;
    // REGISTRY.register(Box::new(SHIM_POD_OVERHEAD_CPU.clone()))?;
    // REGISTRY.register(Box::new(SHIM_POD_OVERHEAD_MEMORY.clone()))?;

    Ok(())
}

fn update_shim_metrics() -> Result<()> {
    let me = procfs::process::Process::myself();

    let me = match me {
        Ok(p) => p,
        Err(e) => {
            warn!(sl!(), "failed to create process instance: {:?}", e);
            return Ok(());
        }
    };

    SHIM_THREADS.set(me.stat.num_threads as f64);

    match me.status() {
        Err(err) => error!(sl!(), "failed to get process status: {:?}", err),
        Ok(status) => set_gauge_vec_proc_status(&SHIM_PROC_STATUS, &status),
    }

    match me.stat() {
        Err(err) => {
            error!(sl!(), "failed to get process stat: {:?}", err);
        }
        Ok(stat) => {
            set_gauge_vec_proc_stat(&SHIM_PROC_STAT, &stat);
        }
    }

    match me.io() {
        Err(err) => {
            error!(sl!(), "failed to get process io stat: {:?}", err);
        }
        Ok(io) => {
            set_gauge_vec_proc_io(&SHIM_IO_STAT, &io);
        }
    }

    match me.fd_count() {
        Err(err) => {
            error!(sl!(), "failed to get process open fds number: {:?}", err);
        }
        Ok(fds) => {
            SHIM_OPEN_FDS.set(fds as f64);
        }
    }

    // TODO:
    // RPC_DURATIONS_HISTOGRAM & SHIM_POD_OVERHEAD_CPU & SHIM_POD_OVERHEAD_MEMORY

    Ok(())
}

fn set_gauge_vec_proc_status(gv: &prometheus::GaugeVec, status: &procfs::process::Status) {
    gv.with_label_values(&["vmpeak"])
        .set(status.vmpeak.unwrap_or(0) as f64);
    gv.with_label_values(&["vmsize"])
        .set(status.vmsize.unwrap_or(0) as f64);
    gv.with_label_values(&["vmlck"])
        .set(status.vmlck.unwrap_or(0) as f64);
    gv.with_label_values(&["vmpin"])
        .set(status.vmpin.unwrap_or(0) as f64);
    gv.with_label_values(&["vmhwm"])
        .set(status.vmhwm.unwrap_or(0) as f64);
    gv.with_label_values(&["vmrss"])
        .set(status.vmrss.unwrap_or(0) as f64);
    gv.with_label_values(&["rssanon"])
        .set(status.rssanon.unwrap_or(0) as f64);
    gv.with_label_values(&["rssfile"])
        .set(status.rssfile.unwrap_or(0) as f64);
    gv.with_label_values(&["rssshmem"])
        .set(status.rssshmem.unwrap_or(0) as f64);
    gv.with_label_values(&["vmdata"])
        .set(status.vmdata.unwrap_or(0) as f64);
    gv.with_label_values(&["vmstk"])
        .set(status.vmstk.unwrap_or(0) as f64);
    gv.with_label_values(&["vmexe"])
        .set(status.vmexe.unwrap_or(0) as f64);
    gv.with_label_values(&["vmlib"])
        .set(status.vmlib.unwrap_or(0) as f64);
    gv.with_label_values(&["vmpte"])
        .set(status.vmpte.unwrap_or(0) as f64);
    gv.with_label_values(&["vmswap"])
        .set(status.vmswap.unwrap_or(0) as f64);
    gv.with_label_values(&["hugetlbpages"])
        .set(status.hugetlbpages.unwrap_or(0) as f64);
    gv.with_label_values(&["voluntary_ctxt_switches"])
        .set(status.voluntary_ctxt_switches.unwrap_or(0) as f64);
    gv.with_label_values(&["nonvoluntary_ctxt_switches"])
        .set(status.nonvoluntary_ctxt_switches.unwrap_or(0) as f64);
}

fn set_gauge_vec_proc_stat(gv: &prometheus::GaugeVec, stat: &procfs::process::Stat) {
    gv.with_label_values(&["utime"]).set(stat.utime as f64);
    gv.with_label_values(&["stime"]).set(stat.stime as f64);
    gv.with_label_values(&["cutime"]).set(stat.cutime as f64);
    gv.with_label_values(&["cstime"]).set(stat.cstime as f64);
}

fn set_gauge_vec_proc_io(gv: &prometheus::GaugeVec, io_stat: &procfs::process::Io) {
    gv.with_label_values(&["rchar"]).set(io_stat.rchar as f64);
    gv.with_label_values(&["wchar"]).set(io_stat.wchar as f64);
    gv.with_label_values(&["syscr"]).set(io_stat.syscr as f64);
    gv.with_label_values(&["syscw"]).set(io_stat.syscw as f64);
    gv.with_label_values(&["read_bytes"])
        .set(io_stat.read_bytes as f64);
    gv.with_label_values(&["write_bytes"])
        .set(io_stat.write_bytes as f64);
    gv.with_label_values(&["cancelled_write_bytes"])
        .set(io_stat.cancelled_write_bytes as f64);
}
