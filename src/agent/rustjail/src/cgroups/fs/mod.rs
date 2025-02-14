// Copyright (c) 2019, 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use cgroups::blkio::{BlkIoController, BlkIoData, IoService};
use cgroups::cpu::CpuController;
use cgroups::cpuacct::CpuAcctController;
use cgroups::cpuset::CpuSetController;
use cgroups::devices::DevicePermissions;
use cgroups::devices::DeviceType;
use cgroups::freezer::{FreezerController, FreezerState};
use cgroups::hugetlb::HugeTlbController;
use cgroups::memory::MemController;
use cgroups::pid::PidController;
use cgroups::{
    BlkIoDeviceResource, BlkIoDeviceThrottleResource, Cgroup, CgroupPid, Controller,
    DeviceResource, HugePageResource, MaxValue, NetworkPriority,
};

use crate::cgroups::{rule_for_all_devices, Manager as CgroupManager};
use crate::container::DEFAULT_DEVICES;
use anyhow::{anyhow, Context, Result};
use libc::{self, pid_t};
use oci::{
    LinuxBlockIo, LinuxCpu, LinuxDevice, LinuxDeviceCgroup, LinuxDeviceCgroupBuilder,
    LinuxHugepageLimit, LinuxMemory, LinuxNetwork, LinuxPids, LinuxResources, Spec,
};
use oci_spec::runtime as oci;

use protobuf::MessageField;
use protocols::agent::{
    BlkioStats, BlkioStatsEntry, CgroupStats, CpuStats, CpuUsage, HugetlbStats, MemoryData,
    MemoryStats, PidsStats, ThrottlingData,
};
use std::any::Any;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use super::DevicesCgroupInfo;

const GUEST_CPUS_PATH: &str = "/sys/devices/system/cpu/online";

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "cgroups"))
}

