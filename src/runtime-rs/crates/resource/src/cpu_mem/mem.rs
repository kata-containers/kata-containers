// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Ok, Result};
use hypervisor::Hypervisor;
use oci::LinuxResources;
use oci_spec::runtime as oci;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::cpu_mem::initial_size::InitialSizeManager;
use crate::ResourceUpdateOp;

// MIB_TO_BYTES_SHIFT the number to shift needed to convert MiB to Bytes
pub const MIB_TO_BYTES_SHIFT: i32 = 20;

#[derive(Default, Debug, Clone)]
pub struct MemResource {
    /// Default memory
    pub(crate) orig_toml_default_mem: u32,

    /// CRI sandbox memory (containers + K8s pod overhead) in MiB when set at sandbox create.
    sandbox_pod_mem_mib: u32,

    /// MemResource of each container
    pub(crate) container_mem_resources: Arc<RwLock<HashMap<String, LinuxResources>>>,
}

impl MemResource {
    pub fn new(init_size_manager: InitialSizeManager) -> Result<Self> {
        Ok(Self {
            container_mem_resources: Arc::new(RwLock::new(HashMap::new())),
            orig_toml_default_mem: init_size_manager.get_orig_toml_default_mem(),
            sandbox_pod_mem_mib: init_size_manager.get_sandbox_pod_mem_mib(),
        })
    }

    pub(crate) async fn update_mem_resources(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
        op: ResourceUpdateOp,
        hypervisor: &dyn Hypervisor,
    ) -> Result<()> {
        self.update_container_mem_resources(cid, linux_resources, op)
            .await
            .context("update container memory resources")?;
        // the unit here is MB
        let mem_sb_mb = self.get_current_mb().await?;
        info!(sl!(), "calculate mem_sb_mb {}", mem_sb_mb);

        let _curr_mem = self
            .do_update_mem_resource(mem_sb_mb, hypervisor)
            .await
            .context("failed to update_mem_resource")?;

        Ok(())
    }

    pub(crate) async fn get_current_mb(&self) -> Result<u32> {
        let container_mib = self
            .total_mems()
            .await
            .context("failed to calculate total memory requirement for containers")?;
        let mut mem_sb_mb = container_mib + self.orig_toml_default_mem;

        // CRI sandbox-memory (containers + K8s pod overhead) is the floor: pod overhead
        // covers the VM base cost, so we must not resize below the CRI-requested total.
        // `orig_toml_default_mem` is not added here because pod overhead already accounts
        // for the VM base, matching the max() in setup_config.
        if self.sandbox_pod_mem_mib > 0 {
            mem_sb_mb = std::cmp::max(mem_sb_mb, self.sandbox_pod_mem_mib);
        }

        Ok(mem_sb_mb)
    }

    async fn total_mems(&self) -> Result<u32> {
        let mut mem_sandbox = 0;
        let resources = self.container_mem_resources.read().await;

        for (_, r) in resources.iter() {
            let hugepage_limits = r.hugepage_limits().clone().unwrap_or_default();
            for l in hugepage_limits {
                mem_sandbox += l.limit();
            }

            if let Some(memory) = &r.memory() {
                // set current_limit to 0 if memory limit is not set to container
                let _current_limit = memory.limit().map_or(0, |limit| {
                    mem_sandbox += limit;
                    info!(sl!(), "memory sb: {}, memory limit: {}", mem_sandbox, limit);
                    limit
                });
                // TODO support memory guest swap
                // https://github.com/kata-containers/kata-containers/issues/7293
            }
        }

        Ok((mem_sandbox >> MIB_TO_BYTES_SHIFT) as u32)
    }

    // update container_cpu_resources field
    async fn update_container_mem_resources(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
        op: ResourceUpdateOp,
    ) -> Result<()> {
        if let Some(r) = linux_resources {
            let mut resources = self.container_mem_resources.write().await;
            match op {
                ResourceUpdateOp::Add | ResourceUpdateOp::Update => {
                    resources.insert(cid.to_owned(), r.clone());
                }
                ResourceUpdateOp::Del => {
                    resources.remove(cid);
                }
            }
        }
        Ok(())
    }

    async fn do_update_mem_resource(
        &self,
        new_mem: u32,
        hypervisor: &dyn Hypervisor,
    ) -> Result<u32> {
        info!(sl!(), "requesting vmm to update memory to {:?}", new_mem);

        let (new_memory, _mem_config) = hypervisor
            .resize_memory(new_mem)
            .await
            .context("resize memory")?;

        Ok(new_memory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci_spec::runtime::{LinuxMemoryBuilder, LinuxResourcesBuilder};

    const MIB: i64 = 1024 * 1024;

    fn linux_res_limit(mem_mib: i64) -> LinuxResources {
        LinuxResourcesBuilder::default()
            .memory(
                LinuxMemoryBuilder::default()
                    .limit(mem_mib * MIB)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn get_current_mb_includes_k8s_sandbox_pod_memory_floor() {
        let container_mem_resources = Arc::new(RwLock::new(HashMap::new()));
        {
            let mut map = container_mem_resources.write().await;
            map.insert("c1".into(), linux_res_limit(400));
            map.insert("c2".into(), linux_res_limit(500));
        }

        // With CRI sandbox: containers=900, orig_toml=130 → 1030; floor=sandbox=1060.
        // Pod overhead is already in sandbox_pod_mem_mib, so we don't add orig_toml again.
        let mem_res = MemResource {
            orig_toml_default_mem: 130,
            sandbox_pod_mem_mib: 1060,
            container_mem_resources: container_mem_resources.clone(),
        };
        assert_eq!(
            mem_res.get_current_mb().await.unwrap(),
            1060,
            "resize floor is the CRI sandbox total; pod overhead covers VM base, no double-count"
        );

        // Without CRI sizing (docker run style): just container_sum + orig_toml.
        let mem_no_k8s = MemResource {
            orig_toml_default_mem: 130,
            sandbox_pod_mem_mib: 0,
            container_mem_resources,
        };
        assert_eq!(mem_no_k8s.get_current_mb().await.unwrap(), 1030);
    }
}
