// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

//! Monitor a process.
//!
//! This module only supports **Linux** platform.

use std::sync::Mutex;

use crate::counter::Counter;
use crate::desc::Desc;
use crate::gauge::Gauge;
use crate::metrics::{Collector, Opts};
use crate::proto;

/// The `pid_t` data type represents process IDs.
pub use libc::pid_t;

// Six metrics per ProcessCollector.
const MERTICS_NUMBER: usize = 6;

/// A collector which exports the current state of
/// process metrics including cpu, memory and file descriptor usage as well as
/// the process start time for the given process id.
#[derive(Debug)]
pub struct ProcessCollector {
    pid: pid_t,
    descs: Vec<Desc>,
    cpu_total: Mutex<Counter>,
    open_fds: Gauge,
    max_fds: Gauge,
    vsize: Gauge,
    rss: Gauge,
    start_time: Gauge,
}

impl ProcessCollector {
    /// Create a `ProcessCollector` with the given process id and namespace.
    pub fn new<S: Into<String>>(pid: pid_t, namespace: S) -> ProcessCollector {
        let namespace = namespace.into();
        let mut descs = Vec::new();

        let cpu_total = Counter::with_opts(
            Opts::new(
                "process_cpu_seconds_total",
                "Total user and system CPU time spent in \
                 seconds.",
            )
            .namespace(namespace.clone()),
        )
        .unwrap();
        descs.extend(cpu_total.desc().into_iter().cloned());

        let open_fds = Gauge::with_opts(
            Opts::new("process_open_fds", "Number of open file descriptors.")
                .namespace(namespace.clone()),
        )
        .unwrap();
        descs.extend(open_fds.desc().into_iter().cloned());

        let max_fds = Gauge::with_opts(
            Opts::new(
                "process_max_fds",
                "Maximum number of open file descriptors.",
            )
            .namespace(namespace.clone()),
        )
        .unwrap();
        descs.extend(max_fds.desc().into_iter().cloned());

        let vsize = Gauge::with_opts(
            Opts::new(
                "process_virtual_memory_bytes",
                "Virtual memory size in bytes.",
            )
            .namespace(namespace.clone()),
        )
        .unwrap();
        descs.extend(vsize.desc().into_iter().cloned());

        let rss = Gauge::with_opts(
            Opts::new(
                "process_resident_memory_bytes",
                "Resident memory size in bytes.",
            )
            .namespace(namespace.clone()),
        )
        .unwrap();
        descs.extend(rss.desc().into_iter().cloned());

        let start_time = Gauge::with_opts(
            Opts::new(
                "process_start_time_seconds",
                "Start time of the process since unix epoch \
                 in seconds.",
            )
            .namespace(namespace.clone()),
        )
        .unwrap();
        descs.extend(start_time.desc().into_iter().cloned());

        ProcessCollector {
            pid,
            descs,
            cpu_total: Mutex::new(cpu_total),
            open_fds,
            max_fds,
            vsize,
            rss,
            start_time,
        }
    }

    /// Return a `ProcessCollector` of the calling process.
    pub fn for_self() -> ProcessCollector {
        let pid = unsafe { libc::getpid() };
        ProcessCollector::new(pid, "")
    }
}

impl Collector for ProcessCollector {
    fn desc(&self) -> Vec<&Desc> {
        self.descs.iter().collect()
    }

    fn collect(&self) -> Vec<proto::MetricFamily> {
        let p = match procfs::process::Process::new(self.pid) {
            Ok(p) => p,
            Err(..) => {
                // we can't construct a Process object, so there's no stats to gather
                return Vec::new();
            }
        };

        // file descriptors
        if let Ok(fd_list) = p.fd() {
            self.open_fds.set(fd_list.len() as f64);
        }
        if let Ok(limits) = p.limits() {
            if let procfs::process::LimitValue::Value(max) = limits.max_open_files.soft_limit {
                self.max_fds.set(max as f64)
            }
        }

        // memory
        self.vsize.set(p.stat.vsize as f64);
        self.rss.set(p.stat.rss as f64 * *PAGESIZE);

        // proc_start_time
        if let Some(boot_time) = *BOOT_TIME {
            self.start_time
                .set(p.stat.starttime as f64 / *CLK_TCK + boot_time);
        }

        // cpu
        let cpu_total_mfs = {
            let cpu_total = self.cpu_total.lock().unwrap();
            let total = (p.stat.utime + p.stat.stime) as f64 / *CLK_TCK;
            let past = cpu_total.get();
            let delta = total - past;
            if delta > 0.0 {
                cpu_total.inc_by(delta);
            }

            cpu_total.collect()
        };

        // collect MetricFamilys.
        let mut mfs = Vec::with_capacity(MERTICS_NUMBER);
        mfs.extend(cpu_total_mfs);
        mfs.extend(self.open_fds.collect());
        mfs.extend(self.max_fds.collect());
        mfs.extend(self.vsize.collect());
        mfs.extend(self.rss.collect());
        mfs.extend(self.start_time.collect());
        mfs
    }
}

lazy_static! {
    // getconf CLK_TCK
    static ref CLK_TCK: f64 = {
        unsafe {
            libc::sysconf(libc::_SC_CLK_TCK) as f64
        }
    };

    // getconf PAGESIZE
    static ref PAGESIZE: f64 = {
        unsafe {
            libc::sysconf(libc::_SC_PAGESIZE) as f64
        }
    };
}

lazy_static! {
    static ref BOOT_TIME: Option<f64> = procfs::boot_time_secs().ok().map(|i| i as f64);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::Collector;
    use crate::registry;

    #[test]
    fn test_process_collector() {
        let pc = ProcessCollector::for_self();
        {
            // Six metrics per process collector.
            let descs = pc.desc();
            assert_eq!(descs.len(), super::MERTICS_NUMBER);
            let mfs = pc.collect();
            assert_eq!(mfs.len(), super::MERTICS_NUMBER);
        }

        let r = registry::Registry::new();
        let res = r.register(Box::new(pc));
        assert!(res.is_ok());
    }
}
