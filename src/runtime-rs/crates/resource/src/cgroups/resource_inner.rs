// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::{HashMap, HashSet};
use std::process;

use anyhow::{anyhow, Context, Result};
use cgroups_rs::manager::is_systemd_cgroup;
use cgroups_rs::{CgroupPid, FsManager, Manager, SystemdManager};
use hypervisor::Hypervisor;
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
        })
    }

    pub(crate) fn restore(config: &CgroupConfig) -> Result<Self> {
        let (sandbox_cgroup, overhead_cgroup) =
            Self::new_cgroup_managers(config).context("restore cgroups")?;

        Ok(Self {
            resources: HashMap::new(),
            sandbox_cgroup,
            overhead_cgroup,
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

    async fn move_vcpus_to_sandbox_cgroup(&mut self, hypervisor: &dyn Hypervisor) -> Result<usize> {
        let hv_pids = hypervisor.get_thread_ids().await?;
        let mut pids = hv_pids.vcpus.values();

        // Use threaded mode only in cgroup v1 + cgroupfs
        if !self.sandbox_cgroup.systemd() && !self.sandbox_cgroup.v2() {
            for pid in pids {
                let pid = CgroupPid::from(*pid as u64);
                self.sandbox_cgroup
                    .add_thread(pid)
                    .with_context(|| format!("add vcpu pid {}", pid.as_raw()))?
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
                .with_context(|| format!("add vcpu tgid {}", tgid))?;
        }

        Ok(hv_pids.vcpus.len())
    }

    async fn update_sandbox_cgroups(&mut self, hypervisor: &dyn Hypervisor) -> Result<bool> {
        // The runtime is under overhead cgroup if available. The
        // hypervisor as a child of the runtime is under the overhead
        // cgroup by default. We should move VMM process/vCPU threads to
        // the sandbox cgroup to prevent them from consuming excessive
        // resources.
        if self.overhead_cgroup.is_some() {
            let vcpu_num = self
                .move_vcpus_to_sandbox_cgroup(hypervisor)
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

        Ok(true)
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
