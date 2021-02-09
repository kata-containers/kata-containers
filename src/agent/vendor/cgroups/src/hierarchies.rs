// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module represents the various control group hierarchies the Linux kernel supports.
//!
//! Currently, we only support the cgroupv1 hierarchy, but in the future we will add support for
//! the Unified Hierarchy.
use nix::sys::statfs;

use std::fs::{self, File};
use std::io::BufRead;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use log::*;

use crate::blkio::BlkIoController;
use crate::cpu::CpuController;
use crate::cpuacct::CpuAcctController;
use crate::cpuset::CpuSetController;
use crate::devices::DevicesController;
use crate::freezer::FreezerController;
use crate::hugetlb::HugeTlbController;
use crate::memory::MemController;
use crate::net_cls::NetClsController;
use crate::net_prio::NetPrioController;
use crate::perf_event::PerfEventController;
use crate::pid::PidController;
use crate::rdma::RdmaController;
use crate::systemd::SystemdController;
use crate::{Controllers, Hierarchy, Subsystem};

use crate::cgroup::Cgroup;

/// The standard, original cgroup implementation. Often referred to as "cgroupv1".
pub struct V1 {
    mount_point: String,
}

pub struct V2 {
    root: String,
}

impl Hierarchy for V1 {
    fn v2(&self) -> bool {
        false
    }

    fn subsystems(&self) -> Vec<Subsystem> {
        let mut subs = vec![];
        if self.check_support(Controllers::Pids) {
            subs.push(Subsystem::Pid(PidController::new(self.root(), false)));
        }
        if self.check_support(Controllers::Mem) {
            subs.push(Subsystem::Mem(MemController::new(self.root(), false)));
        }
        if self.check_support(Controllers::CpuSet) {
            subs.push(Subsystem::CpuSet(CpuSetController::new(self.root(), false)));
        }
        if self.check_support(Controllers::CpuAcct) {
            subs.push(Subsystem::CpuAcct(CpuAcctController::new(self.root())));
        }
        if self.check_support(Controllers::Cpu) {
            subs.push(Subsystem::Cpu(CpuController::new(self.root(), false)));
        }
        if self.check_support(Controllers::Devices) {
            subs.push(Subsystem::Devices(DevicesController::new(self.root())));
        }
        if self.check_support(Controllers::Freezer) {
            subs.push(Subsystem::Freezer(FreezerController::new(
                self.root(),
                false,
            )));
        }
        if self.check_support(Controllers::NetCls) {
            subs.push(Subsystem::NetCls(NetClsController::new(self.root())));
        }
        if self.check_support(Controllers::BlkIo) {
            subs.push(Subsystem::BlkIo(BlkIoController::new(self.root(), false)));
        }
        if self.check_support(Controllers::PerfEvent) {
            subs.push(Subsystem::PerfEvent(PerfEventController::new(self.root())));
        }
        if self.check_support(Controllers::NetPrio) {
            subs.push(Subsystem::NetPrio(NetPrioController::new(self.root())));
        }
        if self.check_support(Controllers::HugeTlb) {
            subs.push(Subsystem::HugeTlb(HugeTlbController::new(
                self.root(),
                false,
            )));
        }
        if self.check_support(Controllers::Rdma) {
            subs.push(Subsystem::Rdma(RdmaController::new(self.root())));
        }
        if self.check_support(Controllers::Systemd) {
            subs.push(Subsystem::Systemd(SystemdController::new(
                self.root(),
                false,
            )));
        }

        subs
    }

    fn root_control_group(&self) -> Cgroup {
        let b: &Hierarchy = self as &Hierarchy;
        Cgroup::load(Box::new(&*b), "".to_string())
    }

    fn check_support(&self, sub: Controllers) -> bool {
        let root = self.root().read_dir().unwrap();
        for entry in root {
            if let Ok(entry) = entry {
                if entry.file_name().into_string().unwrap() == sub.to_string() {
                    return true;
                }
            }
        }
        return false;
    }

    fn root(&self) -> PathBuf {
        PathBuf::from(self.mount_point.clone())
    }
}

impl Hierarchy for V2 {
    fn v2(&self) -> bool {
        true
    }

