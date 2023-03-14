// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module represents the various control group hierarchies the Linux kernel supports.
//!
//! Currently, we only support the cgroupv1 hierarchy, but in the future we will add support for
//! the Unified Hierarchy.

use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

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

/// Process mounts information.
///
/// See `proc(5)` for format details.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Mountinfo {
    /// Mount pathname relative to the process's root.
    pub mount_point: PathBuf,
    /// Filesystem type (main type with optional sub-type).
    pub fs_type: (String, Option<String>),
    /// Superblock options.
    pub super_opts: Vec<String>,
}

pub(crate) fn parse_mountinfo_for_line(line: &str) -> Option<Mountinfo> {
    let s_values: Vec<_> = line.split(" - ").collect();
    if s_values.len() != 2 {
        return None;
    }

    let s0_values: Vec<_> = s_values[0].trim().split(' ').collect();
    let s1_values: Vec<_> = s_values[1].trim().split(' ').collect();
    if s0_values.len() < 6 || s1_values.len() < 3 {
        return None;
    }
    let mount_point = PathBuf::from(s0_values[4]);
    let fs_type_values: Vec<_> = s1_values[0].trim().split('.').collect();
    let fs_type = match fs_type_values.len() {
        1 => (fs_type_values[0].to_string(), None),
        2 => (
            fs_type_values[0].to_string(),
            Some(fs_type_values[1].to_string()),
        ),
        _ => return None,
    };

    let super_opts: Vec<String> = s1_values[2].trim().split(',').map(String::from).collect();
    Some(Mountinfo {
        mount_point,
        fs_type,
        super_opts,
    })
}

/// Parses the provided mountinfo file.
fn mountinfo_file(file: &mut File) -> Vec<Mountinfo> {
    let mut r = Vec::new();
    for line in BufReader::new(file).lines() {
        match line {
            Ok(line) => {
                if let Some(mi) = parse_mountinfo_for_line(&line) {
                    if mi.fs_type.0 == "cgroup" {
                        r.push(mi);
                    }
                }
            }
            Err(_) => break,
        }
    }
    r
}

/// Returns mounts information for the current process.
pub fn mountinfo_self() -> Vec<Mountinfo> {
    match File::open("/proc/self/mountinfo") {
        Ok(mut file) => mountinfo_file(&mut file),
        Err(_) => vec![],
    }
}

/// The standard, original cgroup implementation. Often referred to as "cgroupv1".
#[derive(Debug, Clone)]
pub struct V1 {
    mountinfo: Vec<Mountinfo>,
}

#[derive(Debug, Clone)]
pub struct V2 {
    root: String,
}

impl Hierarchy for V1 {
    fn v2(&self) -> bool {
        false
    }

    fn subsystems(&self) -> Vec<Subsystem> {
        let mut subs = vec![];

        // The cgroup writeback feature requires cooperation between memcgs and blkcgs
        // To avoid exceptions, we should add_task for blkcg before memcg(push BlkIo before Mem)
        // For more Information: https://www.alibabacloud.com/help/doc-detail/155509.htm
        if let Some(root) = self.get_mount_point(Controllers::BlkIo) {
            subs.push(Subsystem::BlkIo(BlkIoController::new(root, false)));
        }
        if let Some(root) = self.get_mount_point(Controllers::Mem) {
            subs.push(Subsystem::Mem(MemController::new(root, false)));
        }
        if let Some(root) = self.get_mount_point(Controllers::Pids) {
            subs.push(Subsystem::Pid(PidController::new(root, false)));
        }
        if let Some(root) = self.get_mount_point(Controllers::CpuSet) {
            subs.push(Subsystem::CpuSet(CpuSetController::new(root, false)));
        }
        if let Some(root) = self.get_mount_point(Controllers::CpuAcct) {
            subs.push(Subsystem::CpuAcct(CpuAcctController::new(root)));
        }
        if let Some(root) = self.get_mount_point(Controllers::Cpu) {
            subs.push(Subsystem::Cpu(CpuController::new(root, false)));
        }
        if let Some(root) = self.get_mount_point(Controllers::Devices) {
            subs.push(Subsystem::Devices(DevicesController::new(root)));
        }
        if let Some(root) = self.get_mount_point(Controllers::Freezer) {
            subs.push(Subsystem::Freezer(FreezerController::new(root, false)));
        }
        if let Some(root) = self.get_mount_point(Controllers::NetCls) {
            subs.push(Subsystem::NetCls(NetClsController::new(root)));
        }
        if let Some(root) = self.get_mount_point(Controllers::PerfEvent) {
            subs.push(Subsystem::PerfEvent(PerfEventController::new(root)));
        }
        if let Some(root) = self.get_mount_point(Controllers::NetPrio) {
            subs.push(Subsystem::NetPrio(NetPrioController::new(root)));
        }
        if let Some(root) = self.get_mount_point(Controllers::HugeTlb) {
            subs.push(Subsystem::HugeTlb(HugeTlbController::new(root, false)));
        }
        if let Some(root) = self.get_mount_point(Controllers::Rdma) {
            subs.push(Subsystem::Rdma(RdmaController::new(root)));
        }
        if let Some(root) = self.get_mount_point(Controllers::Systemd) {
            subs.push(Subsystem::Systemd(SystemdController::new(root, false)));
        }

        subs
    }

