// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::cgroups::FreezerState;
use crate::cgroups::Manager as CgroupManager;
use crate::container::DEFAULT_DEVICES;
use crate::errors::*;
use lazy_static;
use libc::{self, pid_t};
use nix::errno::Errno;
use protobuf::{CachedSize, RepeatedField, SingularPtrField, UnknownFields};
use protocols::agent::{
    BlkioStats, BlkioStatsEntry, CgroupStats, CpuStats, CpuUsage, HugetlbStats, MemoryData,
    MemoryStats, PidsStats, ThrottlingData,
};
use protocols::oci::{LinuxDeviceCgroup, LinuxResources, LinuxThrottleDevice, LinuxWeightDevice};
use regex::Regex;
use std::collections::HashMap;
use std::fs;

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "cgroups"))
    };
}

pub struct CpuSet();
pub struct Cpu();
pub struct Devices();
pub struct Memory();
pub struct CpuAcct();
pub struct Pids();
pub struct Blkio();
pub struct HugeTLB();
pub struct NetCls();
pub struct NetPrio();
pub struct PerfEvent();
pub struct Freezer();
pub struct Named();

pub trait Subsystem {
    fn name(&self) -> String {
        "unknown".to_string()
    }

    fn set(&self, _dir: &str, _r: &LinuxResources, _update: bool) -> Result<()> {
        Ok(())
    }
}

pub const WILDCARD: i64 = -1;

lazy_static! {
    pub static ref DEFAULT_ALLOWED_DEVICES: Vec<LinuxDeviceCgroup> = {
        let mut v = Vec::new();
        v.push(LinuxDeviceCgroup {
            Allow: true,
            Type: "c".to_string(),
            Major: WILDCARD,
            Minor: WILDCARD,
            Access: "m".to_string(),
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });

        v.push(LinuxDeviceCgroup {
            Allow: true,
            Type: "b".to_string(),
            Major: WILDCARD,
            Minor: WILDCARD,
            Access: "m".to_string(),
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });

        v.push(LinuxDeviceCgroup {
            Allow: true,
            Type: "c".to_string(),
            Major: 5,
            Minor: 1,
            Access: "rwm".to_string(),
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });

        v.push(LinuxDeviceCgroup {
            Allow: true,
            Type: "c".to_string(),
            Major: 136,
            Minor: WILDCARD,
            Access: "rwm".to_string(),
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });

        v.push(LinuxDeviceCgroup {
            Allow: true,
            Type: "c".to_string(),
            Major: 5,
            Minor: 2,
            Access: "rwm".to_string(),
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });

        v.push(LinuxDeviceCgroup {
            Allow: true,
            Type: "c".to_string(),
            Major: 10,
            Minor: 200,
            Access: "rwm".to_string(),
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });

        v
    };
    pub static ref BMAP: HashMap<String, u128> = {
        let mut m = HashMap::new();
        m.insert("k".to_string(), KiB);
        m.insert("m".to_string(), MiB);
        m.insert("g".to_string(), GiB);
        m.insert("t".to_string(), TiB);
        m.insert("p".to_string(), PiB);
        m
    };
    pub static ref DMAP: HashMap<String, u128> = {
        let mut m = HashMap::new();
        m.insert("k".to_string(), KB);
        m.insert("m".to_string(), MB);
        m.insert("g".to_string(), GB);
        m.insert("t".to_string(), TB);
        m.insert("p".to_string(), PB);
        m
    };
    pub static ref DABBRS: Vec<String> = {
        let m = vec![
            "B".to_string(),
            "KB".to_string(),
            "MB".to_string(),
            "GB".to_string(),
            "TB".to_string(),
            "PB".to_string(),
            "EB".to_string(),
            "ZB".to_string(),
            "YB".to_string(),
        ];
        m
    };
    pub static ref BABBRS: Vec<String> = {
        let m = vec![
            "B".to_string(),
            "KiB".to_string(),
            "MiB".to_string(),
            "GiB".to_string(),
            "TiB".to_string(),
            "PiB".to_string(),
            "EiB".to_string(),
            "ZiB".to_string(),
            "YiB".to_string(),
        ];
        m
    };
    pub static ref HUGEPAGESIZES: Vec<String> = {
        let m = match get_hugepage_sizes() {
            Err(_) => Vec::new(),
            Ok(s) => s,
        };

        m
    };
}

pub const KB: u128 = 1000;
pub const MB: u128 = 1000 * KB;
pub const GB: u128 = 1000 * MB;
pub const TB: u128 = 1000 * GB;
pub const PB: u128 = 1000 * TB;

pub const KiB: u128 = 1024;
pub const MiB: u128 = 1024 * KiB;
pub const GiB: u128 = 1024 * MiB;
pub const TiB: u128 = 1024 * GiB;
pub const PiB: u128 = 1024 * TiB;

pub const HUGETLB_BASE: &'static str = "hugetlb";
pub const HUGETLB_USAGE: &'static str = "usage_in_bytes";
pub const HUGETLB_MAX_USAGE: &'static str = "max_usage_in_bytes";
pub const HUGETLB_FAILCNT: &'static str = "failcnt";

fn parse_size(s: &str, m: &HashMap<String, u128>) -> Result<u128> {
    let re = Regex::new(r"(?P<num>\d+)(?P<mul>[kKmMgGtTpP]?)[bB]?$")?;
    let caps = re.captures(s).unwrap();

    let num = caps.name("num");
    let size: u128 = if num.is_some() {
        num.unwrap().as_str().trim().parse::<u128>()?
    } else {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    };

    let q = caps.name("mul");
    let mul: u128 = if q.is_some() {
        let t = m.get(q.unwrap().as_str());
        if t.is_some() {
            *t.unwrap()
        } else {
            return Err(nix::Error::Sys(Errno::EINVAL).into());
        }
    } else {
        return Err(nix::Error::Sys(Errno::EINVAL).into());
    };

    Ok(size * mul)
}

