// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::{HashMap, HashSet};
use std::process;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use cgroups_rs::manager::is_systemd_cgroup;
use cgroups_rs::{CgroupPid, FsManager, Manager, SystemdManager};
use hypervisor::{Hypervisor, VcpuThreadIds};
use kata_types::cpu::CpuSet;
use nix::sched::{sched_setaffinity, CpuSet as NixCpuSet};
use nix::unistd::Pid;
use oci_spec::runtime::{LinuxCpu, LinuxCpuBuilder, LinuxResources, LinuxResourcesBuilder};

use crate::cgroups::utils::get_tgid_from_pid;
use crate::cgroups::CgroupConfig;
use crate::ResourceUpdateOp;

pub type CgroupManager = Box<dyn Manager>;

pub(crate) struct CgroupsResourceInner {
    /// Container resources, key is container id, and value is resources.
    resources: HashMap<String, LinuxResources>,
    sandbox_cgroup: CgroupManager,
    overhead_cgroup: Option<CgroupManager>,
    /// User-facing config knob: whether vCPU pinning is allowed at all.
    /// Comes from TOML `enable_vcpus_pinning` or the per-pod annotation.
    enable_vcpus_pinning: bool,
    /// Runtime state: whether pinning is currently active. Pinning is only
    /// turned on when `enable_vcpus_pinning` is true *and* the vCPU count
    /// matches the sandbox cpuset size. Tracked so we know when to reset
    /// threads back to the full cpuset after a mismatch.
    is_vcpus_pinning_on: bool,
}

impl CgroupsResourceInner {
    /// Create cgroup managers according to the cgroup configuration.
    ///
    /// # Returns
    ///
    /// - `Ok((CgroupManager, Option<CgroupManager>))`: A tuple containing
    ///   the sandbox cgroup manager and an optional overhead cgroup
    ///   manager.
    fn new_cgroup_managers(
        config: &CgroupConfig,
    ) -> Result<(CgroupManager, Option<CgroupManager>)> {
        let use_systemd = is_systemd_cgroup(&config.path);
        let sandbox_cgroup = if use_systemd {
            let mut manager = SystemdManager::new(&config.path).context("new systemd manager")?;
            // Set SIGTERM timeout to 5mins, so that the runtime has up to
            // 5mins to do graceful shutdown. Exceeding this timeout, the
            // systemd will forcibly kill the runtime by sending SIGKILL.
            manager.set_term_timeout(300).context("set term timeout")?;
            Box::new(manager) as Box<dyn Manager>
        } else {
            let manager = FsManager::new(&config.path).context("new fs manager")?;
            Box::new(manager) as Box<dyn Manager>
        };

        let overhead_cgroup = if config.sandbox_cgroup_only {
            None
        } else if use_systemd {
            let mut manager = SystemdManager::new(&config.overhead_path)
                .context("new systemd manager for overhead")?;
            manager
                .set_term_timeout(300)
                .context("set term timeout for overhead")?;
            Some(Box::new(manager) as Box<dyn Manager>)
        } else {
            let manager =
                FsManager::new(&config.overhead_path).context("new fs manager for overhead")?;
            Some(Box::new(manager) as Box<dyn Manager>)
        };

        Ok((sandbox_cgroup, overhead_cgroup))
    }

    /// Create a new `CgroupsResourceInner` instance.
    pub(crate) fn new(config: &CgroupConfig) -> Result<Self> {
        let (mut sandbox_cgroup, mut overhead_cgroup) =
            Self::new_cgroup_managers(config).context("create new cgroups")?;

        // The runtime is prioritized to be added to the overhead cgroup.
        let pid = CgroupPid::from(process::id() as u64);
        if let Some(overhead_cgroup) = overhead_cgroup.as_mut() {
            overhead_cgroup
                .add_proc(pid)
                .context("add runtime to overhead cgroup")?;
        } else {
            sandbox_cgroup
                .add_proc(pid)
                .context("add runtime to sandbox cgroup")?;
        }

        Ok(Self {
            resources: HashMap::new(),
            sandbox_cgroup,
            overhead_cgroup,
            enable_vcpus_pinning: config.enable_vcpus_pinning,
            is_vcpus_pinning_on: false,
        })
    }

