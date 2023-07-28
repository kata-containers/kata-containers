// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    collections::{HashMap, HashSet},
    iter::FromIterator,
};

use anyhow::{anyhow, Context, Ok, Result};
use kata_types::capabilities::Capabilities;

use super::inner::DragonballInner;
use crate::{
    utils::{self, get_hvsock_path, get_jailer_root, get_sandbox_path},
    VcpuThreadIds, VmmState,
};

impl DragonballInner {
    pub(crate) async fn prepare_vm(&mut self, id: &str, netns: Option<String>) -> Result<()> {
        self.id = id.to_string();
        self.state = VmmState::NotReady;

        self.vm_path = get_sandbox_path(id);
        self.jailer_root = get_jailer_root(id);
        self.netns = netns;

        Ok(())
    }

    // start_vm will start the hypervisor for the given sandbox.
    // In the context of dragonball, this will start the hypervisor
    pub(crate) async fn start_vm(&mut self, timeout: i32) -> Result<()> {
        self.run_vmm_server().context("start vmm server")?;
        self.cold_start_vm(timeout).await.map_err(|error| {
            error!(sl!(), "start micro vm error {:?}", error);
            if let Err(err) = self.stop_vm() {
                error!(sl!(), "failed to call end err : {:?}", err);
            }
            error
        })?;

        Ok(())
    }

    pub(crate) fn stop_vm(&mut self) -> Result<()> {
        info!(sl!(), "Stopping dragonball VM");
        self.vmm_instance.stop().context("stop")?;
        Ok(())
    }

    pub(crate) fn pause_vm(&self) -> Result<()> {
        info!(sl!(), "do pause vm");
        self.vmm_instance.pause().context("pause vm")?;
        Ok(())
    }

    pub(crate) fn resume_vm(&self) -> Result<()> {
        info!(sl!(), "do resume vm");
        self.vmm_instance.resume().context("resume vm")?;
        Ok(())
    }

    pub(crate) async fn save_vm(&self) -> Result<()> {
        todo!()
    }

    pub(crate) async fn get_agent_socket(&self) -> Result<String> {
        const HYBRID_VSOCK_SCHEME: &str = "hvsock";
        Ok(format!(
            "{}://{}",
            HYBRID_VSOCK_SCHEME,
            get_hvsock_path(&self.id),
        ))
    }

    /// Get the address of agent vsock server used to init connections for io
    pub(crate) async fn get_passfd_listener_addr(&self) -> Result<(String, u32)> {
        if let Some(passfd_port) = self.passfd_listener_port {
            Ok((get_hvsock_path(&self.id), passfd_port))
        } else {
            Err(anyhow!("passfd io listener port not set"))
        }
    }

    pub(crate) async fn get_hypervisor_metrics(&self) -> Result<String> {
        info!(sl!(), "get hypervisor metrics");
        self.vmm_instance.get_hypervisor_metrics()
    }

    pub(crate) async fn disconnect(&mut self) {
        self.state = VmmState::NotReady;
    }

    pub(crate) async fn get_thread_ids(&self) -> Result<VcpuThreadIds> {
        let mut vcpu_thread_ids: VcpuThreadIds = VcpuThreadIds {
            vcpus: HashMap::new(),
        };

        for tid in self.vmm_instance.get_vcpu_tids() {
            vcpu_thread_ids.vcpus.insert(tid.0 as u32, tid.1);
        }
        info!(sl!(), "get thread ids {:?}", vcpu_thread_ids);
        Ok(vcpu_thread_ids)
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        self.cleanup_resource();
        Ok(())
    }

    pub(crate) async fn get_pids(&self) -> Result<Vec<u32>> {
        let mut pids = HashSet::new();
        // get shim thread ids
        pids.insert(self.vmm_instance.pid());

        for tid in utils::get_child_threads(self.vmm_instance.pid()) {
            pids.insert(tid);
        }

        // remove vcpus
        for tid in self.vmm_instance.get_vcpu_tids() {
            pids.remove(&tid.1);
        }

        info!(sl!(), "get pids {:?}", pids);
        Ok(Vec::from_iter(pids.into_iter()))
    }

    pub(crate) async fn get_vmm_master_tid(&self) -> Result<u32> {
        let master_tid = self.vmm_instance.get_vmm_master_tid();
        Ok(master_tid)
    }

    pub(crate) async fn get_ns_path(&self) -> Result<String> {
        let ns_path = self.vmm_instance.get_ns_path();
        Ok(ns_path)
    }

    pub(crate) async fn check(&self) -> Result<()> {
        Ok(())
    }

    pub(crate) async fn get_jailer_root(&self) -> Result<String> {
        Ok(self.jailer_root.clone())
    }

    pub(crate) async fn capabilities(&self) -> Result<Capabilities> {
        Ok(self.capabilities.clone())
    }
}