fn custom_size(mut size: f64, base: f64, m: &Vec<String>) -> String {
    let mut i = 0;
    while size > base {
        size /= base;
        i += 1;
    }

    format!("{}{}", size, m[i].as_str())
}

pub const HUGEPAGESIZE_DIR: &'static str = "/sys/kernel/mm/hugepages";

fn get_hugepage_sizes() -> Result<Vec<String>> {
    let mut m = Vec::new();
    for e in fs::read_dir(HUGEPAGESIZE_DIR)? {
        let name = e?.file_name().into_string().unwrap();
        let parts: Vec<&str> = name.split('-').collect();
        if parts.len() != 2 {
            continue;
        }
        let size = parse_size(parts[1], &BMAP)?;
        m.push(custom_size(size as f64, 1024.0, DABBRS.as_ref()));
    }

    Ok(m)
}

pub const CPUSET_CPUS: &'static str = "cpuset.cpus";
pub const CPUSET_MEMS: &'static str = "cpuset.mems";
pub const CGROUP_PROCS: &'static str = "cgroup.procs";
pub const CPU_RT_PERIOD_US: &'static str = "cpu.rt_period_us";
pub const CPU_RT_RUNTIME_US: &'static str = "cpu.rt_runtime_us";
pub const CPU_SHARES: &'static str = "cpu.shares";
pub const CPU_CFS_QUOTA_US: &'static str = "cpu.cfs_quota_us";
pub const CPU_CFS_PERIOD_US: &'static str = "cpu.cfs_period_us";
pub const DEVICES_ALLOW: &'static str = "devices.allow";
pub const DEVICES_DENY: &'static str = "devices.deny";
pub const MEMORY_LIMIT: &'static str = "memory.limit_in_bytes";
pub const MEMORY_SOFT_LIMIT: &'static str = "memory.soft_limit_in_bytes";
pub const MEMSW_LIMIT: &'static str = "memory.memsw.limit_in_bytes";
pub const KMEM_LIMIT: &'static str = "memory.kmem.limit_in_bytes";
pub const KMEM_TCP_LIMIT: &'static str = "memory.kmem.tcp.limit_in_bytes";
pub const SWAPPINESS: &'static str = "memory.swappiness";
pub const OOM_CONTROL: &'static str = "memory.oom_control";
pub const PIDS_MAX: &'static str = "pids.max";
pub const BLKIO_WEIGHT: &'static str = "blkio.weight";
pub const BLKIO_LEAF_WEIGHT: &'static str = "blkio.leaf_weight";
pub const BLKIO_WEIGHT_DEVICE: &'static str = "blkio.weight_device";
pub const BLKIO_LEAF_WEIGHT_DEVICE: &'static str = "blkio.leaf_weight_device";
pub const BLKIO_READ_BPS_DEVICE: &'static str = "blkio.throttle.read_bps_device";
pub const BLKIO_WRITE_BPS_DEVICE: &'static str = "blkio.throttle.write_bps_device";
pub const BLKIO_READ_IOPS_DEVICE: &'static str = "blkio.throttle.read_iops_device";
pub const BLKIO_WRITE_IOPS_DEVICE: &'static str = "blkio.throttle.write_iops_device";
pub const NET_CLS_CLASSID: &'static str = "net_cls.classid";
pub const NET_PRIO_IFPRIOMAP: &'static str = "net_prio.ifpriomap";

pub const CPU_STAT: &'static str = "cpu.stat";
pub const CPUACCT_STAT: &'static str = "cpuacct.stat";
pub const NANO_PER_SECOND: u64 = 1000000000;
pub const CPUACCT_USAGE: &'static str = "cpuacct.usage";
pub const CPUACCT_PERCPU: &'static str = "cpuacct.usage_percpu";
pub const MEMORY_STAT: &'static str = "memory.stat";
pub const MEM_USAGE: &'static str = "usage_in_bytes";
pub const MEM_MAX_USAGE: &'static str = "max_usage_in_bytes";
pub const MEM_FAILCNT: &'static str = "failcnt";
pub const MEM_LIMIT: &'static str = "limit_in_bytes";
pub const MEM_HIERARCHY: &'static str = "memory.use_hierarchy";
pub const PIDS_CURRENT: &'static str = "pids.current";
pub const BLKIO_SECTORS: &'static str = "blkio.sectors_recursive";
pub const BLKIO_IO_SERVICE_BYTES: &'static str = "blkio.io_service_bytes_recursive";
pub const BLKIO_IO_SERVICED: &'static str = "blkio.io_serviced_recursive";
pub const BLKIO_IO_QUEUED: &'static str = "blkio.io_queued_recursive";
pub const BLKIO_IO_SERVICE_TIME: &'static str = "blkio.io_service_time_recursive";
pub const BLKIO_IO_WAIT_TIME: &'static str = "blkio.io_wait_time_recursive";
pub const BLKIO_IO_MERGED: &'static str = "blkio.io_merged_recursive";
pub const BLKIO_TIME: &'static str = "blkio.time_recursive";
pub const BLKIO_THROTTLE_IO_SERVICE_BYTES: &'static str = "blkio.throttle.io_service_bytes";
pub const BLKIO_THROTTLE_IO_SERVICED: &'static str = "blkio.throttle.io_serviced";

lazy_static! {
    pub static ref CLOCK_TICKS: f64 = {
        let n = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };

        n as f64
    };
}