macro_rules! get_controller_or_return_singular_none {
    ($cg:ident) => {
        match $cg.controller_of() {
            Some(c) => c,
            None => return MessageField::none(),
        }
    };
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Manager {
    pub paths: HashMap<String, String>,
    pub mounts: HashMap<String, String>,
    pub cpath: String,
    #[serde(skip)]
    cgroup: cgroups::Cgroup,
    #[serde(skip)]
    pod_cgroup: Option<cgroups::Cgroup>,
    #[serde(skip)]
    devcg_allowed_all: bool,
}

// set_resource is used to set reources by cgroup controller.
macro_rules! set_resource {
    ($cont:ident, $func:ident, $res:ident, $field:ident) => {
        let resource_value = $res.$field().unwrap_or(0);
        if resource_value != 0 {
            $cont.$func(resource_value)?;
        }
    };
}

impl CgroupManager for Manager {
    fn apply(&self, pid: pid_t) -> Result<()> {
        self.cgroup.add_task_by_tgid(CgroupPid::from(pid as u64))?;
        Ok(())
    }

    fn set(&self, r: &LinuxResources, update: bool) -> Result<()> {
        info!(
            sl(),
            "cgroup manager set resources for container. Resources input {:?}", r
        );

        let res = &mut cgroups::Resources::default();
        let pod_res = &mut cgroups::Resources::default();

        // set cpuset and cpu reources
        if let Some(cpu) = &r.cpu() {
            set_cpu_resources(&self.cgroup, cpu)?;
        }

        // set memory resources
        if let Some(memory) = &r.memory() {
            set_memory_resources(&self.cgroup, memory, update)?;
        }

        // set pids resources
        if let Some(pids_resources) = &r.pids() {
            set_pids_resources(&self.cgroup, pids_resources)?;
        }

        // set block_io resources
        if let Some(blkio) = &r.block_io() {
            set_block_io_resources(&self.cgroup, blkio, res);
        }

        // set hugepages resources
        if let Some(hugepage_limits) = r.hugepage_limits() {
            set_hugepages_resources(&self.cgroup, hugepage_limits, res);
        }

        // set network resources
        if let Some(network) = &r.network() {
            set_network_resources(&self.cgroup, network, res);
        }

        // set devices resources
        if !self.devcg_allowed_all {
            if let Some(devices) = r.devices() {
                set_devices_resources(&self.cgroup, devices, res, pod_res);
            }
        }
        debug!(
            sl(),
            "Resources after processed, pod_res = {:?}, res = {:?}", pod_res, res
        );

        // apply resources
        if let Some(pod_cg) = self.pod_cgroup.as_ref() {
            pod_cg.apply(pod_res)?;
        }
        self.cgroup.apply(res)?;

        Ok(())
    }

    fn get_stats(&self) -> Result<CgroupStats> {
        // CpuStats
        let cpu_usage = get_cpuacct_stats(&self.cgroup);

        let throttling_data = get_cpu_stats(&self.cgroup);

        let cpu_stats = MessageField::some(CpuStats {
            cpu_usage,
            throttling_data,
            ..Default::default()
        });

        // Memorystats
        let memory_stats = get_memory_stats(&self.cgroup);

        // PidsStats
        let pids_stats = get_pids_stats(&self.cgroup);

        // BlkioStats
        // note that virtiofs has no blkio stats
        let blkio_stats = get_blkio_stats(&self.cgroup);

        // HugetlbStats
        let hugetlb_stats = get_hugetlb_stats(&self.cgroup);

        Ok(CgroupStats {
            cpu_stats,
            memory_stats,
            pids_stats,
            blkio_stats,
            hugetlb_stats,
            ..Default::default()
        })
    }

    fn freeze(&self, state: FreezerState) -> Result<()> {
        let freezer_controller: &FreezerController = self.cgroup.controller_of().unwrap();
        match state {
            FreezerState::Thawed => {
                freezer_controller.thaw()?;
            }
            FreezerState::Frozen => {
                freezer_controller.freeze()?;
            }
            _ => {
                return Err(anyhow!("Invalid FreezerState"));
            }
        }

        Ok(())
    }

    fn destroy(&mut self) -> Result<()> {
        if let Err(err) = self.cgroup.delete() {
            warn!(
                sl(),
                "Failed to delete cgroup {}: {}",
                self.cgroup.path(),
                err
            );
        }
        Ok(())
    }

    fn get_pids(&self) -> Result<Vec<pid_t>> {
        let mem_controller: &MemController = self.cgroup.controller_of().unwrap();
        let pids = mem_controller.tasks();
        let result = pids.iter().map(|x| x.pid as i32).collect::<Vec<i32>>();

        Ok(result)
    }

    fn update_cpuset_path(&self, guest_cpuset: &str, container_cpuset: &str) -> Result<()> {
        if guest_cpuset.is_empty() {
            return Ok(());
        }
        info!(sl(), "update_cpuset_path to: {}", guest_cpuset);

        let h = cgroups::hierarchies::auto();
        let root_cg = h.root_control_group();

        let root_cpuset_controller: &CpuSetController = root_cg.controller_of().unwrap();
        let path = root_cpuset_controller.path();
        let root_path = Path::new(path);
        info!(sl(), "root cpuset path: {:?}", &path);

        let container_cpuset_controller: &CpuSetController = self.cgroup.controller_of().unwrap();
        let path = container_cpuset_controller.path();
        let container_path = Path::new(path);
        info!(sl(), "container cpuset path: {:?}", &path);

        let mut paths = vec![];
        for ancestor in container_path.ancestors() {
            if ancestor == root_path {
                break;
            }
            paths.push(ancestor);
        }
        info!(sl(), "parent paths to update cpuset: {:?}", &paths);

        let mut i = paths.len();
        loop {
            if i == 0 {
                break;
            }
            i -= 1;

            // remove cgroup root from path
            let r_path = &paths[i]
                .to_str()
                .unwrap()
                .trim_start_matches(root_path.to_str().unwrap());
            info!(sl(), "updating cpuset for parent path {:?}", &r_path);
            let cg = new_cgroup(cgroups::hierarchies::auto(), r_path)?;
            let cpuset_controller: &CpuSetController = cg.controller_of().unwrap();
            cpuset_controller.set_cpus(guest_cpuset)?;
        }

        if !container_cpuset.is_empty() {
            info!(
                sl(),
                "updating cpuset for container path: {:?} cpuset: {}",
                &container_path,
                container_cpuset
            );
            container_cpuset_controller.set_cpus(container_cpuset)?;
        }

        Ok(())
    }

    fn get_cgroup_path(&self, cg: &str) -> Result<String> {
        if cgroups::hierarchies::is_cgroup2_unified_mode() {
            let cg_path = format!("/sys/fs/cgroup/{}", self.cpath);
            return Ok(cg_path);
        }

        // for cgroup v1
        Ok(self.paths.get(cg).map(|s| s.to_string()).unwrap())
    }

    fn as_any(&self) -> Result<&dyn Any> {
        Ok(self)
    }

    fn name(&self) -> &str {
        "cgroupfs"
    }
}

fn set_network_resources(
    _cg: &cgroups::Cgroup,
    network: &LinuxNetwork,
    res: &mut cgroups::Resources,
) {
    info!(sl(), "cgroup manager set network");

    // set classid
    // description can be found at https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v1/net_cls.html
    let class_id = network.class_id().unwrap_or(0) as u64;
    if class_id != 0 {
        res.network.class_id = Some(class_id);
    }

    // set network priorities
    // description can be found at https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v1/net_prio.html
    let mut priorities = vec![];
    let interface_priority = network.priorities().clone().unwrap_or_default();
    for p in interface_priority.iter() {
        priorities.push(NetworkPriority {
            name: p.name().clone(),
            priority: p.priority() as u64,
        });
    }

    res.network.priorities = priorities;
}

fn set_devices_resources(
    _cg: &cgroups::Cgroup,
    device_resources: &[LinuxDeviceCgroup],
    res: &mut cgroups::Resources,
    pod_res: &mut cgroups::Resources,
) {
    info!(sl(), "cgroup manager set devices");
    let mut devices = vec![];

    for d in device_resources.iter() {
        if rule_for_all_devices(d) {
            continue;
        }
        if let Some(dev) = linux_device_cgroup_to_device_resource(d) {
            devices.push(dev);
        }
    }

    pod_res.devices.devices = devices.clone();
    res.devices.devices = devices;
}

fn set_hugepages_resources(
    cg: &cgroups::Cgroup,
    hugepage_limits: &[LinuxHugepageLimit],
    res: &mut cgroups::Resources,
) {
    info!(sl(), "cgroup manager set hugepage");
    let mut limits = vec![];
    let hugetlb_controller = cg.controller_of::<HugeTlbController>();

    for l in hugepage_limits.iter() {
        if hugetlb_controller.is_some() && hugetlb_controller.unwrap().size_supported(l.page_size())
        {
            let hr = HugePageResource {
                size: l.page_size().clone(),
                limit: l.limit() as u64,
            };
            limits.push(hr);
        } else {
            warn!(
                sl(),
                "{} page size support cannot be verified, dropping requested limit",
                l.page_size()
            );
        }
    }
    res.hugepages.limits = limits;
}

fn set_block_io_resources(
    _cg: &cgroups::Cgroup,
    blkio: &LinuxBlockIo,
    res: &mut cgroups::Resources,
) {
    info!(sl(), "cgroup manager set block io");

    res.blkio.weight = blkio.weight();
    res.blkio.leaf_weight = blkio.leaf_weight();

    let mut blk_device_resources = vec![];
    let default_weight_device = vec![];
    let weight_device = blkio
        .weight_device()
        .as_ref()
        .unwrap_or(&default_weight_device);
    for d in weight_device.iter() {
        let dr = BlkIoDeviceResource {
            major: d.major() as u64,
            minor: d.minor() as u64,
            weight: blkio.weight(),
            leaf_weight: blkio.leaf_weight(),
        };
        blk_device_resources.push(dr);
    }
    res.blkio.weight_device = blk_device_resources;

    res.blkio.throttle_read_bps_device = build_blk_io_device_throttle_resource(
        blkio.throttle_read_bps_device().as_ref().unwrap_or(&vec![]),
    );
    res.blkio.throttle_write_bps_device = build_blk_io_device_throttle_resource(
        blkio
            .throttle_write_bps_device()
            .as_ref()
            .unwrap_or(&vec![]),
    );
    res.blkio.throttle_read_iops_device = build_blk_io_device_throttle_resource(
        blkio
            .throttle_read_iops_device()
            .as_ref()
            .unwrap_or(&vec![]),
    );
    res.blkio.throttle_write_iops_device = build_blk_io_device_throttle_resource(
        blkio
            .throttle_write_iops_device()
            .as_ref()
            .unwrap_or(&vec![]),
    );
}

fn set_cpu_resources(cg: &cgroups::Cgroup, cpu: &LinuxCpu) -> Result<()> {
    info!(sl(), "cgroup manager set cpu");

    let cpuset_controller: &CpuSetController = cg.controller_of().unwrap();

    if let Some(cpus) = cpu.cpus() {
        if let Err(e) = cpuset_controller.set_cpus(cpus) {
            warn!(sl(), "write cpuset failed: {:?}", e);
        }
    }

    if let Some(mems) = cpu.mems() {
        cpuset_controller.set_mems(mems)?;
    }

    let cpu_controller: &CpuController = cg.controller_of().unwrap();

    if let Some(shares) = cpu.shares() {
        let shares = if cg.v2() {
            convert_shares_to_v2_value(shares)
        } else {
            shares
        };
        if shares != 0 {
            cpu_controller.set_shares(shares)?;
        }
    }

    set_resource!(cpu_controller, set_cfs_quota, cpu, quota);
    set_resource!(cpu_controller, set_cfs_period, cpu, period);

    set_resource!(cpu_controller, set_rt_runtime, cpu, realtime_runtime);
    set_resource!(cpu_controller, set_rt_period_us, cpu, realtime_period);

    Ok(())
}

fn set_memory_resources(cg: &cgroups::Cgroup, memory: &LinuxMemory, update: bool) -> Result<()> {
    info!(sl(), "cgroup manager set memory");
    let mem_controller: &MemController = cg.controller_of().unwrap();

    if !update {
        // initialize kmem limits for accounting
        mem_controller.set_kmem_limit(1)?;
        mem_controller.set_kmem_limit(-1)?;
    }

    // If the memory update is set to -1 we should also
    // set swap to -1, it means unlimited memory.
    let mut swap = memory.swap().unwrap_or(0);
    if memory.limit() == Some(-1) {
        swap = -1;
    }

    if memory.limit().is_some() && swap != 0 {
        let memstat = get_memory_stats(cg)
            .into_option()
            .ok_or_else(|| anyhow!("failed to get the cgroup memory stats"))?;
        let memusage = memstat.usage();

        // When update memory limit, the kernel would check the current memory limit
        // set against the new swap setting, if the current memory limit is large than
        // the new swap, then set limit first, otherwise the kernel would complain and
        // refused to set; on the other hand, if the current memory limit is smaller than
        // the new swap, then we should set the swap first and then set the memor limit.
        if swap == -1 || memusage.limit() < swap as u64 {
            mem_controller.set_memswap_limit(swap)?;
            set_resource!(mem_controller, set_limit, memory, limit);
        } else {
            set_resource!(mem_controller, set_limit, memory, limit);
            mem_controller.set_memswap_limit(swap)?;
        }
    } else {
        set_resource!(mem_controller, set_limit, memory, limit);
        swap = if cg.v2() {
            convert_memory_swap_to_v2_value(swap, memory.limit().unwrap_or(0))?
        } else {
            swap
        };
        if swap != 0 {
            mem_controller.set_memswap_limit(swap)?;
        }
    }

    set_resource!(mem_controller, set_soft_limit, memory, reservation);
    set_resource!(mem_controller, set_kmem_limit, memory, kernel);
    set_resource!(mem_controller, set_tcp_limit, memory, kernel_tcp);

    if let Some(swappiness) = memory.swappiness() {
        if (0..=100).contains(&swappiness) {
            mem_controller.set_swappiness(swappiness)?;
        } else {
            return Err(anyhow!(
                "invalid value:{}. valid memory swappiness range is 0-100",
                swappiness
            ));
        }
    }

    if memory.disable_oom_killer().unwrap_or(false) {
        mem_controller.disable_oom_killer()?;
    }

    Ok(())
}

fn set_pids_resources(cg: &cgroups::Cgroup, pids: &LinuxPids) -> Result<()> {
    info!(sl(), "cgroup manager set pids");
    let pid_controller: &PidController = cg.controller_of().unwrap();
    let v = if pids.limit() > 0 {
        MaxValue::Value(pids.limit())
    } else {
        MaxValue::Max
    };
    pid_controller
        .set_pid_max(v)
        .context("failed to set pids resources")
}

fn build_blk_io_device_throttle_resource(
    input: &[oci::LinuxThrottleDevice],
) -> Vec<BlkIoDeviceThrottleResource> {
    let mut blk_io_device_throttle_resources = vec![];
    for d in input.iter() {
        let tr = BlkIoDeviceThrottleResource {
            major: d.major() as u64,
            minor: d.minor() as u64,
            rate: d.rate(),
        };
        blk_io_device_throttle_resources.push(tr);
    }

    blk_io_device_throttle_resources
}

fn linux_device_cgroup_to_device_resource(d: &LinuxDeviceCgroup) -> Option<DeviceResource> {
    let dev_type = match DeviceType::from_char(d.typ().unwrap_or_default().as_str().chars().next())
    {
        Some(t) => t,
        None => return None,
    };

    let mut permissions: Vec<DevicePermissions> = vec![];
    for p in d
        .access()
        .as_ref()
        .unwrap_or(&"".to_owned())
        .chars()
        .collect::<Vec<char>>()
    {
        match p {
            'r' => permissions.push(DevicePermissions::Read),
            'w' => permissions.push(DevicePermissions::Write),
            'm' => permissions.push(DevicePermissions::MkNod),
            _ => {}
        }
    }

    Some(DeviceResource {
        allow: d.allow(),
        devtype: dev_type,
        major: d.major().unwrap_or(0),
        minor: d.minor().unwrap_or(0),
        access: permissions,
    })
}

// split space separated values into an vector of u64
fn line_to_vec(line: &str) -> Vec<u64> {
    line.split_whitespace()
        .filter_map(|x| x.parse::<u64>().ok())
        .collect::<Vec<u64>>()
}

// split flat keyed values into an hashmap of <String, u64>
fn lines_to_map(content: &str) -> HashMap<String, u64> {
    content
        .lines()
        .map(|x| x.split_whitespace().collect::<Vec<&str>>())
        .filter(|x| x.len() == 2 && x[1].parse::<u64>().is_ok())
        .fold(HashMap::new(), |mut hm, x| {
            hm.insert(x[0].to_string(), x[1].parse::<u64>().unwrap());
            hm
        })
}

pub const NANO_PER_SECOND: u64 = 1000000000;
pub const WILDCARD: i64 = -1;

lazy_static! {
    pub static ref CLOCK_TICKS: f64 = {
        let n = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };

        n as f64
    };

    pub static ref DEFAULT_ALLOWED_DEVICES: Vec<LinuxDeviceCgroup> = {
        vec![
            // all mknod to all char devices
            LinuxDeviceCgroupBuilder::default()
                .allow(true)
                .typ(oci::LinuxDeviceType::C)
                .major(WILDCARD)
                .minor(WILDCARD)
                .access("m")
                .build()
                .unwrap(),

            // all mknod to all block devices
            LinuxDeviceCgroupBuilder::default()
                .allow(true)
                .typ(oci::LinuxDeviceType::B)
                .major(WILDCARD)
                .minor(WILDCARD)
                .access("m")
                .build()
                .unwrap(),

            // all read/write/mknod to char device /dev/console
            LinuxDeviceCgroupBuilder::default()
                .allow(true)
                .typ(oci::LinuxDeviceType::C)
                .major(5)
                .minor(1)
                .access("rwm")
                .build()
                .unwrap(),

            // all read/write/mknod to char device /dev/pts/<N>
            LinuxDeviceCgroupBuilder::default()
                .allow(true)
                .typ(oci::LinuxDeviceType::C)
                .major(136)
                .minor(WILDCARD)
                .access("rwm")
                .build()
                .unwrap(),

            // all read/write/mknod to char device /dev/ptmx
            LinuxDeviceCgroupBuilder::default()
                .allow(true)
                .typ(oci::LinuxDeviceType::C)
                .major(5)
                .minor(2)
                .access("rwm")
                .build()
                .unwrap(),

            // all read/write/mknod to char device /dev/net/tun
            LinuxDeviceCgroupBuilder::default()
                .allow(true)
                .typ(oci::LinuxDeviceType::C)
                .major(10)
                .minor(200)
                .access("rwm")
                .build()
                .unwrap(),
        ]
    };
}

