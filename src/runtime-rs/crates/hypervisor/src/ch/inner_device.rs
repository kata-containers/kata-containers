// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::inner::CloudHypervisorInner;
use crate::device::DeviceType;
use crate::BlockConfig;
use crate::HybridVsockConfig;
use crate::NetworkConfig;
use crate::ShareFsDeviceConfig;
use crate::VmmState;
use anyhow::{anyhow, Context, Result};
use ch_config::ch_api::{cloud_hypervisor_vm_blockdev_add, cloud_hypervisor_vm_fs_add};
use ch_config::convert::{DEFAULT_DISK_QUEUES, DEFAULT_DISK_QUEUE_SIZE, DEFAULT_NUM_PCI_SEGMENTS};
use ch_config::DiskConfig;
use ch_config::{net_util::MacAddr, FsConfig, NetConfig};
use safe_path::scoped_join;
use std::convert::TryFrom;
use std::path::PathBuf;

const VIRTIO_FS: &str = "virtio-fs";

pub const DEFAULT_FS_QUEUES: usize = 1;
const DEFAULT_FS_QUEUE_SIZE: u16 = 1024;

impl CloudHypervisorInner {
    pub(crate) async fn add_device(&mut self, device: DeviceType) -> Result<()> {
        if self.state != VmmState::VmRunning {
            // If the VM is not running, add the device to the pending list to
            // be handled later.
            //
            // Note that the only device types considered are DeviceType::ShareFs
            // and DeviceType::Network since:
            //
            // - ShareFs (virtiofsd) is only needed in an non-DM and non-TDX scenario
            //   for the container rootfs.
            //
            // - For all other scenarios, the container rootfs is handled by a
            //   DeviceType::Block and this method is called *after* the VM
            //   has started so the device does not need to be added to the
            //   pending list.
            //
            // - The VM rootfs is handled without waiting for calls to this
            //   method as the file in question (image= or initrd=) is available
            //   from HypervisorConfig.BootInfo.{image,initrd}
            //   (see 'convert.rs').
            //
            // - Network details need to be saved for later application.
            //
            match device {
                DeviceType::ShareFs(_) => self.pending_devices.insert(0, device),
                DeviceType::Network(_) => self.pending_devices.insert(0, device),
                _ => {
                    debug!(
                        sl!(),
                        "ignoring early add device request for device: {:?}", device
                    );
                }
            }

            return Ok(());
        }

        self.handle_add_device(device).await?;

        Ok(())
    }

    async fn handle_add_device(&mut self, device: DeviceType) -> Result<()> {
        match device {
            DeviceType::ShareFs(sharefs) => self.handle_share_fs_device(sharefs.config).await,
            DeviceType::HybridVsock(hvsock) => self.handle_hvsock_device(&hvsock.config).await,
            DeviceType::Block(block) => self.handle_block_device(block.config).await,
            _ => Err(anyhow!("unhandled device: {:?}", device)),
        }
    }

    pub(crate) async fn remove_device(&mut self, _device: DeviceType) -> Result<()> {
        Ok(())
    }

    async fn handle_share_fs_device(&mut self, cfg: ShareFsDeviceConfig) -> Result<()> {
        if cfg.fs_type != VIRTIO_FS {
            return Err(anyhow!("cannot handle share fs type: {:?}", cfg.fs_type));
        }

        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let num_queues: usize = if cfg.queue_num > 0 {
            cfg.queue_num as usize
        } else {
            DEFAULT_FS_QUEUES
        };

        let queue_size: u16 = if cfg.queue_num > 0 {
            u16::try_from(cfg.queue_size)?
        } else {
            DEFAULT_FS_QUEUE_SIZE
        };

        let socket_path = if cfg.sock_path.starts_with('/') {
            PathBuf::from(cfg.sock_path)
        } else {
            scoped_join(&self.vm_path, cfg.sock_path)?
        };

        let fs_config = FsConfig {
            tag: cfg.mount_tag,
            socket: socket_path,
            num_queues,
            queue_size,
            pci_segment: DEFAULT_NUM_PCI_SEGMENTS,

            ..Default::default()
        };

        let response = cloud_hypervisor_vm_fs_add(
            socket.try_clone().context("failed to clone socket")?,
            fs_config,
        )
        .await?;

        if let Some(detail) = response {
            debug!(sl!(), "fs add response: {:?}", detail);
        }

        Ok(())
    }

    async fn handle_hvsock_device(&mut self, _cfg: &HybridVsockConfig) -> Result<()> {
        Ok(())
    }

