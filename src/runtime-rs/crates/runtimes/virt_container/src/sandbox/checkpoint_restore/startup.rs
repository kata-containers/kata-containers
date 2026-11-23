// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use hypervisor::HYPERVISOR_DRAGONBALL;
use resource::ResourceConfig;
use tokio::time::timeout;

use crate::sandbox::startup::StartMode;
use crate::sandbox::VirtSandbox;

use super::RestoreBoot;

fn prepare_resources(resources: &mut Vec<ResourceConfig>, mode: StartMode, hypervisor_name: &str) {
    if matches!(mode, StartMode::Restore) && hypervisor_name != HYPERVISOR_DRAGONBALL {
        resources.retain(|r| !matches!(r, ResourceConfig::VmRootfs(_)));
    }
}

async fn await_agent_health<F, T>(health: F, ready_timeout: Duration) -> Result<()>
where
    F: std::future::Future<Output = Result<T>>,
{
    match timeout(ready_timeout, health).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(anyhow!("restored agent health probe timed out: {err}")),
    }
}

impl VirtSandbox {
    pub(in crate::sandbox) async fn resolve_start_mode(&self) -> StartMode {
        if self.restore_state.read().await.is_restoring() {
            StartMode::Restore
        } else {
            StartMode::Normal
        }
    }

    pub(in crate::sandbox) async fn prepare_restore_boot(
        &self,
        resources: &mut Vec<ResourceConfig>,
        mode: StartMode,
    ) -> Option<RestoreBoot> {
        let hypervisor_name = self
            .resource_manager
            .config()
            .await
            .runtime
            .hypervisor_name
            .clone();
        let boot = self.restore_state.write().await.take_boot();
        prepare_resources(resources, mode, &hypervisor_name);

        boot
    }

    /// Register restored workload disks after the base devices exist, then
    /// validate the complete device inventory before starting the VM.
    pub(in crate::sandbox) async fn register_and_validate_restore_devices(
        &self,
        mode: StartMode,
        boot: Option<&mut RestoreBoot>,
    ) -> Result<()> {
        if let Some(boot) = boot {
            self.register_restore_blocks(boot).await?;
        }

        if matches!(mode, StartMode::Restore) {
            self.validate_restore_device_inventory().await?;
        }

        Ok(())
    }

    async fn register_restore_blocks(&self, boot: &mut RestoreBoot) -> Result<()> {
        // Preserve the cold-start device topology for snapshot restore. In particular, Dragonball assigns MMIO addresses and IRQs
        // in device creation order, so managed workload disks must follow the VM's rootfs, network, and vsock devices just as they
        // did originally.
        let device_manager = self.resource_manager.get_device_manager().await;
        for block in std::mem::take(&mut boot.blocks) {
            device_manager
                .write()
                .await
                .restore_block_device(block)
                .await
                .context("restore workload block device")?;
        }

        Ok(())
    }

    pub(crate) async fn connect_restored_agent(&self, address: &str) -> Result<()> {
        self.agent
            .start(address)
            .await
            .context(format!("connect restore agent server at {:?}", address))?;

        self.wait_agent_ready().await
    }

    /// Prove that the post-reset ttrpc accept loop can serve a request before
    /// issuing restore RPCs that must never be replayed.
    async fn wait_agent_ready(&self) -> Result<()> {
        const READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
        let health_result = await_agent_health(
            self.agent.check(agent::CheckRequest::new("")),
            READY_TIMEOUT,
        )
        .await;

        if let Err(health_err) = health_result {
            if let Err(disconnect_err) = self.agent.disconnect().await {
                return Err(health_err).context(format!(
                    "invalidate failed restored agent transport: {disconnect_err:#}"
                ));
            }
            return Err(health_err);
        }

        Ok(())
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    use hypervisor::BlockConfigModern;
    use resource::network::{NetworkConfig, NetworkWithNetNsConfig};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[test]
    fn preserves_resource_order() {
        for name in [HYPERVISOR_DRAGONBALL, "qemu", "cloud-hypervisor"] {
            for mode in [StartMode::Normal, StartMode::Restore] {
                let mut resources = vec![
                    ResourceConfig::GuestExtensionImage(BlockConfigModern {
                        path_on_host: "extension".into(),
                        ..Default::default()
                    }),
                    ResourceConfig::VmRootfs(BlockConfigModern::default()),
                    ResourceConfig::Network(NetworkConfig::NetNs(NetworkWithNetNsConfig {
                        network_model: "tcfilter".into(),
                        netns_path: String::new(),
                        queues: 1,
                        network_created: false,
                    })),
                    ResourceConfig::InitData(BlockConfigModern {
                        path_on_host: "initdata".into(),
                        ..Default::default()
                    }),
                ];

                prepare_resources(&mut resources, mode, name);

                let keep_rootfs = mode != StartMode::Restore || name == HYPERVISOR_DRAGONBALL;
                assert_eq!(resources.len(), if keep_rootfs { 4 } else { 3 });
                assert!(
                    matches!(&resources[0], ResourceConfig::GuestExtensionImage(c)
                        if c.path_on_host == "extension")
                );
                if keep_rootfs {
                    assert!(matches!(&resources[1], ResourceConfig::VmRootfs(_)));
                }
                let network = if keep_rootfs { 2 } else { 1 };
                assert!(matches!(
                    &resources[network],
                    ResourceConfig::Network { .. }
                ));
                assert!(
                    matches!(&resources[network + 1], ResourceConfig::InitData(c)
                        if c.path_on_host == "initdata")
                );
            }
        }
    }

    #[tokio::test]
    async fn delayed_health_uses_one_request() {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let health = async move {
            observed.fetch_add(1, Ordering::AcqRel);
            tokio::time::sleep(Duration::from_millis(400)).await;
            Ok::<_, anyhow::Error>(())
        };

        await_agent_health(health, Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::Acquire), 1);
    }
}