fn get_cpu_stats(cg: &cgroups::Cgroup) -> MessageField<ThrottlingData> {
    let cpu_controller: &CpuController = get_controller_or_return_singular_none!(cg);
    let stat = cpu_controller.cpu().stat;
    let h = lines_to_map(&stat);

    MessageField::some(ThrottlingData {
        periods: *h.get("nr_periods").unwrap_or(&0),
        throttled_periods: *h.get("nr_throttled").unwrap_or(&0),
        throttled_time: *h.get("throttled_time").unwrap_or(&0),
        ..Default::default()
    })
}

fn get_cpuacct_stats(cg: &cgroups::Cgroup) -> MessageField<CpuUsage> {
    if let Some(cpuacct_controller) = cg.controller_of::<CpuAcctController>() {
        let cpuacct = cpuacct_controller.cpuacct();

        let h = lines_to_map(&cpuacct.stat);
        let usage_in_usermode =
            (((*h.get("user").unwrap_or(&0) * NANO_PER_SECOND) as f64) / *CLOCK_TICKS) as u64;
        let usage_in_kernelmode =
            (((*h.get("system").unwrap_or(&0) * NANO_PER_SECOND) as f64) / *CLOCK_TICKS) as u64;

        let total_usage = cpuacct.usage;

        let percpu_usage = line_to_vec(&cpuacct.usage_percpu);

        return MessageField::some(CpuUsage {
            total_usage,
            percpu_usage,
            usage_in_kernelmode,
            usage_in_usermode,
            ..Default::default()
        });
    }

    // try to get from cpu controller
    let cpu_controller: &CpuController = get_controller_or_return_singular_none!(cg);
    let stat = cpu_controller.cpu().stat;
    let h = lines_to_map(&stat);
    // All fields in CpuUsage are expressed in nanoseconds (ns).
    //
    // For cgroup v1 (cpuacct controller):
    // kata-agent reads the cpuacct.stat file, which reports the number of ticks
    // consumed by the processes in the cgroup. It then converts these ticks to nanoseconds.
    // Ref: https://www.kernel.org/doc/Documentation/cgroup-v1/cpuacct.txt
    //
    // For cgroup v2 (cpu controller):
    // kata-agent reads the cpu.stat file, which reports the time consumed by the
    // processes in the cgroup in microseconds (us). It then converts microseconds to nanoseconds.
    // Ref: https://www.kernel.org/doc/Documentation/cgroup-v2.txt, section 5-1-1. CPU Interface Files
    let usage_in_usermode = *h.get("user_usec").unwrap_or(&0) * 1000;
    let usage_in_kernelmode = *h.get("system_usec").unwrap_or(&0) * 1000;
    let total_usage = *h.get("usage_usec").unwrap_or(&0) * 1000;
    let percpu_usage = vec![];

    MessageField::some(CpuUsage {
        total_usage,
        percpu_usage,
        usage_in_kernelmode,
        usage_in_usermode,
        ..Default::default()
    })
}