    pub(crate) fn restore(config: &CgroupConfig) -> Result<Self> {
        let (sandbox_cgroup, overhead_cgroup) =
            Self::new_cgroup_managers(config).context("restore cgroups")?;

        Ok(Self {
            resources: HashMap::new(),
            sandbox_cgroup,
            overhead_cgroup,
            enable_vcpus_pinning: config.enable_vcpus_pinning,
            is_vcpus_pinning_on: false,
        })
    }
}

impl CgroupsResourceInner {
    /// Add cpuset resources of all containers to the sandbox cgroup.
    fn collect_resources(&self) -> Result<LinuxResources> {
        let mut cpu_cpus = HashSet::new();
        let mut cpu_mems = HashSet::new();

        for res in self.resources.values() {
            if let Some(cpu) = res.cpu() {
                if let Some(cpus) = cpu.cpus() {
                    cpu_cpus.insert(cpus.to_string());
                }
                if let Some(mems) = cpu.mems() {
                    cpu_mems.insert(mems.to_string());
                }
            }
        }

        let mut resources_builder = LinuxResourcesBuilder::default();

        let mut cpu_builder = LinuxCpuBuilder::default();
        if !cpu_cpus.is_empty() {
            cpu_builder = cpu_builder.cpus(cpu_cpus.into_iter().collect::<Vec<_>>().join(","));
        }
        if !cpu_mems.is_empty() {
            cpu_builder = cpu_builder.mems(cpu_mems.into_iter().collect::<Vec<_>>().join(","));
        }
        let cpu = cpu_builder.build().context("build linux cpu")?;
        if cpu != LinuxCpu::default() {
            resources_builder = resources_builder.cpu(cpu);
        }

        let resources = resources_builder.build().context("build linux resources")?;

        Ok(resources)
    }

    async fn move_vcpus_to_sandbox_cgroup(
        &mut self,
        hv_pids: &VcpuThreadIds,
    ) -> Result<usize> {
        let mut pids = hv_pids.vcpus.values();

        // Use threaded mode only in cgroup v1 + cgroupfs
        if !self.sandbox_cgroup.systemd() && !self.sandbox_cgroup.v2() {
            for pid in pids {
                let pid = CgroupPid::from(*pid as u64);
                self.sandbox_cgroup
                    .add_thread(pid)
                    .with_context(|| format!("add vcpu pid {}", pid.pid))?
            }
        } else {
            // No vCPU, exits early
            let vcpu = match pids.next() {
                Some(pid) => *pid,
                None => return Ok(0),
            };

            let tgid = get_tgid_from_pid(vcpu as i32).context("get tgid from vCPU thread")? as u64;
            self.sandbox_cgroup
                .add_proc(CgroupPid::from(tgid))
                .with_context(|| format!("add vcpu tgid {tgid}"))?;
        }

        Ok(hv_pids.vcpus.len())
    }

    async fn update_sandbox_cgroups(&mut self, hypervisor: &dyn Hypervisor) -> Result<bool> {
        let needs_thread_ids =
            self.overhead_cgroup.is_some() || self.enable_vcpus_pinning;

        let thread_ids = if needs_thread_ids {
            Some(
                hypervisor
                    .get_thread_ids()
                    .await
                    .context("get vCPU thread IDs")?,
            )
        } else {
            None
        };

        // The runtime is under overhead cgroup if available. The
        // hypervisor as a child of the runtime is under the overhead
        // cgroup by default. We should move VMM process/vCPU threads to
        // the sandbox cgroup to prevent them from consuming excessive
        // resources.
        if self.overhead_cgroup.is_some() {
            let vcpu_num = self
                .move_vcpus_to_sandbox_cgroup(thread_ids.as_ref().unwrap())
                .await
                .context("move vcpus to sandbox cgroup")?;
            // The cgroup managers will not create cgroups if no processes
            // are added to it. `vcpu_num == 0` reflects that the
            // hypervisor hasn't been started yet. We skip resource
            // setting, as the sandbox cgroup might not be created yet.
            if vcpu_num == 0 {
                return Ok(false);
            }
        }

        let sandbox_resources = self.collect_resources().context("collect resources")?;
        self.sandbox_cgroup.set(&sandbox_resources).context("set")?;

        if let Some(thread_ids) = thread_ids {
            self.check_vcpus_pinning(thread_ids)
                .context("check vCPUs pinning")?;
        }

        Ok(true)
    }

