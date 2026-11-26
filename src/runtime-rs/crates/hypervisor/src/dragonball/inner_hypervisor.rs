// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::iter::FromIterator;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Ok, Result};
use dragonball::api::v1::SnapshotMemoryMode;
use dragonball::ALL_THREADS;
use kata_types::capabilities::Capabilities;

use super::inner::DragonballInner;
use crate::device::DeviceType;
use crate::dragonball::seccomp::get_seccomp_filter;
use crate::utils::{self, get_hvsock_path, get_jailer_root, get_sandbox_path};
use crate::{VcpuThreadIds, VmResourceTopology, VmRestoreOptions, VmSaveResult, VmmState};

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

    /// Save the microVM state into the VM template files configured in
    /// `[hypervisor].vm_template` (boot-to-be-template flow): guest memory
    /// contents to `memory_path`, vCPU/device state to `device_state_path`.
    /// A running VM is paused while the state is captured and resumed
    /// afterwards; an already-paused VM remains paused.
    pub(crate) async fn save_vm(&self) -> Result<()> {
        #[cfg(target_arch = "x86_64")]
        {
            let template = &self.config.vm_template;
            if template.device_state_path.is_empty() || template.memory_path.is_empty() {
                return Err(anyhow!(
                    "vm_template memory_path/device_state_path are not configured"
                ));
            }
            info!(
                sl!(),
                "saving microVM snapshot to template";
                "device_state_path" => &template.device_state_path,
                "memory_path" => &template.memory_path,
            );
            self.vmm_instance
                .save_microvm(
                    template.device_state_path.clone(),
                    template.memory_path.clone(),
                    SnapshotMemoryMode::PersistentPath,
                    false,
                )
                .context("save microvm")?;
            Ok(())
        }
        #[cfg(not(target_arch = "x86_64"))]
        Err(anyhow!("save_vm is not supported on this architecture"))
    }

    pub(crate) async fn checkpoint_vm_state_to(
        &self,
        state_dir: &Path,
        _timeout: Option<Duration>,
    ) -> Result<VmSaveResult> {
        #[cfg(target_arch = "x86_64")]
        {
            let resource_topology = self.current_resource_topology()?;
            let started = Instant::now();
            let state_partial = state_dir.join("state.partial");
            let memory_partial = state_dir.join("memory.partial");
            let state_path = state_dir.join("state");
            let save_at = Instant::now();
            let memory = self
                .vmm_instance
                .save_microvm(
                    state_partial.to_string_lossy().to_string(),
                    memory_partial.to_string_lossy().to_string(),
                    SnapshotMemoryMode::TransientUnlinked,
                    // Queue TRANSPORT_RESET only after the vCPUs are paused,
                    // and carry it in the workload snapshot.
                    true,
                )
                .context("save workload microvm")?;
            let memory = memory.ok_or_else(|| {
                anyhow!("transient Dragonball workload save returned no memory file")
            })?;
            let save_elapsed = save_at.elapsed();
            let sync_at = Instant::now();
            std::fs::File::open(&state_partial)?.sync_all()?;
            let state_sync_elapsed = sync_at.elapsed();
            let publish_at = Instant::now();
            std::fs::rename(&state_partial, &state_path)?;
            std::fs::File::open(state_dir)?.sync_all()?;
            debug!(
                sl!(),
                "Dragonball checkpoint persistence phases completed";
                "stage" => "native-artifact-publish",
                "save_microvm_us" => save_elapsed.as_micros(),
                "state_sync_us" => state_sync_elapsed.as_micros(),
                "rename_dir_sync_us" => publish_at.elapsed().as_micros(),
                "elapsed_us" => started.elapsed().as_micros(),
            );
            Ok(VmSaveResult {
                transient_memory: Some(memory.file),
                memory_base: memory.base,
                delta_ranges: memory.delta_ranges,
                resource_topology: Some(resource_topology),
            })
        }
        #[cfg(not(target_arch = "x86_64"))]
        Err(anyhow!(
            "workload checkpoint/restore is not supported on this architecture"
        ))
    }

    pub(crate) fn configure_restore(&mut self, restore: Option<VmRestoreOptions>) -> Result<()> {
        let Some(restore) = restore else {
            self.restore_path = None;
            return Ok(());
        };
        let VmRestoreOptions {
            state_path,
            network_macs,
            ..
        } = restore;
        info!(
            sl!(),
            "configure Dragonball workload restore";
            "template_enabled" => self.config.vm_template.boot_from_template,
        );
        let networks = self
            .pending_devices
            .iter()
            .filter(|device| matches!(device, DeviceType::Network(_)))
            .count();
        if networks != network_macs.len() {
            return Err(anyhow!(
                "restored network inventory has {} devices, checkpoint has {}",
                networks,
                network_macs.len()
            ));
        }
        self.restore_path = Some(state_path);
        Ok(())
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

    pub(crate) fn current_resource_topology(&self) -> Result<VmResourceTopology> {
        let vcpus = u32::try_from(self.vmm_instance.get_vcpu_tids().len())
            .context("too many vCPUs in checkpoint topology")?;
        let memory_mb = self
            .config
            .memory_info
            .default_memory
            .checked_add(self.mem_hotplug_size_mb)
            .context("checkpoint memory topology overflow")?;

        Ok(VmResourceTopology { vcpus, memory_mb })
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

#[cfg(test)]
mod restore_network_tests {
    use super::*;
    use crate::{Address, NetworkConfig, NetworkDevice, VIRTIO_BLOCK_MMIO};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn restore_keeps_registered_network_mac() {
        for (devices, expected) in [(0, 0), (1, 1), (0, 1), (1, 0), (2, 1)] {
            let (sender, _receiver) = mpsc::channel(1);
            let mut inner = DragonballInner::new(sender);
            inner.config.shared_fs.shared_fs = None;
            inner.config.blockdev_info.block_device_driver = VIRTIO_BLOCK_MMIO.into();
            inner.config.boot_info.vm_rootfs_driver = VIRTIO_BLOCK_MMIO.into();
            inner.config.network_info.disable_vhost_net = true;
            inner.config.machine_info.entropy_source.clear();
            inner.config.device_info.reclaim_guest_freed_memory = false;
            let mac = [2, 0, 0, 0, 0, 1];
            for index in 0..devices {
                inner
                    .add_device(DeviceType::Network(NetworkDevice {
                        device_id: format!("net-{index}"),
                        config: NetworkConfig {
                            host_dev_name: format!("tap-{index}"),
                            guest_mac: Some(Address(mac)),
                            ..Default::default()
                        },
                    }))
                    .await
                    .unwrap();
            }
            let state_path = std::path::PathBuf::from("checkpoint");
            let result = inner.configure_restore(Some(VmRestoreOptions {
                state_path: state_path.clone(),
                network_macs: vec![mac; expected],
                resource_topology: VmResourceTopology {
                    vcpus: 1,
                    memory_mb: 512,
                },
            }));
            if devices == expected {
                result.unwrap();
                assert_eq!(inner.restore_path, Some(state_path));
                inner.config.shared_fs.shared_fs = Some("virtio-fs".into());
                inner.configure_restore(None).unwrap();
                assert!(inner.restore_path.is_none());
            } else {
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains("restored network inventory"));
                assert!(inner.restore_path.is_none());
            }
            for device in &inner.pending_devices {
                let DeviceType::Network(network) = device else {
                    panic!("expected network device");
                };
                assert_eq!(network.config.guest_mac.as_ref().unwrap().0, mac);
                let backend = crate::dragonball::build_dragonball_network_config(
                    &inner.config,
                    &network.config,
                );
                assert_eq!(backend.guest_mac.unwrap().get_bytes(), &mac);
            }
        }
    }
}
