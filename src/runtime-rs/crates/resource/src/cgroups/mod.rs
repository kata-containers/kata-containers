// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod cgroup_persist;
mod ops;
mod utils;

use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use cgroup_persist::CgroupState;
use cgroups_rs::cgroup::CGROUP_MODE_THREADED;
use cgroups_rs::cgroup_builder::CgroupBuilder;
use cgroups_rs::{hierarchies, Cgroup, CgroupPid, CpuResources, Resources};
use hypervisor::Hypervisor;
use kata_sys_util::spec::load_oci_spec;
use kata_types::config::TomlConfig;
use oci::LinuxResources;
use oci_spec::runtime as oci;
use persist::sandbox_persist::Persist;
use tokio::sync::RwLock;

use crate::cgroups::ops::{delete_v1_cgroups, delete_v2_cgroups};
use crate::ResourceUpdateOp;

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
        let spec = load_oci_spec().ok();
        let v2 = hierarchies::is_cgroup2_unified_mode();
        let (path, overhead_path) = utils::new_cgroup_paths(sid, spec.as_ref(), v2);

        Ok(Self {
            path,
            overhead_path,
            sandbox_cgroup_only: toml_config.runtime.sandbox_cgroup_only,
        })
    }
}

impl CgroupConfig {
    /// Returns true if we are using cgroup v2 threaded mode. The threaded
    /// mode is enabled when cgroup v2 and overhead cgroup
    /// (sandbox_cgroup_only=false) is enabled.
    fn threaded_mode(&self) -> bool {
        hierarchies::is_cgroup2_unified_mode() && !self.sandbox_cgroup_only
    }

    /// Returns threaded controllers. Please make sure that cgroup v2 is
    /// on.
    fn threaded_controllers(&self) -> Vec<String> {
        vec!["cpuset".to_string(), "cpu".to_string()]
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

        // Init cgroups for sandbox and overhead
        let (sandbox_cgroup, overhead_cgroup) =
            Self::new_cgroups(&config).context("new cgroups")?;

        // Add the runtime to the VMM sandbox resource controller
        let pid = CgroupPid { pid: 0 };
        if let Some(cgroup) = overhead_cgroup.as_ref() {
            cgroup
                .add_task_by_tgid(pid)
                .context("add runtime to overhead cgroup")?;
        } else {
            sandbox_cgroup
                .add_task_by_tgid(pid)
                .context("add task by tgid with sandbox only")?;
        }

        Ok(Self {
            cgroup_manager: sandbox_cgroup,
            resources: Arc::new(RwLock::new(HashMap::new())),
            overhead_cgroup_manager: overhead_cgroup,
            cgroup_config: config,
        })
    }

    fn new_cgroups(config: &CgroupConfig) -> Result<(Cgroup, Option<Cgroup>)> {
        let new_cgroup = |path: &str| -> Result<Cgroup> {
            let mut cg_builder = CgroupBuilder::new(path);
            if config.threaded_mode() {
                // write "+{controller} to cgroup.subtree_control
                cg_builder = cg_builder.set_specified_controllers(config.threaded_controllers());
            }
            let hier = hierarchies::auto();
            let cgroup = cg_builder
                .build(hier)
                .map_err(|err| anyhow!("failed to build cgroup: {:?}", err))?;

            // Set cgroup type to "threaded" if needed
            if config.threaded_mode() {
                cgroup
                    .set_cgroup_type(CGROUP_MODE_THREADED)
                    .context("set threaded mode")?;
            }

            Ok(cgroup)
        };

        let sandbox_cgroup = new_cgroup(&config.path).context("new sandbox cgroup")?;
        let overhead_cgroup = if config.sandbox_cgroup_only {
            None
        } else {
            Some(new_cgroup(&config.overhead_path).context("new overhead cgroup")?)
        };

        Ok((sandbox_cgroup, overhead_cgroup))
    }

    /// delete will move the running processes in the cgroup_manager and
    /// overhead_cgroup_manager to the parent and then delete the cgroups.
    pub async fn delete(&self) -> Result<()> {
        if !hierarchies::is_cgroup2_unified_mode() {
            delete_v1_cgroups(self).context("delete v1 cgroups")?;
        } else {
            delete_v2_cgroups(self).context("delete v2 cgroups")?;
        }

        Ok(())
    }

    pub async fn update_cgroups(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
        op: ResourceUpdateOp,
        h: &dyn Hypervisor,
    ) -> Result<()> {
        let new_resources = self.calc_resource(linux_resources);
        let old_resources = self.update_resources(cid, new_resources.clone(), op).await;

        if let Some(old_resource) = old_resources.clone() {
            if old_resource == new_resources {
                return Ok(());
            }
        }

        match self.do_update_cgroups(h).await {
            Err(e) => {
                // if update failed, we should roll back the records in resources
                let mut resources = self.resources.write().await;
                match op {
                    ResourceUpdateOp::Add => {
                        resources.remove(cid);
                    }
                    ResourceUpdateOp::Update | ResourceUpdateOp::Del => {
                        if let Some(old_resource) = old_resources {
                            resources.insert(cid.to_owned(), old_resource);
                        }
                    }
                }
                Err(e)
            }
            Ok(()) => Ok(()),
        }
    }

    async fn update_resources(
        &self,
        cid: &str,
        new_resource: Resources,
        op: ResourceUpdateOp,
    ) -> Option<Resources> {
        let mut resources = self.resources.write().await;
        match op {
            ResourceUpdateOp::Add | ResourceUpdateOp::Update => {
                resources.insert(cid.to_owned(), new_resource.clone())
            }
            ResourceUpdateOp::Del => resources.remove(cid),
        }
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
                .add_task(CgroupPid { pid: *tid as u64 })?
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
        let cpus = linux_resources
            .and_then(|res| res.cpu().clone())
            .and_then(|cpu| cpu.cpus().clone());

        let mems = linux_resources
            .and_then(|res| res.cpu().clone())
            .and_then(|cpu| cpu.mems().clone());

        CpuResources {
            cpus,
            mems,
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