pub fn init_static() {
    lazy_static::initialize(&DEFAULT_ALLOWED_DEVICES);
    lazy_static::initialize(&BMAP);
    lazy_static::initialize(&DMAP);
    lazy_static::initialize(&BABBRS);
    lazy_static::initialize(&DABBRS);
    lazy_static::initialize(&HUGEPAGESIZES);
    lazy_static::initialize(&CLOCK_TICKS);
}

fn write_file<T>(dir: &str, file: &str, v: T) -> Result<()>
where
    T: ToString,
{
    let p = format!("{}/{}", dir, file);
    info!(sl!(), "{}", p.as_str());
    fs::write(p.as_str(), v.to_string().as_bytes())?;
    Ok(())
}

fn read_file(dir: &str, file: &str) -> Result<String> {
    let p = format!("{}/{}", dir, file);
    let ret = fs::read_to_string(p.as_str())?;
    Ok(ret)
}

fn copy_parent(dir: &str, file: &str) -> Result<()> {
    let parent = if let Some(index) = dir.rfind('/') {
        &dir[..index]
    } else {
        return Err(ErrorKind::ErrorCode("cannot copy file from parent".to_string()).into());
    };

    match read_file(parent, file) {
        Ok(v) => {
            if !v.trim().is_empty() {
                info!(sl!(), "value: \"{}\"", v.as_str().trim());
                return write_file(dir, file, v);
            } else {
                copy_parent(parent, file)?;
                return copy_parent(dir, file);
            }
        }
        Err(Error(ErrorKind::Io(e), _)) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                copy_parent(parent, file)?;
                return copy_parent(dir, file);
            }
            return Err(ErrorKind::Io(e).into());
        }
        Err(e) => return Err(e.into()),
    }
}

fn write_nonzero(dir: &str, file: &str, v: i128) -> Result<()> {
    if v != 0 {
        write_file(dir, file, v.to_string())?;
    }

    Ok(())
}

fn try_write_nonzero(dir: &str, file: &str, v: i128) -> Result<()> {
    match write_nonzero(dir, file, v) {
        Ok(_) => Ok(()),
        Err(Error(ErrorKind::Io(e), _)) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                return Ok(());
            } else {
                return Err(ErrorKind::Io(e).into());
            }
        }
        e => e,
    }
}