    fn subsystems(&self) -> Vec<Subsystem> {
        let p = format!("{}/{}", UNIFIED_MOUNTPOINT, "cgroup.controllers");
        let ret = fs::read_to_string(p.as_str());
        if ret.is_err() {
            return vec![];
        }

        let mut subs = vec![];

        let controllers = ret.unwrap().trim().to_string();
        let controller_list: Vec<&str> = controllers.split(' ').collect();

        for s in controller_list {
            match s {
                "cpu" => {
                    subs.push(Subsystem::Cpu(CpuController::new(self.root(), true)));
                }
                "io" => {
                    subs.push(Subsystem::BlkIo(BlkIoController::new(self.root(), true)));
                }
                "cpuset" => {
                    subs.push(Subsystem::CpuSet(CpuSetController::new(self.root(), true)));
                }
                "memory" => {
                    subs.push(Subsystem::Mem(MemController::new(self.root(), true)));
                }
                "pids" => {
                    subs.push(Subsystem::Pid(PidController::new(self.root(), true)));
                }
                "freezer" => {
                    subs.push(Subsystem::Freezer(FreezerController::new(
                        self.root(),
                        true,
                    )));
                }
                "hugetlb" => {
                    subs.push(Subsystem::HugeTlb(HugeTlbController::new(
                        self.root(),
                        true,
                    )));
                }
                _ => {}
            }
        }

        subs
    }

    fn root_control_group(&self) -> Cgroup {
        let b: &Hierarchy = self as &Hierarchy;
        Cgroup::load(Box::new(&*b), "".to_string())
    }

    fn check_support(&self, _sub: Controllers) -> bool {
        return false;
    }

    fn root(&self) -> PathBuf {
        PathBuf::from(self.root.clone())
    }
}

impl V1 {
    /// Finds where control groups are mounted to and returns a hierarchy in which control groups
    /// can be created.
    pub fn new() -> V1 {
        let mount_point = find_v1_mount().unwrap();
        V1 {
            mount_point: mount_point,
        }
    }
}

impl V2 {
    /// Finds where control groups are mounted to and returns a hierarchy in which control groups
    /// can be created.
    pub fn new() -> V2 {
        V2 {
            root: String::from(UNIFIED_MOUNTPOINT),
        }
    }
}

pub const UNIFIED_MOUNTPOINT: &'static str = "/sys/fs/cgroup";

#[cfg(all(target_os = "linux", not(target_env = "musl")))]
pub fn is_cgroup2_unified_mode() -> bool {
    let path = Path::new(UNIFIED_MOUNTPOINT);
    let fs_stat = statfs::statfs(path);
    if fs_stat.is_err() {
        return false;
    }

    // FIXME notwork, nix will not compile CGROUP2_SUPER_MAGIC because not(target_env = "musl")
    fs_stat.unwrap().filesystem_type() == statfs::CGROUP2_SUPER_MAGIC
}

pub const INIT_CGROUP_PATHS: &'static str = "/proc/1/cgroup";

#[cfg(all(target_os = "linux", target_env = "musl"))]
pub fn is_cgroup2_unified_mode() -> bool {
    let lines = fs::read_to_string(INIT_CGROUP_PATHS);
    if lines.is_err() {
        return false;
    }

    for line in lines.unwrap().lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() != 3 {
            continue;
        }
        if fields[0] != "0" {
            return false;
        }
    }

    true
}

pub fn auto() -> Box<dyn Hierarchy> {
    if is_cgroup2_unified_mode() {
        Box::new(V2::new())
    } else {
        Box::new(V1::new())
    }
}

fn find_v1_mount() -> Option<String> {
    // Open mountinfo so we can get a parseable mount list
    let mountinfo_path = Path::new("/proc/self/mountinfo");

    // If /proc isn't mounted, or something else happens, then bail out
    if mountinfo_path.exists() == false {
        return None;
    }

    let mountinfo_file = File::open(mountinfo_path).unwrap();
    let mountinfo_reader = BufReader::new(&mountinfo_file);
    for _line in mountinfo_reader.lines() {
        let line = _line.unwrap();
        let mut fields = line.split_whitespace();
        let index = line.find(" - ").unwrap();
        let more_fields = line[index + 3..].split_whitespace().collect::<Vec<_>>();
        if more_fields.len() == 0 {
            continue;
        }
        if more_fields[0] == "cgroup" {
            if more_fields.len() < 3 {
                continue;
            }
            let cgroups_mount = fields.nth(4).unwrap();
            if let Some(parent) = std::path::Path::new(cgroups_mount).parent() {
                if let Some(path) = parent.as_os_str().to_str() {
                    debug!("found cgroups {:?} from {:?}", path, cgroups_mount);
                    return Some(path.to_string());
                }
            }
            continue;
        }
    }

    None
}
