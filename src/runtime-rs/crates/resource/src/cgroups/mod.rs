// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod cgroup_persist;
mod utils;

use std::{
    collections::{HashMap, HashSet},
    iter::FromIterator,
    sync::Arc,
};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use cgroup_persist::CgroupState;
use cgroups_rs::{cgroup_builder::CgroupBuilder, Cgroup, CgroupPid, CpuResources, Resources};
use hypervisor::Hypervisor;
use kata_sys_util::spec::load_oci_spec;
use kata_types::config::TomlConfig;
use oci::LinuxResources;
use persist::sandbox_persist::Persist;
use tokio::sync::RwLock;

pub struct CgroupArgs {
    pub sid: String,
    pub config: TomlConfig,
}

pub struct CgroupConfig {
    pub path: String,
    pub overhead_path: String,
    pub sandbox_cgroup_only: bool,
}

impl CgroupConfig {
    fn new(sid: &str, toml_config: &TomlConfig) -> Result<Self> {
        let overhead_path = utils::gen_overhead_path(sid);
        let spec = load_oci_spec()?;
        let path = spec
            .linux
            // The trim of '/' is important, because cgroup_path is a relative path.
            .map(|linux| linux.cgroups_path.trim_start_matches('/').to_string())
            .unwrap_or_default();

        Ok(Self {
            path,
            overhead_path,
            sandbox_cgroup_only: toml_config.runtime.sandbox_cgroup_only,
        })
    }
}

pub struct CgroupsResource {
    resources: Arc<RwLock<HashMap<String, Resources>>>,
    cgroup_manager: Cgroup,
    overhead_cgroup_manager: Option<Cgroup>,
    cgroup_config: CgroupConfig,
}

impl CgroupsResource {
    pub fn new(sid: &str, toml_config: &TomlConfig) -> Result<Self> {
        let config = CgroupConfig::new(sid, toml_config)?;

        // Create the sandbox cgroups manager (cgroups on Linux).
        // Depending on the sandbox_cgroup_only value, this cgroup
        // will either hold all the pod threads (sandbox_cgroup_only is true)
        // or only the virtual CPU ones (sandbox_cgroup_only is false).
        let hier = cgroups_rs::hierarchies::auto();
        let cgroup_manager = CgroupBuilder::new(&config.path).build(hier);

        // The shim configuration is requesting that we do not put all threads
        // into the sandbox resource controller.
        // We're creating an overhead controller, with no constraints. Everything but
        // the vCPU threads will eventually make it there.
        let overhead_cgroup_manager = if !config.sandbox_cgroup_only {
            let hier = cgroups_rs::hierarchies::auto();
            Some(CgroupBuilder::new(&config.overhead_path).build(hier))
        } else {
            None
        };

        // Add the runtime to the VMM sandbox resource controller

        // By adding the runtime process to either the sandbox or overhead controller, we are making
        // sure that any child process of the runtime (i.e. *all* processes serving a Kata pod)
        // will initially live in this controller. Depending on the sandbox_cgroup_only settings, we will
        // then move the vCPU threads between resource controllers.
        let pid = CgroupPid { pid: 0 };
        if let Some(manager) = overhead_cgroup_manager.as_ref() {
            manager.add_task_by_tgid(pid).context("add task by tgid")?;
        } else {
            cgroup_manager
                .add_task_by_tgid(pid)
                .context("add task by tgid with sandbox only")?;
        }

        Ok(Self {
            cgroup_manager,
            resources: Arc::new(RwLock::new(HashMap::new())),
            overhead_cgroup_manager,
            cgroup_config: config,
        })
    }

    /// delete will move the running processes in the cgroup_manager and
    /// overhead_cgroup_manager to the parent and then delete the cgroups.
    pub async fn delete(&self) -> Result<()> {
        for cg_pid in self.cgroup_manager.tasks() {
            self.cgroup_manager.remove_task(cg_pid);
        }

        self.cgroup_manager
            .delete()
            .context("delete cgroup manager")?;

        if let Some(overhead) = self.overhead_cgroup_manager.as_ref() {
            for cg_pid in overhead.tasks() {
                overhead.remove_task(cg_pid);
            }
            overhead.delete().context("delete overhead")?;
        }

        Ok(())
    }