fn remove(dir: &str) -> Result<()> {
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn apply(dir: &str, pid: pid_t) -> Result<()> {
    write_file(dir, CGROUP_PROCS, pid)?;
    Ok(())
}

fn try_write_file<T: ToString>(dir: &str, file: &str, v: T) -> Result<()> {
    match write_file(dir, file, v) {
        Err(e) => {
            let err = Errno::last();
            if err == Errno::EINVAL || err == Errno::ENODEV || err == Errno::ERANGE {
                warn!(sl!(), "Invalid Arguments!");
                return Ok(());
            }

            info!(sl!(), "{}", err.desc());

            return Err(e);
        }

        Ok(_) => {}
    }

    Ok(())
}

impl Subsystem for CpuSet {
    fn name(&self) -> String {
        "cpuset".to_string()
    }

    fn set(&self, dir: &str, r: &LinuxResources, update: bool) -> Result<()> {
        let mut cpus: &str = "";
        let mut mems: &str = "";

        if r.CPU.is_some() {
            let cpu = r.CPU.as_ref().unwrap();
            cpus = cpu.Cpus.as_str();
            mems = cpu.Mems.as_str();
        }

        // For updatecontainer, just set the new value
        if update {
            if !cpus.is_empty() {
                try_write_file(dir, CPUSET_CPUS, cpus)?;
            }

            if !mems.is_empty() {
                try_write_file(dir, CPUSET_MEMS, mems)?;
            }

            return Ok(());
        }

        // for the first time

        if !update {
            copy_parent(dir, CPUSET_CPUS)?;
            copy_parent(dir, CPUSET_MEMS)?;
        }

        // cpuset and mems can be invalid
        // how to deal with it? Just ingore error for now
        if !cpus.is_empty() {
            try_write_file(dir, CPUSET_CPUS, cpus)?;
        }

        if !mems.is_empty() {
            info!(sl!(), "{}", mems);
            try_write_file(dir, CPUSET_MEMS, mems)?;
        }

        Ok(())
    }
}

impl Subsystem for Cpu {
    fn name(&self) -> String {
        "cpu".to_string()
    }

    fn set(&self, dir: &str, r: &LinuxResources, _update: bool) -> Result<()> {
        if r.CPU.is_none() {
            return Ok(());
        }

        let cpu = r.CPU.as_ref().unwrap();

        try_write_nonzero(dir, CPU_RT_PERIOD_US, cpu.RealtimePeriod as i128)?;
        try_write_nonzero(dir, CPU_RT_RUNTIME_US, cpu.RealtimeRuntime as i128)?;
        write_nonzero(dir, CPU_SHARES, cpu.Shares as i128)?;
        write_nonzero(dir, CPU_CFS_QUOTA_US, cpu.Quota as i128)?;
        write_nonzero(dir, CPU_CFS_PERIOD_US, cpu.Period as i128)?;

        Ok(())
    }
}

fn get_param_key_value(dir: &str, file: &str) -> Result<HashMap<String, String>> {
    let mut m = HashMap::new();
    let p = format!("{}/{}", dir, file);

    for l in fs::read_to_string(p.as_str())?.lines() {
        let t: Vec<&str> = l.split(' ').collect();
        if t.len() != 2 {
            continue;
        }

        m.insert(t[0].to_string(), t[1].to_string());
    }

    Ok(m)
}

fn get_param_key_u64(dir: &str, file: &str) -> Result<HashMap<String, u64>> {
    let mut m = HashMap::new();
    let p = format!("{}/{}", dir, file);

    for l in fs::read_to_string(p.as_str())?.lines() {
        let t: Vec<&str> = l.split(' ').collect();
        if t.len() != 2 {
            continue;
        }

        m.insert(t[0].to_string(), t[1].trim().parse::<u64>()?);
    }

    Ok(m)
}

fn get_param_u64(dir: &str, file: &str) -> Result<u64> {
    let p = format!("{}/{}", dir, file);
    let ret = fs::read_to_string(p.as_str())?.trim().parse::<u64>()?;
    Ok(ret)
}

impl Cpu {
    fn get_stats(&self, dir: &str) -> Result<ThrottlingData> {
        let h = get_param_key_u64(dir, CPU_STAT)?;

        Ok(ThrottlingData {
            periods: *h.get("nr_periods").unwrap(),
            throttled_periods: *h.get("nr_throttled").unwrap(),
            throttled_time: *h.get("throttled_time").unwrap(),
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        })
    }
}

impl Subsystem for CpuAcct {
    fn name(&self) -> String {
        "cpuacct".to_string()
    }

    fn set(&self, _dir: &str, _r: &LinuxResources, _update: bool) -> Result<()> {
        Ok(())
    }
}

fn get_cpuacct_percpu_usage(dir: &str) -> Result<Vec<u64>> {
    let mut m = Vec::new();
    let file = format!("{}/{}", dir, CPUACCT_PERCPU);

    for n in fs::read_to_string(file.as_str())?.split(' ') {
        m.push(n.trim().parse::<u64>()?);
    }

    Ok(m)
}

fn get_percpu_usage(dir: &str, file: &str) -> Result<Vec<u64>> {
    let mut m = Vec::new();
    let p = format!("{}/{}", dir, file);
    info!(sl!(), "{}", p.as_str());

    for n in fs::read_to_string(p.as_str())?.split(' ') {
        info!(sl!(), "{}", n);
        if !n.trim().is_empty() {
            m.push(n.trim().parse::<u64>()?);
        }
    }

    Ok(m)
}

impl CpuAcct {
    fn get_stats(&self, dir: &str) -> Result<CpuUsage> {
        let h = get_param_key_u64(dir, CPUACCT_STAT)?;

        let usage_in_usermode =
            (((*h.get("user").unwrap() * NANO_PER_SECOND) as f64) / *CLOCK_TICKS) as u64;
        let usage_in_kernelmode =
            (((*h.get("system").unwrap() * NANO_PER_SECOND) as f64) / *CLOCK_TICKS) as u64;

        info!(sl!(), "stat");

        let total_usage = get_param_u64(dir, CPUACCT_USAGE)?;
        info!(sl!(), "usage");

        let percpu_usage = get_percpu_usage(dir, CPUACCT_PERCPU)?;
        info!(sl!(), "percpu");

        Ok(CpuUsage {
            total_usage,
            percpu_usage,
            usage_in_kernelmode,
            usage_in_usermode,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        })
    }
}

fn write_device(d: &LinuxDeviceCgroup, dir: &str) -> Result<()> {
    let file = if d.Allow { DEVICES_ALLOW } else { DEVICES_DENY };

    let major = if d.Major == WILDCARD {
        "*".to_string()
    } else {
        d.Major.to_string()
    };

    let minor = if d.Minor == WILDCARD {
        "*".to_string()
    } else {
        d.Minor.to_string()
    };

    let t = if d.Type.is_empty() {
        "a"
    } else {
        d.Type.as_str()
    };

    let v = format!(
        "{} {}:{} {}",
        t,
        major.as_str(),
        minor.as_str(),
        d.Access.as_str()
    );

    info!(sl!(), "{}", v.as_str());

    write_file(dir, file, v.as_str())
}

impl Subsystem for Devices {
    fn name(&self) -> String {
        "devices".to_string()
    }

    fn set(&self, dir: &str, r: &LinuxResources, _update: bool) -> Result<()> {
        for d in r.Devices.iter() {
            write_device(d, dir)?;
        }

        for d in DEFAULT_DEVICES.iter() {
            let td = LinuxDeviceCgroup {
                Allow: true,
                Type: d.Type.clone(),
                Major: d.Major,
                Minor: d.Minor,
                Access: "rwm".to_string(),
                unknown_fields: UnknownFields::default(),
                cached_size: CachedSize::default(),
            };

            write_device(&td, dir)?;
        }

        for d in DEFAULT_ALLOWED_DEVICES.iter() {
            write_device(d, dir)?;
        }

        Ok(())
    }
}

fn try_write<T>(dir: &str, file: &str, v: T) -> Result<()>
where
    T: ToString,
{
    match write_file(dir, file, v) {
        Err(Error(ErrorKind::Io(e), _)) => {
            if e.kind() != std::io::ErrorKind::PermissionDenied
                && e.kind() != std::io::ErrorKind::Other
            {
                return Err(ErrorKind::Io(e).into());
            }

            return Ok(());
        }

        Err(e) => return Err(e.into()),

        Ok(_) => return Ok(()),
    }
}

impl Subsystem for Memory {
    fn name(&self) -> String {
        "memory".to_string()
    }

    fn set(&self, dir: &str, r: &LinuxResources, update: bool) -> Result<()> {
        if r.Memory.is_none() {
            return Ok(());
        }

        let memory = r.Memory.as_ref().unwrap();
        // initialize kmem limits for accounting
        if !update {
            try_write(dir, KMEM_LIMIT, 1)?;
            try_write(dir, KMEM_LIMIT, -1)?;
        }

        write_nonzero(dir, MEMORY_LIMIT, memory.Limit as i128)?;
        write_nonzero(dir, MEMORY_SOFT_LIMIT, memory.Reservation as i128)?;

        try_write_nonzero(dir, MEMSW_LIMIT, memory.Swap as i128)?;
        try_write_nonzero(dir, KMEM_LIMIT, memory.Kernel as i128)?;

        write_nonzero(dir, KMEM_TCP_LIMIT, memory.KernelTCP as i128)?;

        if memory.Swappiness <= 100 {
            write_file(dir, SWAPPINESS, memory.Swappiness)?;
        }

        if memory.DisableOOMKiller {
            write_file(dir, OOM_CONTROL, 1)?;
        }

        Ok(())
    }
}

fn get_exist_memory_data(dir: &str, sub: &str) -> Result<Option<MemoryData>> {
    let res = match get_memory_data(dir, sub) {
        Err(Error(ErrorKind::Io(e), _)) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                None
            } else {
                return Err(ErrorKind::Io(e).into());
            }
        }

        Ok(r) => Some(r),

        Err(e) => return Err(e.into()),
    };

    Ok(res)
}