fn get_memory_stats(cg: &cgroups::Cgroup) -> MessageField<MemoryStats> {
    let memory_controller: &MemController = get_controller_or_return_singular_none!(cg);

    // cache from memory stat
    let memory = memory_controller.memory_stat();
    let cache = memory.stat.cache;

    // use_hierarchy
    let value = memory.use_hierarchy;
    let use_hierarchy = value == 1;

    // get memory data
    let usage = MessageField::some(MemoryData {
        usage: memory.usage_in_bytes,
        max_usage: memory.max_usage_in_bytes,
        failcnt: memory.fail_cnt,
        limit: memory.limit_in_bytes as u64,
        ..Default::default()
    });

    // get swap usage
    let memswap = memory_controller.memswap();

    let swap_usage = MessageField::some(MemoryData {
        usage: memswap.usage_in_bytes,
        max_usage: memswap.max_usage_in_bytes,
        failcnt: memswap.fail_cnt,
        limit: memswap.limit_in_bytes as u64,
        ..Default::default()
    });

    // get kernel usage
    let kmem_stat = memory_controller.kmem_stat();

    let kernel_usage = MessageField::some(MemoryData {
        usage: kmem_stat.usage_in_bytes,
        max_usage: kmem_stat.max_usage_in_bytes,
        failcnt: kmem_stat.fail_cnt,
        limit: kmem_stat.limit_in_bytes as u64,
        ..Default::default()
    });

    MessageField::some(MemoryStats {
        cache,
        usage,
        swap_usage,
        kernel_usage,
        use_hierarchy,
        stats: memory.stat.raw,
        ..Default::default()
    })
}

fn get_pids_stats(cg: &cgroups::Cgroup) -> MessageField<PidsStats> {
    let pid_controller: &PidController = get_controller_or_return_singular_none!(cg);

    let current = pid_controller.get_pid_current().unwrap_or(0);
    let max = pid_controller.get_pid_max();

    let limit = match max {
        Err(_) => 0,
        Ok(max) => match max {
            MaxValue::Value(v) => v,
            MaxValue::Max => 0,
        },
    } as u64;

    MessageField::some(PidsStats {
        current,
        limit,
        ..Default::default()
    })
}

/*
examples(from runc, cgroup v1):
https://github.com/opencontainers/runc/blob/a5847db387ae28c0ca4ebe4beee1a76900c86414/libcontainer/cgroups/fs/blkio.go

    blkio.sectors
    8:0 6792

    blkio.io_service_bytes
    8:0 Read 1282048
    8:0 Write 2195456
    8:0 Sync 2195456
    8:0 Async 1282048
    8:0 Total 3477504
    Total 3477504

    blkio.io_serviced
    8:0 Read 124
    8:0 Write 104
    8:0 Sync 104
    8:0 Async 124
    8:0 Total 228
    Total 228

    blkio.io_queued
    8:0 Read 0
    8:0 Write 0
    8:0 Sync 0
    8:0 Async 0
    8:0 Total 0
    Total 0
*/

