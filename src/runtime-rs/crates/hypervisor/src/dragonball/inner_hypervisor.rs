// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::path::Path;

use anyhow::{anyhow, Context, Ok, Result};
use dragonball::ALL_THREADS;
use kata_types::capabilities::Capabilities;

use super::inner::DragonballInner;
use crate::dragonball::seccomp::get_seccomp_filter;
use crate::snapshot::require_snapshot_files;
use crate::utils::{self, get_hvsock_path, get_jailer_root, get_sandbox_path};
use crate::{RestoreVmRequest, VcpuThreadIds, VmmState};

/// Snapshot artifacts written by save_vm() under the snapshot directory.
const DRAGONBALL_SNAPSHOT_STATE_FILE: &str = "state";
const DRAGONBALL_SNAPSHOT_MEMORY_FILE: &str = "memory";

impl DragonballInner {
    pub(crate) async fn prepare_vm(
        &mut self,
        id: &str,
        netns: Option<String>,
        selinux_label: Option<String>,
    ) -> Result<()> {
        self.id = id.to_string();
        self.state = VmmState::NotReady;

        self.vm_path = get_sandbox_path(id);
        self.jailer_root = get_jailer_root(id);
        self.netns = netns;

        // Dragonball is a built-in VMM that runs as a thread inside the
        // runtime. Because it is not a standalone process, Dragonball cannot
        // independently set per-VM SELinux exec labels; any provided
        // selinux_label will be ignored.
        if selinux_label.is_some() {
            warn!(sl!(),
                    "SELinux label is provided for Dragonball VM, but Dragonball does not support SELinux; the label will be ignored",
            );
        }

        if !self.config.security_info.disable_seccomp {
            let seccomp =
                HashMap::from([(ALL_THREADS.to_string(), get_seccomp_filter(ALL_THREADS))]);
            self.vmm_instance.set_seccomp(seccomp);
        }

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

    /// Save the paused microVM: guest memory to `<dir>/memory`, vCPU and
    /// device state to `<dir>/state`.
    pub(crate) async fn save_vm(&self, snapshot_dir: &Path) -> Result<()> {
        #[cfg(target_arch = "x86_64")]
        {
            let state_path = snapshot_dir.join(DRAGONBALL_SNAPSHOT_STATE_FILE);
            let memory_path = snapshot_dir.join(DRAGONBALL_SNAPSHOT_MEMORY_FILE);
            info!(
                sl!(),
                "saving microVM snapshot";
                "state_path" => state_path.display().to_string(),
                "memory_path" => memory_path.display().to_string(),
            );
            self.vmm_instance
                .save_microvm(
                    state_path.to_string_lossy().into_owned(),
                    memory_path.to_string_lossy().into_owned(),
                )
                .context("save microvm")
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            let _ = snapshot_dir;
            Err(anyhow!("save_vm is not supported on this architecture"))
        }
    }

    pub(crate) async fn restore_vm(&mut self, request: RestoreVmRequest) -> Result<()> {
        if request.timeout_secs <= 0 {
            return Err(anyhow!("restore timeout must be positive"));
        }
        Self::validate_snapshot(&request.snapshot_dir)?;

        #[cfg(target_arch = "x86_64")]
        {
            let state_path = request.snapshot_dir.join(DRAGONBALL_SNAPSHOT_STATE_FILE);
            let memory_path = request.snapshot_dir.join(DRAGONBALL_SNAPSHOT_MEMORY_FILE);
            self.run_vmm_server().context("start vmm server")?;
            self.restore_vm_from_snapshot(
                state_path.to_string_lossy().into_owned(),
                memory_path.to_string_lossy().into_owned(),
                request.timeout_secs,
            )
            .await
            .inspect_err(|error| {
                error!(sl!(), "restore micro vm error {:?}", error);
                if let Err(stop_error) = self.stop_vm() {
                    error!(
                        sl!(),
                        "failed to stop VM after restore error: {:?}", stop_error
                    );
                }
            })
        }
        #[cfg(not(target_arch = "x86_64"))]
        Err(anyhow!("restore_vm is not supported on this architecture"))
    }

    pub(crate) fn validate_snapshot(snapshot_dir: &Path) -> Result<()> {
        require_snapshot_files(
            snapshot_dir,
            &[
                DRAGONBALL_SNAPSHOT_STATE_FILE,
                DRAGONBALL_SNAPSHOT_MEMORY_FILE,
            ],
        )
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
        self.cleanup_resource()
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
        Ok(Vec::from_iter(pids))
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