fn get_memory_data(dir: &str, sub: &str) -> Result<MemoryData> {
    let base = "memory";
    let (fusage, fmax_usage, ffailcnt, flimit) = if sub.is_empty() {
        (
            format!("{}.{}", base, MEM_USAGE),
            format!("{}.{}", base, MEM_MAX_USAGE),
            format!("{}.{}", base, MEM_FAILCNT),
            format!("{}.{}", base, MEM_LIMIT),
        )
    } else {
        (
            format!("{}.{}.{}", base, sub, MEM_USAGE),
            format!("{}.{}.{}", base, sub, MEM_MAX_USAGE),
            format!("{}.{}.{}", base, sub, MEM_FAILCNT),
            format!("{}.{}.{}", base, sub, MEM_LIMIT),
        )
    };

    let usage = get_param_u64(dir, fusage.as_str())?;
    let max_usage = get_param_u64(dir, fmax_usage.as_str())?;
    let failcnt = get_param_u64(dir, ffailcnt.as_str())?;
    let limit = get_param_u64(dir, flimit.as_str())?;

    Ok(MemoryData {
        usage,
        max_usage,
        failcnt,
        limit,
        unknown_fields: UnknownFields::default(),
        cached_size: CachedSize::default(),
    })
}

impl Memory {
    fn get_stats(&self, dir: &str) -> Result<MemoryStats> {
        let h = get_param_key_u64(dir, MEMORY_STAT)?;
        let cache = *h.get("cache").unwrap();
        info!(sl!(), "cache");

        let value = get_param_u64(dir, MEM_HIERARCHY)?;
        let use_hierarchy = if value == 1 { true } else { false };

        info!(sl!(), "hierarchy");

        // gte memory datas
        let usage = SingularPtrField::from_option(get_exist_memory_data(dir, "")?);
        let swap_usage = SingularPtrField::from_option(get_exist_memory_data(dir, "memsw")?);
        let kernel_usage = SingularPtrField::from_option(get_exist_memory_data(dir, "kmem")?);

        Ok(MemoryStats {
            cache,
            usage,
            swap_usage,
            kernel_usage,
            use_hierarchy,
            stats: h,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        })
    }
}

impl Subsystem for Pids {
    fn name(&self) -> String {
        "pids".to_string()
    }

    fn set(&self, dir: &str, r: &LinuxResources, _update: bool) -> Result<()> {
        if r.Pids.is_none() {
            return Ok(());
        }

        let pids = r.Pids.as_ref().unwrap();

        let v = if pids.Limit > 0 {
            pids.Limit.to_string()
        } else {
            "max".to_string()
        };

        write_file(dir, PIDS_MAX, v.as_str())?;

        Ok(())
    }
}

fn get_param_string(dir: &str, file: &str) -> Result<String> {
    let p = format!("{}/{}", dir, file);

    let c = fs::read_to_string(p.as_str())?;

    Ok(c)
}

impl Pids {
    fn get_stats(&self, dir: &str) -> Result<PidsStats> {
        let current = get_param_u64(dir, PIDS_CURRENT)?;
        let c = get_param_string(dir, PIDS_MAX)?;

        let limit = if c.contains("max") {
            0
        } else {
            c.trim().parse::<u64>()?
        };

        Ok(PidsStats {
            current,
            limit,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        })
    }
}

#[inline]
fn weight(d: &LinuxWeightDevice) -> (String, String) {
    (
        format!("{}:{} {}", d.Major, d.Minor, d.Weight),
        format!("{}:{} {}", d.Major, d.Minor, d.LeafWeight),
    )
}

#[inline]
fn rate(d: &LinuxThrottleDevice) -> String {
    format!("{}:{} {}", d.Major, d.Minor, d.Rate)
}

fn write_blkio_device<T: ToString>(dir: &str, file: &str, v: T) -> Result<()> {
    match write_file(dir, file, v) {
        Err(Error(ErrorKind::Io(e), _)) => {
            // only ignore ENODEV
            if e.kind() == std::io::ErrorKind::Other {
                let raw = std::io::Error::last_os_error().raw_os_error().unwrap();
                if Errno::from_i32(raw) == Errno::ENODEV {
                    return Ok(());
                }
            }

            return Err(ErrorKind::Io(e).into());
        }

        Err(e) => return Err(e.into()),

        Ok(_) => {}
    }

    Ok(())
}

impl Subsystem for Blkio {
    fn name(&self) -> String {
        "blkio".to_string()
    }