    async fn handle_block_device(&mut self, cfg: BlockConfig) -> Result<()> {
        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let num_queues: usize = DEFAULT_DISK_QUEUES;
        let queue_size: u16 = DEFAULT_DISK_QUEUE_SIZE;

        let block_config = DiskConfig {
            path: Some(cfg.path_on_host.as_str().into()),
            readonly: cfg.is_readonly,
            num_queues,
            queue_size,
            ..Default::default()
        };

        let response = cloud_hypervisor_vm_blockdev_add(
            socket.try_clone().context("failed to clone socket")?,
            block_config,
        )
        .await?;

        if let Some(detail) = response {
            debug!(sl!(), "blockdev add response: {:?}", detail);
        }

        Ok(())
    }

    pub(crate) async fn get_shared_devices(
        &mut self,
    ) -> Result<(Option<Vec<FsConfig>>, Option<Vec<NetConfig>>)> {
        let mut shared_fs_devices = Vec::<FsConfig>::new();
        let mut network_devices = Vec::<NetConfig>::new();

        while let Some(dev) = self.pending_devices.pop() {
            match dev {
                DeviceType::ShareFs(dev) => {
                    let settings = ShareFsSettings::new(dev.config, self.vm_path.clone());

                    let fs_cfg = FsConfig::try_from(settings)?;

                    shared_fs_devices.push(fs_cfg);
                }
                DeviceType::Network(net_device) => {
                    let net_config = NetConfig::try_from(net_device.config)?;
                    network_devices.push(net_config);
                }
                _ => continue,
            }
        }

        Ok((Some(shared_fs_devices), Some(network_devices)))
    }
}

impl TryFrom<NetworkConfig> for NetConfig {
    type Error = anyhow::Error;

    fn try_from(cfg: NetworkConfig) -> Result<Self, Self::Error> {
        if let Some(mac) = cfg.guest_mac {
            let net_config = NetConfig {
                tap: Some(cfg.host_dev_name.clone()),
                id: Some(cfg.virt_iface_name.clone()),
                num_queues: cfg.queue_num,
                queue_size: cfg.queue_size as u16,
                mac: MacAddr { bytes: mac.0 },
                ..Default::default()
            };

            return Ok(net_config);
        }

        Err(anyhow!("Missing mac address for network device"))
    }
}
#[derive(Debug)]
pub struct ShareFsSettings {
    cfg: ShareFsDeviceConfig,
    vm_path: String,
}

impl ShareFsSettings {
    pub fn new(cfg: ShareFsDeviceConfig, vm_path: String) -> Self {
        ShareFsSettings { cfg, vm_path }
    }
}

impl TryFrom<ShareFsSettings> for FsConfig {
    type Error = anyhow::Error;

    fn try_from(settings: ShareFsSettings) -> Result<Self, Self::Error> {
        let cfg = settings.cfg;
        let vm_path = settings.vm_path;

        let num_queues: usize = if cfg.queue_num > 0 {
            cfg.queue_num as usize
        } else {
            DEFAULT_FS_QUEUES
        };

        let queue_size: u16 = if cfg.queue_num > 0 {
            u16::try_from(cfg.queue_size)?
        } else {
            DEFAULT_FS_QUEUE_SIZE
        };

        let socket_path = if cfg.sock_path.starts_with('/') {
            PathBuf::from(cfg.sock_path)
        } else {
            PathBuf::from(vm_path).join(cfg.sock_path)
        };

        let fs_cfg = FsConfig {
            tag: cfg.mount_tag,
            socket: socket_path,
            num_queues,
            queue_size,
            ..Default::default()
        };

        Ok(fs_cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Address;

    #[test]
    fn test_networkconfig_to_netconfig() {
        let mut cfg = NetworkConfig {
            host_dev_name: String::from("tap0"),
            virt_iface_name: String::from("eth0"),
            queue_size: 256,
            queue_num: 2,
            guest_mac: None,
            index: 1,
        };

        let net = NetConfig::try_from(cfg.clone());
        assert_eq!(
            net.unwrap_err().to_string(),
            "Missing mac address for network device"
        );

        let v: [u8; 6] = [10, 11, 128, 3, 4, 5];
        let mac_address = Address(v);
        cfg.guest_mac = Some(mac_address.clone());

        let expected = NetConfig {
            tap: Some(cfg.host_dev_name.clone()),
            id: Some(cfg.virt_iface_name.clone()),
            num_queues: cfg.queue_num,
            queue_size: cfg.queue_size as u16,
            mac: MacAddr { bytes: v },
            ..Default::default()
        };

        let net = NetConfig::try_from(cfg);
        assert!(net.is_ok());
        assert_eq!(net.unwrap(), expected);
    }
}
