// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! OpenVMM device management over the standalone VM service.

use anyhow::{anyhow, Context, Result};

use super::inner::OpenVmmInner;
use crate::device::DeviceType;
use crate::{VmmState, KATA_BLK_DEV_TYPE};

impl OpenVmmInner {
    pub(crate) async fn add_device(&mut self, device: DeviceType) -> Result<DeviceType> {
        if self.state == VmmState::NotReady {
            info!(sl!(), "openvmm: VMM not ready, queueing device {}", device);
            self.pending_devices.push(device.clone());
            return Ok(device);
        }

        match device {
            DeviceType::BlockModern(block_device) => {
                let (device_id, path_on_host, is_readonly, driver_option) = {
                    let block = block_device.lock().await;
                    (
                        block.device_id.clone(),
                        block.config.path_on_host.clone(),
                        block.config.is_readonly,
                        block.config.driver_option.clone(),
                    )
                };

                if driver_option != KATA_BLK_DEV_TYPE {
                    return Err(anyhow!(
                        "openvmm only supports '{}' block hotplug, got '{}'",
                        KATA_BLK_DEV_TYPE,
                        driver_option
                    ));
                }

                if path_on_host.is_empty() {
                    return Err(anyhow!("openvmm block hotplug requires a host path"));
                }

                let port = self.reserve_block_hotplug_port(&device_id)?;
                let hotplug_result = self
                    .vmm_instance
                    .add_pcie_device(&port.name, path_on_host.clone(), is_readonly)
                    .await
                    .with_context(|| {
                        format!(
                            "failed to hotplug block device {} into PCIe port {}",
                            path_on_host, port.name
                        )
                    });

                if let Err(err) = hotplug_result {
                    let _ = self.release_block_hotplug_port(&device_id);
                    return Err(err);
                }

                info!(
                    sl!(),
                    "openvmm: hotplugged block device {} as virtio-blk-pci at port {} (pci_path {})",
                    path_on_host,
                    port.name,
                    port.pci_path
                );

                // The agent resolves the device from its guest PCI path; make
                // sure no stale SCSI address is left set.
                let mut block = block_device.lock().await;
                block.config.pci_path = Some(port.pci_path.clone());
                block.config.scsi_addr = None;
                drop(block);
                Ok(DeviceType::BlockModern(block_device))
            }
            other => {
                if matches!(other, DeviceType::Vfio(_)) {
                    return Err(anyhow!(
                        "openvmm: VFIO devices are cold-plug only and must be \
                         added before start_vm; got {} after VMM start",
                        other
                    ));
                }
                warn!(sl!(), "openvmm: add_device stub for {}", other);
                Ok(other)
            }
        }
    }

    pub(crate) async fn remove_device(&mut self, device: DeviceType) -> Result<()> {
        match device {
            DeviceType::BlockModern(block_device) => {
                let device_id = block_device.lock().await.device_id.clone();
                let Some(port) = self.block_hotplug_port(&device_id) else {
                    warn!(
                        sl!(),
                        "openvmm: no hotplug mapping found for block device {}", device_id
                    );
                    return Ok(());
                };

                self.vmm_instance
                    .remove_pcie_device(&port.name)
                    .await
                    .with_context(|| {
                        format!(
                            "failed to hot-remove block device {} from PCIe port {}",
                            device_id, port.name
                        )
                    })?;

                let _ = self.release_block_hotplug_port(&device_id);

                info!(
                    sl!(),
                    "openvmm: hot-removed block device {} from PCIe port {}", device_id, port.name
                );
                Ok(())
            }
            other => {
                warn!(sl!(), "openvmm: remove_device stub for {}", other);
                Ok(())
            }
        }
    }

    pub(crate) async fn update_device(&mut self, device: DeviceType) -> Result<()> {
        warn!(sl!(), "openvmm: update_device stub for {}", device);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlockConfigModern, BlockDeviceModernHandle};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn remove_block_device_keeps_port_reserved_when_rpc_fails() {
        let (exit_notify, _exit_waiter) = mpsc::channel(1);
        let mut inner = OpenVmmInner::new(exit_notify);
        let device_id = "block0";
        let port = inner.reserve_block_hotplug_port(device_id).unwrap();
        let free_ports = inner.free_block_hotplug_ports.len();
        let device = DeviceType::BlockModern(
            BlockDeviceModernHandle::new(device_id.to_string(), BlockConfigModern::default()).arc(),
        );

        assert!(inner.remove_device(device).await.is_err());

        let attached_port = inner.block_hotplug_port(device_id).unwrap();
        assert_eq!(attached_port.name, port.name);
        assert_eq!(inner.free_block_hotplug_ports.len(), free_ports);

        let released_port = inner.release_block_hotplug_port(device_id).unwrap();
        assert_eq!(released_port.name, port.name);
        assert!(inner.block_hotplug_port(device_id).is_none());
        assert_eq!(inner.free_block_hotplug_ports.len(), free_ports + 1);
    }
}