    fn set(&self, dir: &str, r: &LinuxResources, _update: bool) -> Result<()> {
        if r.BlockIO.is_none() {
            return Ok(());
        }

        let blkio = r.BlockIO.as_ref().unwrap();

        write_nonzero(dir, BLKIO_WEIGHT, blkio.Weight as i128)?;
        write_nonzero(dir, BLKIO_LEAF_WEIGHT, blkio.LeafWeight as i128)?;

        for d in blkio.WeightDevice.iter() {
            let (w, lw) = weight(d);
            write_blkio_device(dir, BLKIO_WEIGHT_DEVICE, w)?;
            write_blkio_device(dir, BLKIO_LEAF_WEIGHT_DEVICE, lw)?;
        }

        for d in blkio.ThrottleReadBpsDevice.iter() {
            write_blkio_device(dir, BLKIO_READ_BPS_DEVICE, rate(d))?;
        }

        for d in blkio.ThrottleWriteBpsDevice.iter() {
            write_blkio_device(dir, BLKIO_WRITE_BPS_DEVICE, rate(d))?;
        }

        for d in blkio.ThrottleReadIOPSDevice.iter() {
            write_blkio_device(dir, BLKIO_READ_IOPS_DEVICE, rate(d))?;
        }

        for d in blkio.ThrottleWriteIOPSDevice.iter() {
            write_blkio_device(dir, BLKIO_WRITE_IOPS_DEVICE, rate(d))?;
        }

        Ok(())
    }
}

fn get_blkio_stat(dir: &str, file: &str) -> Result<RepeatedField<BlkioStatsEntry>> {
    let p = format!("{}/{}", dir, file);
    let mut m = RepeatedField::new();

    for l in fs::read_to_string(p.as_str())?.lines() {
        let parts: Vec<&str> = l.split(' ').collect();

        if parts.len() < 3 {
            if parts.len() == 2 && parts[0].to_lowercase() == "total".to_string() {
                continue;
            } else {
                return Err(nix::Error::Sys(Errno::EINVAL).into());
            }
        }

        let op = parts[1].to_string();
        let value = parts[2].parse::<u64>()?;

        let devno: Vec<&str> = parts[0].split(':').collect();

        if devno.len() != 2 {
            return Err(nix::Error::Sys(Errno::EINVAL).into());
        }

        let major = devno[0].parse::<u64>()?;
        let minor = devno[1].parse::<u64>()?;

        m.push(BlkioStatsEntry {
            major,
            minor,
            op,
            value,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        });
    }

    /*
        if m.len() == 0 {
            // return Err here? not sure about it
            return Err(nix::Error::Sys(Errno:ENODATA).into());
        }
    */

    Ok(m)
}
impl Blkio {
    fn get_stats(&self, dir: &str) -> Result<BlkioStats> {
        let mut m = BlkioStats::new();
        let entry = get_blkio_stat(dir, BLKIO_IO_SERVICED)?;

        if entry.len() == 0 {
            // fall back to generic stats
            // blkio.throttle.io_service_bytes,
            // maybe io_service_bytes_recursive?
            // stick to runc for now
            m.io_service_bytes_recursive = get_blkio_stat(dir, BLKIO_THROTTLE_IO_SERVICE_BYTES)?;
            m.io_serviced_recursive = get_blkio_stat(dir, BLKIO_THROTTLE_IO_SERVICED)?;
        } else {
            // cfq stats
            m.sectors_recursive = get_blkio_stat(dir, BLKIO_SECTORS)?;
            m.io_service_bytes_recursive = get_blkio_stat(dir, BLKIO_IO_SERVICE_BYTES)?;
            m.io_serviced_recursive = get_blkio_stat(dir, BLKIO_IO_SERVICED)?;
            m.io_queued_recursive = get_blkio_stat(dir, BLKIO_IO_QUEUED)?;
            m.io_service_time_recursive = get_blkio_stat(dir, BLKIO_IO_SERVICE_TIME)?;
            m.io_wait_time_recursive = get_blkio_stat(dir, BLKIO_IO_WAIT_TIME)?;
            m.io_merged_recursive = get_blkio_stat(dir, BLKIO_IO_MERGED)?;
            m.io_time_recursive = get_blkio_stat(dir, BLKIO_TIME)?;
        }

        Ok(m)
    }
}

impl Subsystem for HugeTLB {
    fn name(&self) -> String {
        "hugetlb".to_string()
    }

    fn set(&self, dir: &str, r: &LinuxResources, _update: bool) -> Result<()> {
        for l in r.HugepageLimits.iter() {
            let file = format!("hugetlb.{}.limit_in_bytes", l.Pagesize);
            write_file(dir, file.as_str(), l.Limit)?;
        }
        Ok(())
    }
}

impl HugeTLB {
    fn get_stats(&self, dir: &str) -> Result<HashMap<String, HugetlbStats>> {
        let mut h = HashMap::new();
        for pagesize in HUGEPAGESIZES.iter() {
            let fusage = format!("{}.{}.{}", HUGETLB_BASE, pagesize, HUGETLB_USAGE);
            let fmax = format!("{}.{}.{}", HUGETLB_BASE, pagesize, HUGETLB_MAX_USAGE);
            let ffailcnt = format!("{}.{}.{}", HUGETLB_BASE, pagesize, HUGETLB_FAILCNT);

            let usage = get_param_u64(dir, fusage.as_str())?;
            let max_usage = get_param_u64(dir, fmax.as_str())?;
            let failcnt = get_param_u64(dir, ffailcnt.as_str())?;

            h.insert(
                pagesize.to_string(),
                HugetlbStats {
                    usage,
                    max_usage,
                    failcnt,
                    unknown_fields: UnknownFields::default(),
                    cached_size: CachedSize::default(),
                },
            );
        }

        Ok(h)
    }
}

impl Subsystem for NetCls {
    fn name(&self) -> String {
        "net_cls".to_string()
    }

    fn set(&self, dir: &str, r: &LinuxResources, _update: bool) -> Result<()> {
        if r.Network.is_none() {
            return Ok(());
        }

        let network = r.Network.as_ref().unwrap();

        write_nonzero(dir, NET_CLS_CLASSID, network.ClassID as i128)?;

        Ok(())
    }
}