    fn root_control_group(&self) -> Cgroup {
        Cgroup::load(auto(), "")
    }

    fn parent_control_group(&self, path: &str) -> Cgroup {
        let path = Path::new(path);
        let parent_path = path.parent().unwrap().to_string_lossy().to_string();
        Cgroup::load(auto(), parent_path)
    }

    fn root(&self) -> PathBuf {
        self.mountinfo
            .iter()
            .find_map(|m| {
                if m.fs_type.0 == "cgroup" {
                    return Some(m.mount_point.parent().unwrap());
                }
                None
            })
            .unwrap()
            .to_path_buf()
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
        let mut controller_list: Vec<&str> = controllers.split(' ').collect();

        // The freezer functionality is present in V2, but not as a controller,
        // but apparently as a core functionality. FreezerController supports
        // that, but we must explicitly fake the controller here.
        controller_list.push("freezer");

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
        Cgroup::load(auto(), "")
    }

    fn parent_control_group(&self, path: &str) -> Cgroup {
        let path = Path::new(path);
        let parent_path = path.parent().unwrap().to_string_lossy().to_string();
        Cgroup::load(auto(), parent_path)
    }

    fn root(&self) -> PathBuf {
        PathBuf::from(self.root.clone())
    }
}

impl V1 {
    /// Finds where control groups are mounted to and returns a hierarchy in which control groups
    /// can be created.
    pub fn new() -> V1 {
        V1 {
            mountinfo: mountinfo_self(),
        }
    }

    pub fn get_mount_point(&self, controller: Controllers) -> Option<PathBuf> {
        self.mountinfo.iter().find_map(|m| {
            if m.fs_type.0 == "cgroup" && m.super_opts.contains(&controller.to_string()) {
                return Some(m.mount_point.clone());
            }
            None
        })
    }
}

impl Default for V1 {
    fn default() -> Self {
        Self::new()
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

impl Default for V2 {
    fn default() -> Self {
        Self::new()
    }
}

pub const UNIFIED_MOUNTPOINT: &str = "/sys/fs/cgroup";

#[cfg(any(
    all(target_os = "linux", not(target_env = "musl")),
    target_os = "android"
))]
pub fn is_cgroup2_unified_mode() -> bool {
    use nix::sys::statfs;

    let path = std::path::Path::new(UNIFIED_MOUNTPOINT);
    let fs_stat = statfs::statfs(path);
    if fs_stat.is_err() {
        return false;
    }

    // FIXME notwork, nix will not compile CGROUP2_SUPER_MAGIC because not(target_env = "musl")
    fs_stat.unwrap().filesystem_type() == statfs::CGROUP2_SUPER_MAGIC
}

pub const INIT_CGROUP_PATHS: &str = "/proc/1/cgroup";

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mount() {
        let mountinfo = vec![
            ("29 26 0:26 / /sys/fs/cgroup/cpuset,cpu,cpuacct rw,nosuid,nodev,noexec,relatime shared:10 - cgroup cgroup rw,cpuset,cpu,cpuacct",
             Mountinfo{mount_point: PathBuf::from("/sys/fs/cgroup/cpuset,cpu,cpuacct"), fs_type: ("cgroup".to_string(), None), super_opts: vec![
                "rw".to_string(),
                "cpuset".to_string(),
                "cpu".to_string(),
                "cpuacct".to_string(),
             ]}),
            ("121 1731 0:42 / /shm rw,nosuid,nodev,noexec,relatime shared:68 master:66 - tmpfs shm rw,size=65536k",
             Mountinfo{mount_point: PathBuf::from("/shm"), fs_type: ("tmpfs".to_string(), None), super_opts: vec![
                "rw".to_string(),
                "size=65536k".to_string(),
             ]}),
            ("121 1731 0:42 / /shm rw,nosuid,nodev,noexec,relatime shared:68 master:66 - tmpfs.123 shm rw,size=65536k",
             Mountinfo{mount_point: PathBuf::from("/shm"), fs_type: ("tmpfs".to_string(), Some("123".to_string())), super_opts: vec![
                "rw".to_string(),
                "size=65536k".to_string(),
             ]}),
        ];

        for mi in mountinfo {
            let info = parse_mountinfo_for_line(mi.0).unwrap();
            assert_eq!(info, mi.1)
        }
    }
}