fn get_blkio_stat_blkiodata(blkiodata: &[BlkIoData]) -> Vec<BlkioStatsEntry> {
    let mut m = Vec::new();
    if blkiodata.is_empty() {
        return m;
    }

    // blkio.time_recursive and blkio.sectors_recursive have no op field.
    let op = "".to_string();
    for d in blkiodata {
        m.push(BlkioStatsEntry {
            major: d.major as u64,
            minor: d.minor as u64,
            op: op.clone(),
            value: d.data,
            ..Default::default()
        });
    }

    m
}

fn get_blkio_stat_ioservice(services: &[IoService]) -> Vec<BlkioStatsEntry> {
    let mut m = Vec::new();

    if services.is_empty() {
        return m;
    }

    for s in services {
        m.push(build_blkio_stats_entry(s.major, s.minor, "read", s.read));
        m.push(build_blkio_stats_entry(s.major, s.minor, "write", s.write));
        m.push(build_blkio_stats_entry(s.major, s.minor, "sync", s.sync));
        m.push(build_blkio_stats_entry(
            s.major, s.minor, "async", s.r#async,
        ));
        m.push(build_blkio_stats_entry(s.major, s.minor, "total", s.total));
    }
    m
}

fn build_blkio_stats_entry(major: i16, minor: i16, op: &str, value: u64) -> BlkioStatsEntry {
    BlkioStatsEntry {
        major: major as u64,
        minor: minor as u64,
        op: op.to_string(),
        value,
        ..Default::default()
    }
}

fn get_blkio_stats_v2(cg: &cgroups::Cgroup) -> MessageField<BlkioStats> {
    let blkio_controller: &BlkIoController = get_controller_or_return_singular_none!(cg);
    let blkio = blkio_controller.blkio();

    let mut resp = BlkioStats::new();
    let mut blkio_stats = Vec::new();

    let stat = blkio.io_stat;
    for s in stat {
        blkio_stats.push(build_blkio_stats_entry(s.major, s.minor, "read", s.rbytes));
        blkio_stats.push(build_blkio_stats_entry(s.major, s.minor, "write", s.wbytes));
        blkio_stats.push(build_blkio_stats_entry(s.major, s.minor, "rios", s.rios));
        blkio_stats.push(build_blkio_stats_entry(s.major, s.minor, "wios", s.wios));
        blkio_stats.push(build_blkio_stats_entry(
            s.major, s.minor, "dbytes", s.dbytes,
        ));
        blkio_stats.push(build_blkio_stats_entry(s.major, s.minor, "dios", s.dios));
    }

    resp.io_service_bytes_recursive = blkio_stats;

    MessageField::some(resp)
}

fn get_blkio_stats(cg: &cgroups::Cgroup) -> MessageField<BlkioStats> {
    if cg.v2() {
        return get_blkio_stats_v2(cg);
    }

    let blkio_controller: &BlkIoController = get_controller_or_return_singular_none!(cg);
    let blkio = blkio_controller.blkio();

    let mut m = BlkioStats::new();
    let io_serviced_recursive = blkio.io_serviced_recursive;

    if io_serviced_recursive.is_empty() {
        // fall back to generic stats
        // blkio.throttle.io_service_bytes,
        // maybe io_service_bytes_recursive?
        // stick to runc for now
        m.io_service_bytes_recursive = get_blkio_stat_ioservice(&blkio.throttle.io_service_bytes);
        m.io_serviced_recursive = get_blkio_stat_ioservice(&blkio.throttle.io_serviced);
    } else {
        // Try to read CFQ stats available on all CFQ enabled kernels first
        // IoService type data
        m.io_service_bytes_recursive = get_blkio_stat_ioservice(&blkio.io_service_bytes_recursive);
        m.io_serviced_recursive = get_blkio_stat_ioservice(&io_serviced_recursive);
        m.io_queued_recursive = get_blkio_stat_ioservice(&blkio.io_queued_recursive);
        m.io_service_time_recursive = get_blkio_stat_ioservice(&blkio.io_service_time_recursive);
        m.io_wait_time_recursive = get_blkio_stat_ioservice(&blkio.io_wait_time_recursive);
        m.io_merged_recursive = get_blkio_stat_ioservice(&blkio.io_merged_recursive);

        // BlkIoData type data
        m.io_time_recursive = get_blkio_stat_blkiodata(&blkio.time_recursive);
        m.sectors_recursive = get_blkio_stat_blkiodata(&blkio.sectors_recursive);
    }

    MessageField::some(m)
}

fn get_hugetlb_stats(cg: &cgroups::Cgroup) -> HashMap<String, HugetlbStats> {
    let mut h = HashMap::new();

    let hugetlb_controller: Option<&HugeTlbController> = cg.controller_of();
    if hugetlb_controller.is_none() {
        return h;
    }
    let hugetlb_controller = hugetlb_controller.unwrap();

    let sizes = hugetlb_controller.get_sizes();
    for size in sizes {
        let usage = hugetlb_controller.usage_in_bytes(&size).unwrap_or(0);
        let max_usage = hugetlb_controller.max_usage_in_bytes(&size).unwrap_or(0);
        let failcnt = hugetlb_controller.failcnt(&size).unwrap_or(0);

        h.insert(
            size.to_string(),
            HugetlbStats {
                usage,
                max_usage,
                failcnt,
                ..Default::default()
            },
        );
    }

    h
}

pub const PATHS: &str = "/proc/self/cgroup";
pub const MOUNTS: &str = "/proc/self/mountinfo";

pub fn get_paths() -> Result<HashMap<String, String>> {
    let mut m = HashMap::new();
    for l in fs::read_to_string(PATHS)?.lines() {
        let fl: Vec<&str> = l.split(':').collect();
        if fl.len() != 3 {
            info!(sl(), "Corrupted cgroup data!");
            continue;
        }

        let keys: Vec<&str> = fl[1].split(',').collect();
        for key in &keys {
            m.insert(key.to_string(), fl[2].to_string());
        }
    }
    Ok(m)
}