impl Subsystem for NetPrio {
    fn name(&self) -> String {
        "net_prio".to_string()
    }

    fn set(&self, dir: &str, r: &LinuxResources, _update: bool) -> Result<()> {
        if r.Network.is_none() {
            return Ok(());
        }

        let network = r.Network.as_ref().unwrap();

        for p in network.Priorities.iter() {
            let prio = format!("{} {}", p.Name, p.Priority);

            try_write_file(dir, NET_PRIO_IFPRIOMAP, prio)?;
        }

        Ok(())
    }
}

impl Subsystem for PerfEvent {
    fn name(&self) -> String {
        "perf_event".to_string()
    }

    fn set(&self, _dir: &str, _r: &LinuxResources, _update: bool) -> Result<()> {
        Ok(())
    }
}

impl Subsystem for Freezer {
    fn name(&self) -> String {
        "freezer".to_string()
    }

    fn set(&self, _dir: &str, _r: &LinuxResources, _update: bool) -> Result<()> {
        Ok(())
    }
}

impl Subsystem for Named {
    fn name(&self) -> String {
        "name=systemd".to_string()
    }

    fn set(&self, _dir: &str, _r: &LinuxResources, _update: bool) -> Result<()> {
        Ok(())
    }
}

fn get_subsystem(name: &str) -> Result<Box<dyn Subsystem>> {
    match name {
        "cpuset" => Ok(Box::new(CpuSet())),
        "cpu" => Ok(Box::new(Cpu())),
        "devices" => Ok(Box::new(Devices())),
        "memory" => Ok(Box::new(Memory())),
        "cpuacct" => Ok(Box::new(CpuAcct())),
        "pids" => Ok(Box::new(Pids())),
        "blkio" => Ok(Box::new(Blkio())),
        "hugetlb" => Ok(Box::new(HugeTLB())),
        "net_cls" => Ok(Box::new(NetCls())),
        "net_prio" => Ok(Box::new(NetPrio())),
        "perf_event" => Ok(Box::new(PerfEvent())),
        "freezer" => Ok(Box::new(Freezer())),
        "name=systemd" => Ok(Box::new(Named())),
        _ => Err(nix::Error::Sys(Errno::EINVAL).into()),
    }
}

pub const PATHS: &'static str = "/proc/self/cgroup";
pub const MOUNTS: &'static str = "/proc/self/mountinfo";

fn get_paths() -> Result<HashMap<String, String>> {
    let mut m = HashMap::new();
    for l in fs::read_to_string(PATHS)?.lines() {
        let fl: Vec<&str> = l.split(':').collect();
        if fl.len() != 3 {
            info!(sl!(), "Corrupted cgroup data!");
            continue;
        }

        let keys: Vec<&str> = fl[1].split(',').collect();
        for key in &keys {
            m.insert(key.to_string(), fl[2].to_string());
        }
    }
    Ok(m)
}

fn get_mounts() -> Result<HashMap<String, String>> {
    let mut m = HashMap::new();
    let paths = get_paths()?;

    for l in fs::read_to_string(MOUNTS)?.lines() {
        let p: Vec<&str> = l.split(" - ").collect();
        let pre: Vec<&str> = p[0].split(' ').collect();
        let post: Vec<&str> = p[1].split(' ').collect();

        if post.len() != 3 {
            warn!(sl!(), "mountinfo corrupted!");
            continue;
        }

        if post[0] != "cgroup" && post[0] != "cgroup2" {
            continue;
        }

        let names: Vec<&str> = post[2].split(',').collect();

        for name in &names {
            if paths.contains_key(*name) {
                m.insert(name.to_string(), pre[4].to_string());
            }
        }
    }

    Ok(m)
}

fn get_procs(dir: &str) -> Result<Vec<i32>> {
    let file = format!("{}/{}", dir, CGROUP_PROCS);
    let mut m = Vec::new();

    for l in fs::read_to_string(file.as_str())?.lines() {
        m.push(l.trim().parse::<i32>()?);
    }

    Ok(m)
}

fn get_all_procs(dir: &str) -> Result<Vec<i32>> {
    let mut m = Vec::new();

    for e in fs::read_dir(dir)? {
        let path = e?.path();

        if path.is_dir() {
            m.append(get_all_procs(path.to_str().unwrap())?.as_mut());
        }

        if path.is_file() && path.ends_with(CGROUP_PROCS) {
            let dir = path.parent().unwrap().to_str().unwrap();

            m.append(get_procs(dir)?.as_mut());

            return Ok(m);
        }

        if path.is_file() {
            continue;
        }
    }

    Ok(m)
}

#[derive(Debug, Clone)]
pub struct Manager {
    pub paths: HashMap<String, String>,
    pub mounts: HashMap<String, String>,
    pub rels: HashMap<String, String>,
    pub cpath: String,
}

pub const THAWED: &'static str = "THAWED";
pub const FROZEN: &'static str = "FROZEN";

impl CgroupManager for Manager {
    fn apply(&self, pid: pid_t) -> Result<()> {
        for (key, value) in &self.paths {
            info!(sl!(), "apply cgroup {}", key);
            apply(value, pid)?;
        }

        Ok(())
    }

    fn set(&self, spec: &LinuxResources, update: bool) -> Result<()> {
        for (key, value) in &self.paths {
            let _ = fs::create_dir_all(value);
            let sub = get_subsystem(key)?;
            info!(sl!(), "setting cgroup {}", key);
            sub.set(value, spec, update)?;
        }

        Ok(())
    }

