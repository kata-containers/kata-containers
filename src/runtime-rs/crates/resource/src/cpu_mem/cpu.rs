// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Ok, Result};
use hypervisor::Hypervisor;
use kata_types::{config::TomlConfig, cpu::LinuxContainerCpuResources};
use oci::LinuxCpu;
use oci_spec::runtime as oci;
use std::{
    cmp,
    collections::{HashMap, HashSet},
    convert::TryFrom,
    sync::Arc,
};
use tokio::sync::RwLock;

use crate::ResourceUpdateOp;

#[derive(Default, Debug, Clone)]
pub struct CpuResource {
    /// Current number of vCPUs
    pub(crate) current_vcpu: Arc<RwLock<u32>>,

    /// Default number of vCPUs
    pub(crate) default_vcpu: u32,

    /// CpuResource of each container
    pub(crate) container_cpu_resources: Arc<RwLock<HashMap<String, LinuxContainerCpuResources>>>,
}

impl CpuResource {
    pub fn new(config: Arc<TomlConfig>) -> Result<Self> {
        let hypervisor_name = config.runtime.hypervisor_name.clone();
        let hypervisor_config = config
            .hypervisor
            .get(&hypervisor_name)
            .context(format!("failed to get hypervisor {}", hypervisor_name))?;
        Ok(Self {
            current_vcpu: Arc::new(RwLock::new(hypervisor_config.cpu_info.default_vcpus as u32)),
            default_vcpu: hypervisor_config.cpu_info.default_vcpus as u32,
            container_cpu_resources: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub(crate) async fn update_cpu_resources(
        &self,
        cid: &str,
        linux_cpus: Option<&LinuxCpu>,
        op: ResourceUpdateOp,
        hypervisor: &dyn Hypervisor,
    ) -> Result<()> {
        self.update_container_cpu_resources(cid, linux_cpus, op)
            .await
            .context("update container cpu resources")?;
        let vcpu_required = self
            .calc_cpu_resources()
            .await
            .context("calculate vcpus required")?;

        if vcpu_required == self.current_vcpu().await {
            return Ok(());
        }

        let curr_vcpus = self
            .do_update_cpu_resources(vcpu_required, op, hypervisor)
            .await?;
        self.update_current_vcpu(curr_vcpus).await;
        Ok(())
    }

    pub(crate) async fn current_vcpu(&self) -> u32 {
        let current_vcpu = self.current_vcpu.read().await;
        *current_vcpu
    }

    async fn update_current_vcpu(&self, new_vcpus: u32) {
        let mut current_vcpu = self.current_vcpu.write().await;
        *current_vcpu = new_vcpus;
    }

    // update container_cpu_resources field
    async fn update_container_cpu_resources(
        &self,
        cid: &str,
        linux_cpus: Option<&LinuxCpu>,
        op: ResourceUpdateOp,
    ) -> Result<()> {
        if let Some(cpu) = linux_cpus {
            let container_resource = LinuxContainerCpuResources::try_from(cpu)?;
            let mut resources = self.container_cpu_resources.write().await;
            match op {
                ResourceUpdateOp::Add => {
                    resources.insert(cid.to_owned(), container_resource);
                }
                ResourceUpdateOp::Update => {
                    let resource = resources.insert(cid.to_owned(), container_resource.clone());
                    if let Some(old_container_resource) = resource {
                        // the priority of cpu-quota is higher than cpuset when determine the number of vcpus.
                        // we should better ignore the resource update when update cpu only by cpuset if cpu-quota
                        // has been set previously.
                        if old_container_resource.quota() > 0 && container_resource.quota() < 0 {
                            resources.insert(cid.to_owned(), old_container_resource);
                        }
                    }
                }
                ResourceUpdateOp::Del => {
                    resources.remove(cid);
                }
            }
        }

        Ok(())
    }

    // calculates the total required vcpus by adding each container's requirements within the pod
    async fn calc_cpu_resources(&self) -> Result<u32> {
        let mut total_vcpu = 0;
        let mut cpuset_vcpu: HashSet<u32> = HashSet::new();

        let resources = self.container_cpu_resources.read().await;
        for (_, cpu_resource) in resources.iter() {
            let vcpu = cpu_resource.get_vcpus().unwrap_or(0) as u32;
            cpuset_vcpu.extend(cpu_resource.cpuset().iter());
            total_vcpu += vcpu;
        }

        // contrained only by cpuset
        if total_vcpu == 0 && !cpuset_vcpu.is_empty() {
            info!(sl!(), "(from cpuset)get vcpus # {:?}", cpuset_vcpu);
            return Ok(cpuset_vcpu.len() as u32);
        }

        info!(
            sl!(),
            "(from cfs_quota&cfs_period)get vcpus count {}", total_vcpu
        );
        Ok(total_vcpu)
    }

    // do hotplug and hot-unplug the vcpu
    async fn do_update_cpu_resources(
        &self,
        new_vcpus: u32,
        op: ResourceUpdateOp,
        hypervisor: &dyn Hypervisor,
    ) -> Result<u32> {
        let old_vcpus = self.current_vcpu().await;

        // when adding vcpus, ignore old_vcpus > new_vcpus
        // when deleting vcpus, ignore old_vcpus < new_vcpus
        if (op == ResourceUpdateOp::Add && old_vcpus > new_vcpus)
            || (op == ResourceUpdateOp::Del && old_vcpus < new_vcpus)
        {
            return Ok(old_vcpus);
        }

        // do not reduce computing power
        // the number of vcpus would not be lower than the default size
        let new_vcpus = cmp::max(new_vcpus, self.default_vcpu);

        let (_, new) = hypervisor
            .resize_vcpu(old_vcpus, new_vcpus)
            .await
            .context("resize vcpus")?;

        Ok(new)
    }
}