    fn collect_sandbox_cpuset(&self) -> CpuSet {
        let mut cpuset = CpuSet::new();
        for res in self.resources.values() {
            let Some(cpus_str) = res.cpu().as_ref().and_then(|c| c.cpus().as_deref()) else {
                continue;
            };
            match CpuSet::from_str(cpus_str) {
                Ok(parsed) => cpuset.extend(&parsed),
                Err(e) => warn!(
                    sl!(),
                    "vCPU pinning: failed to parse cpuset \"{}\": {}", cpus_str, e
                ),
            }
        }
        cpuset
    }

    fn set_thread_affinity(tid: u32, cpus: &[u32]) -> Result<()> {
        let nix_cpuset = build_nix_cpuset(cpus)?;
        sched_setaffinity(Pid::from_raw(tid as i32), &nix_cpuset).map_err(|e| {
            anyhow!(
                "sched_setaffinity failed for thread {} to cpus {:?}: {}",
                tid,
                cpus,
                e
            )
        })
    }

    fn check_vcpus_pinning(&mut self, thread_ids: VcpuThreadIds) -> Result<()> {
        if !self.enable_vcpus_pinning {
            return Ok(());
        }

        let cpuset = self.collect_sandbox_cpuset();
        let cpuset_slice: Vec<u32> = cpuset.iter().copied().collect();

        let num_vcpus = thread_ids.vcpus.len();
        let num_cpus = cpuset_slice.len();

        if num_vcpus == 0 || num_cpus == 0 || num_vcpus != num_cpus {
            if num_vcpus == 0 {
                info!(sl!(), "vCPU pinning: no vCPU threads found, skipping");
            } else if num_cpus == 0 {
                info!(sl!(), "vCPU pinning: no cpuset configured, skipping");
            } else {
                info!(
                    sl!(),
                    "vCPU pinning: vCPU count ({}) != cpuset size ({}), pinning not possible",
                    num_vcpus,
                    num_cpus
                );
            }
            if self.is_vcpus_pinning_on && num_vcpus > 0 {
                info!(sl!(), "vCPU pinning: resetting previous pinning");
                self.reset_vcpus_pinning(&thread_ids.vcpus, &cpuset_slice)?;
                self.is_vcpus_pinning_on = false;
            }
            return Ok(());
        }

        // Pin vCPU i to cpuset_slice[i] (both sorted by index)
        let mut sorted_vcpus: Vec<(u32, u32)> = thread_ids.vcpus.into_iter().collect();
        sorted_vcpus.sort_by_key(|(idx, _)| *idx);

        for (i, (_vcpu_idx, tid)) in sorted_vcpus.iter().enumerate() {
            if let Err(e) = Self::set_thread_affinity(*tid, &cpuset_slice[i..i + 1]) {
                // On failure, reset all pinning and propagate the error
                let all_vcpus: HashMap<u32, u32> = sorted_vcpus.iter().copied().collect();
                let _ = self.reset_vcpus_pinning(&all_vcpus, &cpuset_slice);
                return Err(e).context(format!(
                    "failed to pin vCPU thread {} to CPU {}",
                    tid, cpuset_slice[i]
                ));
            }
        }

        self.is_vcpus_pinning_on = true;
        info!(
            sl!(),
            "vCPU pinning: pinned {} vCPU threads to cpuset {:?}", num_vcpus, cpuset_slice
        );
        Ok(())
    }