pub fn get_mounts(paths: &HashMap<String, String>) -> Result<HashMap<String, String>> {
    let mut m = HashMap::new();

    for l in fs::read_to_string(MOUNTS)?.lines() {
        let p: Vec<&str> = l.splitn(2, " - ").collect();
        let pre: Vec<&str> = p[0].split(' ').collect();
        let post: Vec<&str> = p[1].split(' ').collect();

        if post.len() != 3 {
            warn!(sl(), "can't parse {} line {:?}", MOUNTS, l);
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

#[inline]
fn new_cgroup(h: Box<dyn cgroups::Hierarchy>, path: &str) -> Result<Cgroup> {
    let valid_path = path.trim_start_matches('/').to_string();
    cgroups::Cgroup::new(h, valid_path.as_str()).map_err(anyhow::Error::from)
}

#[inline]
fn load_cgroup(h: Box<dyn cgroups::Hierarchy>, path: &str) -> Cgroup {
    let valid_path = path.trim_start_matches('/').to_string();
    cgroups::Cgroup::load(h, valid_path.as_str())
}

impl Manager {
    pub fn new(
        cpath: &str,
        spec: &Spec,
        devcg_info: Option<Arc<RwLock<DevicesCgroupInfo>>>,
    ) -> Result<Self> {
        let (paths, mounts) = Self::get_paths_and_mounts(cpath).context("Get paths and mounts")?;

        // Do not expect poisoning lock
        let mut devices_group_info = devcg_info.as_ref().map(|i| i.write().unwrap());
        let pod_cgroup: Option<Cgroup>;

        if let Some(devices_group_info) = devices_group_info.as_mut() {
            // Cgroup path of parent of container
            let pod_cpath = PathBuf::from(cpath)
                .parent()
                .unwrap_or(Path::new("/"))
                .display()
                .to_string();

            if pod_cpath.as_str() == "/" {
                // Skip setting pod cgroup for cpath due to no parent path
                pod_cgroup = None
            } else {
                // Create a cgroup for the pod if not exists.
                // Note that creating pod cgroup MUST be done before the pause
                // container's cgroup created, since the upper node might have
                // some excessive permissions, and children inherit upper
                // node's rules. You'll feel painful to shrink upper nodes'
                // permissions if the new permissions are subset of old.
                pod_cgroup = Some(load_cgroup(cgroups::hierarchies::auto(), &pod_cpath));
                let pod_cg = pod_cgroup.as_ref().unwrap();

                let is_allowded_all = Self::has_allowed_all_devices_rule(spec);
                if devices_group_info.inited {
                    debug!(sl(), "Devices cgroup has been initialzied.");

                    // Set allowed all devices to pod cgroup
                    if !devices_group_info.allowed_all && is_allowded_all {
                        info!(
                            sl(),
                            "Pod devices cgroup is changed to allowed all devices mode, devices_group_info = {:?}",
                            devices_group_info
                        );
                        Self::setup_allowed_all_mode(pod_cg).with_context(|| {
                            format!("Setup allowed all devices mode for {}", pod_cpath)
                        })?;
                        devices_group_info.allowed_all = true;
                    }
                } else {
                    // This is the first container (aka pause container)
                    debug!(sl(), "Started to init devices cgroup");

                    pod_cg.create().context("Create pod cgroup")?;

                    if !is_allowded_all {
                        Self::setup_devcg_whitelist(pod_cg).with_context(|| {
                            format!("Setup device cgroup whitelist for {}", pod_cpath)
                        })?;
                    } else {
                        Self::setup_allowed_all_mode(pod_cg)
                            .with_context(|| format!("Setup allowed all mode for {}", pod_cpath))?;
                        devices_group_info.allowed_all = true;
                    }

                    devices_group_info.inited = true
                }
            }
        } else {
            pod_cgroup = None;
        }

        // Create a cgroup for the container.
        let cg = new_cgroup(cgroups::hierarchies::auto(), cpath)?;
        // The rules of container cgroup are copied from its parent, which
        // contains some permissions that the container doesn't need.
        // Therefore, resetting the container's devices cgroup is required.
        if let Some(devices_group_info) = devices_group_info.as_ref() {
            if !devices_group_info.allowed_all {
                Self::setup_devcg_whitelist(&cg)
                    .with_context(|| format!("Setup device cgroup whitelist for {}", cpath))?;
            }
        }

        Ok(Self {
            paths,
            mounts,
            // rels: paths,
            cpath: cpath.to_string(),
            cgroup: cg,
            pod_cgroup,
            devcg_allowed_all: devices_group_info
                .map(|info| info.allowed_all)
                .unwrap_or(false),
        })
    }

    /// Create a cgroupfs manager for systemd cgroup.
    /// The device cgroup is disabled in systemd cgroup, given that it is
    /// implemented by eBPF.
    pub fn new_systemd(cpath: &str) -> Result<Self> {
        let (paths, mounts) = Self::get_paths_and_mounts(cpath).context("Get paths and mounts")?;

        let cg = new_cgroup(cgroups::hierarchies::auto(), cpath)?;

        Ok(Self {
            paths,
            mounts,
            cpath: cpath.to_string(),
            pod_cgroup: None,
            cgroup: cg,
            devcg_allowed_all: false,
        })
    }

    pub fn subcgroup(&self) -> &str {
        // Check if we're in a Docker-in-Docker setup by verifying:
        // 1. We're using cgroups v2 (which restricts direct process control)
        // 2. An "init" subdirectory exists (used by DinD for process delegation)
        let is_dind = cgroups::hierarchies::is_cgroup2_unified_mode()
            && cgroups::hierarchies::auto()
                .root()
                .join(&self.cpath)
                .join("init")
                .exists();
        if is_dind {
            "/init/"
        } else {
            "/"
        }
    }

    fn get_paths_and_mounts(
        cpath: &str,
    ) -> Result<(HashMap<String, String>, HashMap<String, String>)> {
        let mut m = HashMap::new();

        let paths = get_paths()?;
        let mounts = get_mounts(&paths)?;

        for key in paths.keys() {
            let mnt = mounts.get(key);

            if mnt.is_none() {
                continue;
            }

            m.insert(key.to_string(), format!("{}/{}", mnt.unwrap(), cpath));
        }

        Ok((m, mounts))
    }

    fn setup_allowed_all_mode(cgroup: &cgroups::Cgroup) -> Result<()> {
        // Insert two rules: `b *:* rwm` and `c *:* rwm`.
        // The reason of not inserting `a *:* rwm` is that the Linux kernel
        // will deny writing `a` to `devices.allow` once a cgroup has
        // children. You can refer to
        // https://www.kernel.org/doc/Documentation/cgroup-v1/devices.txt.
        let res = cgroups::Resources {
            devices: cgroups::DeviceResources {
                devices: vec![
                    DeviceResource {
                        allow: true,
                        devtype: DeviceType::Block,
                        major: -1,
                        minor: -1,
                        access: vec![
                            DevicePermissions::Read,
                            DevicePermissions::Write,
                            DevicePermissions::MkNod,
                        ],
                    },
                    DeviceResource {
                        allow: true,
                        devtype: DeviceType::Char,
                        major: -1,
                        minor: -1,
                        access: vec![
                            DevicePermissions::Read,
                            DevicePermissions::Write,
                            DevicePermissions::MkNod,
                        ],
                    },
                ],
            },
            ..Default::default()
        };
        cgroup.apply(&res)?;

        Ok(())
    }

    /// Setup device cgroup whitelist:
    /// - Deny all devices in order to cleanup device cgroup.
    /// - Allow default devices and default allowed devices.
    fn setup_devcg_whitelist(cgroup: &cgroups::Cgroup) -> Result<()> {
        #[allow(unused_mut)]
        let mut dev_res_list = vec![DeviceResource {
            allow: false,
            devtype: DeviceType::All,
            major: -1,
            minor: -1,
            access: vec![
                DevicePermissions::Read,
                DevicePermissions::Write,
                DevicePermissions::MkNod,
            ],
        }];
        // Do not append default allowed devices for simplicity while
        // testing.
        #[cfg(not(test))]
        dev_res_list.append(&mut default_allowed_devices());

        let res = cgroups::Resources {
            devices: cgroups::DeviceResources {
                devices: dev_res_list,
            },
            ..Default::default()
        };
        cgroup.apply(&res)?;

        Ok(())
    }

    /// Check if OCI spec contains a rule of allowed all devices.
    fn has_allowed_all_devices_rule(spec: &Spec) -> bool {
        let linux = match spec.linux().as_ref() {
            Some(linux) => linux,
            None => return false,
        };
        let resources = match linux.resources().as_ref() {
            Some(resource) => resource,
            None => return false,
        };

        resources
            .devices()
            .as_ref()
            .and_then(|devices| {
                devices
                    .iter()
                    .find(|dev| rule_for_all_devices(dev))
                    .map(|dev| dev.allow())
            })
            .unwrap_or_default()
    }
}

/// Generate a list for allowed devices including `DEFAULT_DEVICES` and
/// `DEFAULT_ALLOWED_DEVICES`.
fn default_allowed_devices() -> Vec<DeviceResource> {
    let mut dev_res_list = Vec::new();
    DEFAULT_DEVICES.iter().for_each(|dev| {
        if let Some(dev_res) = linux_device_to_device_resource(dev) {
            dev_res_list.push(dev_res)
        }
    });
    DEFAULT_ALLOWED_DEVICES.iter().for_each(|dev| {
        if let Some(dev_res) = linux_device_cgroup_to_device_resource(dev) {
            dev_res_list.push(dev_res)
        }
    });
    dev_res_list
}

/// Convert LinuxDevice to DeviceResource.
fn linux_device_to_device_resource(d: &LinuxDevice) -> Option<DeviceResource> {
    let dev_type = match DeviceType::from_char(d.typ().as_str().chars().next()) {
        Some(t) => t,
        None => return None,
    };

    let permissions = vec![
        DevicePermissions::Read,
        DevicePermissions::Write,
        DevicePermissions::MkNod,
    ];

    Some(DeviceResource {
        allow: true,
        devtype: dev_type,
        major: d.major(),
        minor: d.minor(),
        access: permissions,
    })
}

// get the guest's online cpus.
pub fn get_guest_cpuset() -> Result<String> {
    let c = fs::read_to_string(GUEST_CPUS_PATH)?;
    Ok(c.trim().to_string())
}

// Since the OCI spec is designed for cgroup v1, in some cases
// there is need to convert from the cgroup v1 configuration to cgroup v2
// the formula for cpuShares is y = (1 + ((x - 2) * 9999) / 262142)
// convert from [2-262144] to [1-10000]
// 262144 comes from Linux kernel definition "#define MAX_SHARES (1UL << 18)"
// from https://github.com/opencontainers/runc/blob/a5847db387ae28c0ca4ebe4beee1a76900c86414/libcontainer/cgroups/utils.go#L394
pub fn convert_shares_to_v2_value(shares: u64) -> u64 {
    if shares == 0 {
        return 0;
    }
    1 + ((shares - 2) * 9999) / 262142
}

// ConvertMemorySwapToCgroupV2Value converts MemorySwap value from OCI spec
// for use by cgroup v2 drivers. A conversion is needed since Resources.MemorySwap
// is defined as memory+swap combined, while in cgroup v2 swap is a separate value.
fn convert_memory_swap_to_v2_value(memory_swap: i64, memory: i64) -> Result<i64> {
    // for compatibility with cgroup1 controller, set swap to unlimited in
    // case the memory is set to unlimited, and swap is not explicitly set,
    // treating the request as "set both memory and swap to unlimited".
    if memory == -1 && memory_swap == 0 {
        return Ok(-1);
    }
    if memory_swap == -1 || memory_swap == 0 {
        // -1 is "max", 0 is "unset", so treat as is
        return Ok(memory_swap);
    }
    // sanity checks
    if memory == 0 || memory == -1 {
        return Err(anyhow!("unable to set swap limit without memory limit"));
    }
    if memory < 0 {
        return Err(anyhow!("invalid memory value: {}", memory));
    }
    if memory_swap < memory {
        return Err(anyhow!("memory+swap limit should be >= memory limit"));
    }
    Ok(memory_swap - memory)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::process::Command;
    use std::sync::{Arc, RwLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    use cgroups::devices::{DevicePermissions, DeviceType};
    use oci::{
        LinuxBuilder, LinuxDeviceCgroup, LinuxDeviceCgroupBuilder, LinuxDeviceType,
        LinuxResourcesBuilder, SpecBuilder,
    };
    use oci_spec::runtime as oci;
    use test_utils::skip_if_not_root;

    use super::default_allowed_devices;
    use crate::cgroups::fs::{
        line_to_vec, lines_to_map, Manager, DEFAULT_ALLOWED_DEVICES, WILDCARD,
    };
    use crate::cgroups::DevicesCgroupInfo;
    use crate::container::DEFAULT_DEVICES;

    #[test]
    fn test_line_to_vec() {
        let test_cases = vec![
            ("1 2 3", vec![1, 2, 3]),
            ("a 1 b 2 3 c", vec![1, 2, 3]),
            ("a b c", vec![]),
        ];

        for test_case in test_cases {
            let result = line_to_vec(test_case.0);
            assert_eq!(
                result, test_case.1,
                "except: {:?} for input {}",
                test_case.1, test_case.0
            );
        }
    }

    #[test]
    fn test_lines_to_map() {
        let hm1: HashMap<String, u64> = [
            ("a".to_string(), 1),
            ("b".to_string(), 2),
            ("c".to_string(), 3),
            ("e".to_string(), 5),
        ]
        .iter()
        .cloned()
        .collect();
        let hm2: HashMap<String, u64> = [("a".to_string(), 1)].iter().cloned().collect();

        let test_cases = vec![
            ("a 1\nb 2\nc 3\nd X\ne 5\n", hm1),
            ("a 1", hm2),
            ("a c", HashMap::new()),
        ];

        for test_case in test_cases {
            let result = lines_to_map(test_case.0);
            assert_eq!(
                result, test_case.1,
                "except: {:?} for input {}",
                test_case.1, test_case.0
            );
        }
    }

    struct MockSandbox {
        devcg_info: Arc<RwLock<DevicesCgroupInfo>>,
    }

    impl MockSandbox {
        fn new() -> Self {
            Self {
                devcg_info: Arc::new(RwLock::new(DevicesCgroupInfo::default())),
            }
        }
    }

    #[test]
    fn test_new_fs_manager() {
        skip_if_not_root!();

        let output = Command::new("stat")
            .arg("-f")
            .arg("-c")
            .arg("%T")
            .arg("/sys/fs/cgroup/")
            .output()
            .unwrap();
        let output_str = String::from_utf8(output.stdout).unwrap();
        let cgroup_version = output_str.strip_suffix("\n").unwrap();
        if cgroup_version.eq("cgroup2fs") {
            println!("INFO: Skipping the test as cgroups v2 is used by default");
            return;
        }

        struct TestCase {
            cpath: Vec<String>,
            devices: Vec<Vec<LinuxDeviceCgroup>>,
            allowed_all: Vec<bool>,
            pod_devices_list: Vec<String>,
            container_devices_list: Vec<String>,
        }

        let allow_all = LinuxDeviceCgroupBuilder::default()
            .allow(true)
            .typ(LinuxDeviceType::A)
            .major(0)
            .minor(0)
            .access("rwm")
            .build()
            .unwrap();
        let deny_all = LinuxDeviceCgroupBuilder::default()
            .allow(false)
            .typ(LinuxDeviceType::A)
            .major(0)
            .minor(0)
            .access("rwm")
            .build()
            .unwrap();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let one_time_pod_name = format!("kata-agent-fs-manager-test-{}", now);
        let one_time_cpath =
            |child: &str| -> String { format!("/{}/{}", one_time_pod_name, child) };

        let test_cases = vec![
            TestCase {
                cpath: vec![one_time_cpath("child1")],
                devices: vec![vec![allow_all.clone()]],
                allowed_all: vec![true],
                pod_devices_list: vec![String::from("a *:* rwm\n")],
                container_devices_list: vec![String::from("a *:* rwm\n")],
            },
            TestCase {
                cpath: vec![one_time_cpath("child1")],
                devices: vec![vec![deny_all.clone()]],
                allowed_all: vec![false],
                pod_devices_list: vec![String::new()],
                container_devices_list: vec![String::new()],
            },
            TestCase {
                cpath: vec![one_time_cpath("child1"), one_time_cpath("child2")],
                devices: vec![vec![deny_all.clone()], vec![allow_all.clone()]],
                allowed_all: vec![false, true],
                pod_devices_list: vec![String::new(), String::from("b *:* rwm\nc *:* rwm\n")],
                container_devices_list: vec![String::new(), String::from("b *:* rwm\nc *:* rwm\n")],
            },
            TestCase {
                cpath: vec![one_time_cpath("child1"), one_time_cpath("child2")],
                devices: vec![vec![allow_all], vec![deny_all]],
                allowed_all: vec![true, true],
                pod_devices_list: vec![String::from("a *:* rwm\n"), String::from("a *:* rwm\n")],
                container_devices_list: vec![
                    String::from("a *:* rwm\n"),
                    String::from("a *:* rwm\n"),
                ],
            },
        ];

        for (round, tc) in test_cases.iter().enumerate() {
            let sandbox = MockSandbox::new();
            let devcg_info = sandbox.devcg_info.read().unwrap();
            assert!(!devcg_info.inited);
            assert!(!devcg_info.allowed_all);
            drop(devcg_info);
            let mut managers = Vec::with_capacity(tc.devices.len());

            for cid in 0..tc.devices.len() {
                let spec = SpecBuilder::default()
                    .linux(
                        LinuxBuilder::default()
                            .resources(
                                LinuxResourcesBuilder::default()
                                    .devices(tc.devices[cid].clone())
                                    .build()
                                    .unwrap(),
                            )
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap();
                managers.push(
                    Manager::new(&tc.cpath[cid], &spec, Some(sandbox.devcg_info.clone())).unwrap(),
                );

                let devcg_info = sandbox.devcg_info.read().unwrap();
                assert!(devcg_info.inited);
                assert_eq!(
                    devcg_info.allowed_all, tc.allowed_all[cid],
                    "Test case {}: cid {} allowed all assertion failure",
                    round, cid
                );
                drop(devcg_info);

                let pod_devices_list = Command::new("cat")
                    .arg(&format!(
                        "/sys/fs/cgroup/devices/{}/devices.list",
                        one_time_pod_name
                    ))
                    .output()
                    .unwrap();
                let container_devices_list = Command::new("cat")
                    .arg(&format!(
                        "/sys/fs/cgroup/devices{}/devices.list",
                        tc.cpath[cid]
                    ))
                    .output()
                    .unwrap();

                let pod_devices_list = String::from_utf8(pod_devices_list.stdout).unwrap();
                let container_devices_list =
                    String::from_utf8(container_devices_list.stdout).unwrap();

                assert_eq!(
                    &pod_devices_list, &tc.pod_devices_list[cid],
                    "Test case {}: cid {} allowed all assertion failure",
                    round, cid
                );
                assert_eq!(
                    &container_devices_list, &tc.container_devices_list[cid],
                    "Test case {}: cid {} allowed all assertion failure",
                    round, cid
                )
            }

            // Clean up cgroups
            managers
                .iter()
                .for_each(|manager| manager.cgroup.delete().unwrap());
            // The pod_cgroup must not be None
            managers[0].pod_cgroup.as_ref().unwrap().delete().unwrap();
        }
    }

    #[test]
    fn test_default_allowed_devices() {
        let allowed_devices = default_allowed_devices();
        assert_eq!(
            allowed_devices.len(),
            DEFAULT_DEVICES.len() + DEFAULT_ALLOWED_DEVICES.len()
        );

        let allowed_permissions = vec![
            DevicePermissions::Read,
            DevicePermissions::Write,
            DevicePermissions::MkNod,
        ];

        let default_devices_0 = &allowed_devices[0];
        assert!(default_devices_0.allow);
        assert_eq!(default_devices_0.devtype, DeviceType::Char);
        assert_eq!(default_devices_0.major, 1);
        assert_eq!(default_devices_0.minor, 3);
        assert!(default_devices_0
            .access
            .iter()
            .all(|&p| allowed_permissions.iter().any(|&ap| ap == p)));

        let default_allowed_devices_0 = &allowed_devices[DEFAULT_DEVICES.len()];
        assert!(default_allowed_devices_0.allow);
        assert_eq!(default_allowed_devices_0.devtype, DeviceType::Char);
        assert_eq!(default_allowed_devices_0.major, WILDCARD);
        assert_eq!(default_allowed_devices_0.minor, WILDCARD);
        assert_eq!(
            default_allowed_devices_0.access,
            vec![DevicePermissions::MkNod]
        );
    }
}