    pub async fn update_cgroups(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
        h: &dyn Hypervisor,
    ) -> Result<()> {
        let resource = self.calc_resource(linux_resources);
        let changed = self.update_resources(cid, resource).await;

        if !changed {
            return Ok(());
        }

        self.do_update_cgroups(h).await
    }

    async fn update_resources(&self, cid: &str, new_resource: Resources) -> bool {
        let mut resources = self.resources.write().await;
        let old_resource = resources.insert(cid.to_owned(), new_resource.clone());

        if let Some(old_resource) = old_resource {
            if old_resource == new_resource {
                return false;
            }
        }

        true
    }

    async fn do_update_cgroups(&self, h: &dyn Hypervisor) -> Result<()> {
        let merged_resources = self.merge_resources().await;
        self.cgroup_manager
            .apply(&merged_resources)
            .map_err(|e| anyhow!(e))?;

        if self.overhead_cgroup_manager.is_some() {
            // If we have an overhead controller, new vCPU threads would start there,
            // as being children of the VMM PID.
            // We need to constrain them by moving them into the sandbox controller.
            self.constrain_hypervisor(h).await?
        }

        Ok(())
    }

    /// constrain_hypervisor will place the VMM and vCPU threads into resource controllers (cgroups on Linux).
    async fn constrain_hypervisor(&self, h: &dyn Hypervisor) -> Result<()> {
        let tids = h.get_thread_ids().await?;
        let tids = tids.vcpus.values();

        // All vCPU threads move to the sandbox controller.
        for tid in tids {
            self.cgroup_manager
                .add_task_by_tgid(CgroupPid { pid: *tid as u64 })?
        }

        Ok(())
    }

    async fn merge_resources(&self) -> Resources {
        let resources = self.resources.read().await;

        let mut cpu_list: HashSet<String> = HashSet::new();
        let mut mem_list: HashSet<String> = HashSet::new();

        resources.values().for_each(|r| {
            if let Some(cpus) = &r.cpu.cpus {
                cpu_list.insert(cpus.clone());
            }
            if let Some(mems) = &r.cpu.mems {
                mem_list.insert(mems.clone());
            }
        });

        let cpu_resource = CpuResources {
            cpus: Some(Vec::from_iter(cpu_list.into_iter()).join(",")),
            mems: Some(Vec::from_iter(mem_list.into_iter()).join(",")),
            ..Default::default()
        };

        Resources {
            cpu: cpu_resource,
            ..Default::default()
        }
    }

    fn calc_cpu_resources(&self, linux_resources: Option<&LinuxResources>) -> CpuResources {
        let cpu = || -> Option<oci::LinuxCpu> { linux_resources.as_ref()?.cpu.clone() }();

        CpuResources {
            cpus: cpu.clone().map(|cpu| cpu.cpus),
            mems: cpu.map(|cpu| cpu.mems),
            ..Default::default()
        }
    }

    fn calc_resource(&self, linux_resources: Option<&LinuxResources>) -> Resources {
        Resources {
            cpu: self.calc_cpu_resources(linux_resources),
            ..Default::default()
        }
    }
}

#[async_trait]
impl Persist for CgroupsResource {
    type State = CgroupState;
    type ConstructorArgs = CgroupArgs;
    /// Save a state of the component.
    async fn save(&self) -> Result<Self::State> {
        Ok(CgroupState {
            path: Some(self.cgroup_config.path.clone()),
            overhead_path: Some(self.cgroup_config.overhead_path.clone()),
            sandbox_cgroup_only: self.cgroup_config.sandbox_cgroup_only,
        })
    }
    /// Restore a component from a specified state.
    async fn restore(
        cgroup_args: Self::ConstructorArgs,
        cgroup_state: Self::State,
    ) -> Result<Self> {
        let hier = cgroups_rs::hierarchies::auto();
        let config = CgroupConfig::new(&cgroup_args.sid, &cgroup_args.config)?;
        let path = cgroup_state.path.unwrap_or_default();
        let cgroup_manager = Cgroup::load(hier, path.as_str());
        Ok(Self {
            cgroup_manager,
            resources: Arc::new(RwLock::new(HashMap::new())),
            overhead_cgroup_manager: None,
            cgroup_config: config,
        })
    }
}