    fn reset_vcpus_pinning(
        &self,
        vcpus: &HashMap<u32, u32>,
        cpuset_slice: &[u32],
    ) -> Result<()> {
        if cpuset_slice.is_empty() {
            return Ok(());
        }
        for tid in vcpus.values() {
            Self::set_thread_affinity(*tid, cpuset_slice).with_context(|| {
                format!("failed to reset vCPU thread {} affinity", tid)
            })?;
        }
        Ok(())
    }
}

impl CgroupsResourceInner {
    pub(crate) async fn delete(&mut self) -> Result<()> {
        self.sandbox_cgroup
            .destroy()
            .context("destroy sandbox cgroup")?;

        if let Some(overhead_cgroup) = self.overhead_cgroup.as_mut() {
            overhead_cgroup
                .destroy()
                .context("destroy overhead cgroup")?;
        }

        Ok(())
    }

    pub(crate) async fn update(
        &mut self,
        cid: &str,
        resources: Option<&LinuxResources>,
        op: ResourceUpdateOp,
        hypervisor: &dyn Hypervisor,
    ) -> Result<()> {
        let old = match op {
            ResourceUpdateOp::Add | ResourceUpdateOp::Update => {
                let resources = resources.ok_or_else(|| {
                    anyhow::anyhow!("resources should not be empty for Add or Update operation")
                })?;
                let new = new_cpuset_resources(resources).context("new cpuset resources")?;
                let old = self.resources.insert(cid.to_string(), new.clone());
                // If the new resources are the same as the old ones, we
                // can skip the update.
                if let Some(old) = old.as_ref() {
                    if old == &new {
                        return Ok(());
                    }
                }
                old
            }
            ResourceUpdateOp::Del => self.resources.remove(cid),
        };

        let ret = self
            .update_sandbox_cgroups(hypervisor)
            .await
            .context("update sandbox cgroups");

        // Rollback if the update fails
        if ret.is_err() {
            match op {
                ResourceUpdateOp::Add => {
                    self.resources.remove(cid);
                }
                ResourceUpdateOp::Update | ResourceUpdateOp::Del => {
                    if let Some(old) = old {
                        self.resources.insert(cid.to_string(), old);
                    }
                }
            }
        }

        ret.map(|_| ())
    }

    pub(crate) async fn setup_after_start_vm(&mut self, hypervisor: &dyn Hypervisor) -> Result<()> {
        let updated = self
            .update_sandbox_cgroups(hypervisor)
            .await
            .context("update sandbox cgroups after start vm")?;

        // There is an overhead cgroup and we are falling to move the vCPUs
        // to the sandbox cgroup, it results in those threads being under
        // the overhead cgroup, and allowing them to consume more resources
        // than we have allocated for the sandbox.
        if self.overhead_cgroup.is_some() && !updated {
            return Err(anyhow!("hypervisor cannot be moved to sandbox cgroup"));
        }

        Ok(())
    }
}

/// Build a `NixCpuSet` from a slice of CPU ids.
fn build_nix_cpuset(cpus: &[u32]) -> Result<NixCpuSet> {
    let mut nix_cpuset = NixCpuSet::new();
    for cpu_id in cpus {
        nix_cpuset
            .set(*cpu_id as usize)
            .map_err(|e| anyhow!("failed to set CPU {} in cpuset: {}", cpu_id, e))?;
    }
    Ok(nix_cpuset)
}