    fn get_stats(&self) -> Result<CgroupStats> {
        // CpuStats
        info!(sl!(), "cpu_usage");
        let cpu_usage = if self.paths.get("cpuacct").is_some() {
            SingularPtrField::some(CpuAcct().get_stats(self.paths.get("cpuacct").unwrap())?)
        } else {
            SingularPtrField::none()
        };

        info!(sl!(), "throttling_data");
        let throttling_data = if self.paths.get("cpu").is_some() {
            SingularPtrField::some(Cpu().get_stats(self.paths.get("cpu").unwrap())?)
        } else {
            SingularPtrField::none()
        };

        info!(sl!(), "cpu_stats");
        let cpu_stats = if cpu_usage.is_none() && throttling_data.is_none() {
            SingularPtrField::none()
        } else {
            SingularPtrField::some(CpuStats {
                cpu_usage,
                throttling_data,
                unknown_fields: UnknownFields::default(),
                cached_size: CachedSize::default(),
            })
        };

        // Memorystats
        info!(sl!(), "memory_stats");
        let memory_stats = if self.paths.get("memory").is_some() {
            SingularPtrField::some(Memory().get_stats(self.paths.get("memory").unwrap())?)
        } else {
            SingularPtrField::none()
        };

        // PidsStats
        info!(sl!(), "pids_stats");
        let pids_stats = if self.paths.get("pids").is_some() {
            SingularPtrField::some(Pids().get_stats(self.paths.get("pids").unwrap())?)
        } else {
            SingularPtrField::none()
        };

        // BlkioStats
        info!(sl!(), "blkio_stats");
        let blkio_stats = if self.paths.get("blkio").is_some() {
            SingularPtrField::some(Blkio().get_stats(self.paths.get("blkio").unwrap())?)
        } else {
            SingularPtrField::none()
        };

        // HugetlbStats
        info!(sl!(), "hugetlb_stats");
        let hugetlb_stats = if self.paths.get("hugetlb").is_some() {
            HugeTLB().get_stats(self.paths.get("hugetlb").unwrap())?
        } else {
            HashMap::new()
        };

        Ok(CgroupStats {
            cpu_stats,
            memory_stats,
            pids_stats,
            blkio_stats,
            hugetlb_stats,
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        })
    }

    fn get_paths(&self) -> Result<HashMap<String, String>> {
        Ok(self.paths.clone())
    }

    fn freeze(&self, state: FreezerState) -> Result<()> {
        if state == THAWED || state == FROZEN {
            if self.paths.get("freezer").is_some() {
                let dir = self.paths.get("freezer").unwrap();
                write_file(dir, "freezer.state", state)?;
            }
        } else {
            if !state.is_empty() {
                // invalid state
                return Err(nix::Error::Sys(Errno::EINVAL).into());
            }
        }
        Ok(())
    }

    fn destroy(&mut self) -> Result<()> {
        for (_, d) in &self.paths {
            remove(d)?;
        }

        self.paths = HashMap::new();

        Ok(())
    }

    fn get_pids(&self) -> Result<Vec<pid_t>> {
        let m = if self.paths.get("devices").is_some() {
            get_procs(self.paths.get("devices").unwrap())?
        } else {
            return Err(ErrorKind::ErrorCode("no devices cgroup".to_string()).into());
        };

        Ok(m)
    }

    fn get_all_pids(&self) -> Result<Vec<pid_t>> {
        let m = if self.paths.get("devices").is_some() {
            get_all_procs(self.paths.get("devices").unwrap())?
        } else {
            return Err(ErrorKind::ErrorCode("no devices cgroup".to_string()).into());
        };

        Ok(m)
    }
}

impl Manager {
    pub fn new(cpath: &str) -> Result<Self> {
        let mut m = HashMap::new();

        if !cpath.starts_with('/') {
            return Err(nix::Error::Sys(Errno::EINVAL).into());
        }

        let paths = get_paths()?;
        let mounts = get_mounts()?;

        for (key, value) in &paths {
            let mnt = mounts.get(key);

            if mnt.is_none() {
                continue;
            }

            let p = if value == "/" {
                format!("{}{}", mnt.unwrap(), cpath)
            } else {
                format!("{}{}{}", mnt.unwrap(), value, cpath)
            };

            m.insert(key.to_string(), p);
        }

        Ok(Self {
            paths: m,
            mounts,
            rels: paths,
            cpath: cpath.to_string(),
        })
    }

    pub fn update_cpuset_path(&self, cpuset: &str) -> Result<()> {
        let root = if self.mounts.get("cpuset").is_some() {
            self.mounts.get("cpuset").unwrap()
        } else {
            return Err(nix::Error::Sys(Errno::ENOENT).into());
        };

        let relss = if self.rels.get("cpuset").is_some() {
            self.rels.get("cpuset").unwrap()
        } else {
            return Err(nix::Error::Sys(Errno::ENOENT).into());
        };

        let mut dir: String = root.to_string();
        let rels: Vec<&str> = relss.split('/').collect();
        let cpaths: Vec<&str> = self.cpath.as_str().split('/').collect();

        for d in rels.iter() {
            if d.is_empty() {
                continue;
            }

            dir.push('/');
            dir.push_str(d);
            write_file(dir.as_str(), CPUSET_CPUS, cpuset)?;
        }

        for d in cpaths.iter() {
            if d.is_empty() {
                continue;
            }

            dir.push('/');
            dir.push_str(d);
            write_file(dir.as_str(), CPUSET_CPUS, cpuset)?;
        }

        Ok(())
    }
}

pub fn get_guest_cpuset() -> Result<String> {
    let m = get_mounts()?;

    if m.get("cpuset").is_none() {
        warn!(sl!(), "no cpuset cgroup!");
        return Err(nix::Error::Sys(Errno::ENOENT).into());
    }

    get_param_string(m.get("cpuset").unwrap(), CPUSET_CPUS)
}