/// Copy cpu.cpus and cpu.mems from the given resources to new resources.
fn new_cpuset_resources(resources: &LinuxResources) -> Result<LinuxResources> {
    let cpu = resources.cpu();
    let cpus = cpu.as_ref().and_then(|c| c.cpus().clone());
    let mems = cpu.as_ref().and_then(|c| c.mems().clone());

    let mut builder = LinuxCpuBuilder::default();
    if let Some(cpus) = cpus {
        builder = builder.cpus(cpus);
    }
    if let Some(mems) = mems {
        builder = builder.mems(mems);
    }
    let linux_cpu = builder.build().context("build linux cpu")?;

    let builder = LinuxResourcesBuilder::default().cpu(linux_cpu);
    let resources = builder.build().context("build linux resources")?;

    Ok(resources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn make_resources_with_cpus(cpus: &str) -> LinuxResources {
        let cpu = LinuxCpuBuilder::default()
            .cpus(cpus.to_string())
            .build()
            .unwrap();
        LinuxResourcesBuilder::default()
            .cpu(cpu)
            .build()
            .unwrap()
    }

    fn make_inner_for_test(enable_pinning: bool) -> CgroupsResourceInner {
        CgroupsResourceInner {
            resources: HashMap::new(),
            sandbox_cgroup: Box::new(
                FsManager::new("test_sandbox_cgroup_pinning").unwrap(),
            ),
            overhead_cgroup: None,
            enable_vcpus_pinning: enable_pinning,
            is_vcpus_pinning_on: false,
        }
    }

    #[rstest]
    #[case::empty(vec![], vec![])]
    #[case::single_range(vec![("c1", "0-3")], vec![0, 1, 2, 3])]
    #[case::single_list(vec![("c1", "0,2,4")], vec![0, 2, 4])]
    #[case::multi_container_disjoint(
        vec![("c1", "0,1"), ("c2", "2,3")],
        vec![0, 1, 2, 3]
    )]
    #[case::multi_container_overlapping(
        vec![("c1", "0-2"), ("c2", "1-3")],
        vec![0, 1, 2, 3]
    )]
    #[case::three_containers(
        vec![("c1", "0"), ("c2", "4-6"), ("c3", "2")],
        vec![0, 2, 4, 5, 6]
    )]
    fn test_collect_sandbox_cpuset(
        #[case] containers: Vec<(&str, &str)>,
        #[case] expected: Vec<u32>,
    ) {
        let mut inner = make_inner_for_test(true);
        for (cid, cpus) in containers {
            inner
                .resources
                .insert(cid.to_string(), make_resources_with_cpus(cpus));
        }
        let cpuset = inner.collect_sandbox_cpuset();
        let cpus: Vec<u32> = cpuset.iter().copied().collect();
        assert_eq!(cpus, expected);
    }

    #[test]
    fn test_collect_sandbox_cpuset_no_cpu_field() {
        let mut inner = make_inner_for_test(true);
        let resources = LinuxResourcesBuilder::default().build().unwrap();
        inner.resources.insert("c1".to_string(), resources);
        let cpuset = inner.collect_sandbox_cpuset();
        assert!(cpuset.is_empty());
    }

    #[rstest]
    #[case::specific_cpus(&[0, 2, 4], &[0, 2, 4], &[1, 3])]
    #[case::contiguous(&[0, 1, 2, 3], &[0, 1, 2, 3], &[4, 5])]
    #[case::single(&[7], &[7], &[0, 6])]
    fn test_build_nix_cpuset(
        #[case] input: &[u32],
        #[case] expected_set: &[u32],
        #[case] expected_unset: &[u32],
    ) {
        let cpuset = build_nix_cpuset(input).unwrap();
        for cpu in expected_set {
            assert!(
                cpuset.is_set(*cpu as usize).unwrap(),
                "CPU {} should be set",
                cpu
            );
        }
        for cpu in expected_unset {
            assert!(
                !cpuset.is_set(*cpu as usize).unwrap(),
                "CPU {} should not be set",
                cpu
            );
        }
    }

    #[test]
    fn test_build_nix_cpuset_empty() {
        let cpuset = build_nix_cpuset(&[]).unwrap();
        assert!(!cpuset.is_set(0).unwrap());
    }

    #[rstest]
    #[case::disabled(false, false)]
    #[case::enabled(true, false)]
    fn test_pinning_initial_state(#[case] enable: bool, #[case] expected_on: bool) {
        let inner = make_inner_for_test(enable);
        assert_eq!(inner.enable_vcpus_pinning, enable);
        assert_eq!(inner.is_vcpus_pinning_on, expected_on);
    }
}
