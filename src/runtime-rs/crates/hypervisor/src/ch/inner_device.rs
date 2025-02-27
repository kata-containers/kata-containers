// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::inner::CloudHypervisorInner;
use crate::device::pci_path::PciPath;
use crate::device::DeviceType;
use crate::HybridVsockDevice;
use crate::NetworkConfig;
use crate::NetworkDevice;
use crate::ShareFsConfig;
use crate::ShareFsDevice;
use crate::VfioDevice;
use crate::VmmState;
use crate::{BlockConfig, BlockDevice};
use anyhow::{anyhow, Context, Result};
use ch_config::ch_api::cloud_hypervisor_vm_device_add;
use ch_config::ch_api::{
    cloud_hypervisor_vm_blockdev_add, cloud_hypervisor_vm_device_remove,
    cloud_hypervisor_vm_fs_add, cloud_hypervisor_vm_netdev_add, cloud_hypervisor_vm_vsock_add,
    PciDeviceInfo, VmRemoveDeviceData,
};
use ch_config::convert::{DEFAULT_DISK_QUEUES, DEFAULT_DISK_QUEUE_SIZE, DEFAULT_NUM_PCI_SEGMENTS};
use ch_config::DiskConfig;
use ch_config::{net_util::MacAddr, DeviceConfig, FsConfig, NetConfig, VsockConfig};
use safe_path::scoped_join;
use std::convert::TryFrom;
use std::path::PathBuf;

const VIRTIO_FS: &str = "virtio-fs";

pub const DEFAULT_FS_QUEUES: usize = 1;
const DEFAULT_FS_QUEUE_SIZE: u16 = 1024;

impl CloudHypervisorInner {
    pub(crate) async fn add_device(&mut self, device: DeviceType) -> Result<DeviceType> {
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
                DeviceType::ShareFs(_) => self.pending_devices.insert(0, device.clone()),
                DeviceType::Network(_) => self.pending_devices.insert(0, device.clone()),
                _ => {
                    debug!(
                        sl!(),
                        "ignoring early add device request for device: {:?}", device
                    );
                }
            }

            return Ok(device);
        }

        self.handle_add_device(device).await
    }

    async fn handle_add_device(&mut self, device: DeviceType) -> Result<DeviceType> {
        match device {
            DeviceType::ShareFs(sharefs) => self.handle_share_fs_device(sharefs).await,
            DeviceType::HybridVsock(hvsock) => self.handle_hvsock_device(hvsock).await,
            DeviceType::Block(block) => self.handle_block_device(block).await,
            DeviceType::Vfio(vfiodev) => self.handle_vfio_device(vfiodev).await,
            DeviceType::Network(netdev) => self.handle_network_device(netdev).await,
            _ => Err(anyhow!("unhandled device: {:?}", device)),
        }
    }

    /// Add the device that were requested to be added before the VMM was
    /// started.
    #[allow(dead_code)]
    pub(crate) async fn handle_pending_devices_after_boot(&mut self) -> Result<()> {
        if self.state != VmmState::VmRunning {
            return Err(anyhow!(
                "cannot handle pending devices with VMM state {:?}",
                self.state
            ));
        }

        while let Some(dev) = self.pending_devices.pop() {
            self.add_device(dev).await.context("add_device")?;
        }

        Ok(())
    }

    pub(crate) async fn remove_device(&mut self, device: DeviceType) -> Result<()> {
        match device {
            DeviceType::Vfio(vfiodev) => self.remove_vfio_device(&vfiodev).await,
            _ => Ok(()),
        }
    }

    pub(crate) async fn update_device(&mut self, _device: DeviceType) -> Result<()> {
        Ok(())
    }

    async fn handle_share_fs_device(&mut self, sharefs: ShareFsDevice) -> Result<DeviceType> {
        let device: ShareFsDevice = sharefs.clone();
        if device.config.fs_type != VIRTIO_FS {
            return Err(anyhow!(
                "cannot handle share fs type: {:?}",
                device.config.fs_type
            ));
        }

        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let num_queues: usize = if device.config.queue_num > 0 {
            device.config.queue_num as usize
        } else {
            DEFAULT_FS_QUEUES
        };

        let queue_size: u16 = if device.config.queue_num > 0 {
            u16::try_from(device.config.queue_size)?
        } else {
            DEFAULT_FS_QUEUE_SIZE
        };

        let socket_path = if device.config.sock_path.starts_with('/') {
            PathBuf::from(device.config.sock_path)
        } else {
            scoped_join(&self.vm_path, device.config.sock_path)?
        };

        let fs_config = FsConfig {
            tag: device.config.mount_tag,
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

        Ok(DeviceType::ShareFs(sharefs))
    }

    async fn handle_vfio_device(&mut self, device: VfioDevice) -> Result<DeviceType> {
        let mut vfio_device: VfioDevice = device.clone();

        // A device with multi-funtions, or a IOMMU group with one more
        // devices, the Primary device is selected to be passed to VM.
        // And the the first one is Primary device.
        // safe here, devices is not empty.
        let primary_device = device.devices.first().ok_or(anyhow!(
            "Primary device list empty for vfio device {:?}",
            device
        ))?;

        let primary_device = primary_device.clone();

        let sysfsdev = primary_device.sysfs_path.clone();

        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let device_config = DeviceConfig {
            path: PathBuf::from(sysfsdev),
            iommu: false,
            ..Default::default()
        };

        let response = cloud_hypervisor_vm_device_add(
            socket.try_clone().context("failed to clone socket")?,
            device_config,
        )
        .await?;

        if let Some(detail) = response {
            debug!(sl!(), "VFIO add response: {:?}", detail);

            // Store the cloud-hypervisor device id to be used later for remving the device
            let dev_info: PciDeviceInfo =
                serde_json::from_str(detail.as_str()).map_err(|e| anyhow!(e))?;
            self.device_ids
                .insert(device.device_id.clone(), dev_info.id);

            // Update PCI path for the vfio host device. It is safe to directly access the slice element
            // here as we have already checked if it exists.
            // Todo: Handle vfio-ap mediated devices - return error for them.
            vfio_device.devices[0].guest_pci_path =
                Some(Self::clh_pci_info_to_path(&dev_info.bdf)?);
        }

        Ok(DeviceType::Vfio(vfio_device))
    }

    async fn remove_vfio_device(&mut self, device: &VfioDevice) -> Result<()> {
        let clh_device_id = self.device_ids.get(&device.device_id);

        if clh_device_id.is_none() {
            return Err(anyhow!(
                "Device id for cloud-hypervisor not found while removing device"
            ));
        }

        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let clh_device_id = clh_device_id.unwrap();
        let rm_data = VmRemoveDeviceData {
            id: clh_device_id.clone(),
        };

        let response = cloud_hypervisor_vm_device_remove(
            socket.try_clone().context("failed to clone socket")?,
            rm_data,
        )
        .await?;

        if let Some(detail) = response {
            debug!(sl!(), "vfio remove response: {:?}", detail);
        }

        Ok(())
    }

    // Various cloud-hypervisor APIs report a PCI address in "BB:DD.F"
    // form within the PciDeviceInfo struct.
    // eg "0000:00:DD.F"
    fn clh_pci_info_to_path(bdf: &str) -> Result<PciPath> {
        let tokens: Vec<&str> = bdf.split(':').collect();
        if tokens.len() != 3 || tokens[0] != "0000" || tokens[1] != "00" {
            return Err(anyhow!(
                "Unexpected PCI address {:?} for clh device add",
                bdf
            ));
        }

        let toks: Vec<&str> = tokens[2].split('.').collect();
        if toks.len() != 2 || toks[1] != "0" || toks[0].len() != 2 {
            return Err(anyhow!(
                "Unexpected PCI address {:?} for clh device add",
                bdf
            ));
        }

        PciPath::try_from(toks[0])
    }

    async fn handle_hvsock_device(&mut self, device: HybridVsockDevice) -> Result<DeviceType> {
        let hvsock_config = device.config.clone();
        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let vsock_config = VsockConfig {
            cid: hvsock_config.guest_cid.into(),
            socket: hvsock_config.uds_path.into(),
            ..Default::default()
        };

        let response = cloud_hypervisor_vm_vsock_add(
            socket.try_clone().context("failed to clone socket")?,
            vsock_config,
        )
        .await?;

        if let Some(detail) = response {
            debug!(sl!(), "hvsock add response: {:?}", detail);
        }

        Ok(DeviceType::HybridVsock(device))
    }

    async fn handle_block_device(&mut self, device: BlockDevice) -> Result<DeviceType> {
        let mut block_dev = device.clone();
        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let mut disk_config = DiskConfig::try_from(device.config.clone())?;
        disk_config.direct = self.config.blockdev_info.block_device_cache_direct;

        let response = cloud_hypervisor_vm_blockdev_add(
            socket.try_clone().context("failed to clone socket")?,
            disk_config,
        )
        .await?;

        if let Some(detail) = response {
            debug!(sl!(), "blockdev add response: {:?}", detail);

            let dev_info: PciDeviceInfo =
                serde_json::from_str(detail.as_str()).map_err(|e| anyhow!(e))?;
            self.device_ids.insert(device.device_id, dev_info.id);
            block_dev.config.pci_path = Some(Self::clh_pci_info_to_path(dev_info.bdf.as_str())?);
        }

        Ok(DeviceType::Block(block_dev))
    }

    async fn handle_network_device(&mut self, device: NetworkDevice) -> Result<DeviceType> {
        let netdev = device.clone();

        let socket = self
            .api_socket
            .as_ref()
            .ok_or("missing socket")
            .map_err(|e| anyhow!(e))?;

        let clh_net_config = NetConfig::try_from(device.config)?;

        let response = cloud_hypervisor_vm_netdev_add(
            socket.try_clone().context("failed to clone socket")?,
            clh_net_config,
        )
        .await?;

        if let Some(detail) = response {
            debug!(sl!(), "netdev add response: {:?}", detail);
        }

        Ok(DeviceType::Network(netdev))
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

impl TryFrom<BlockConfig> for DiskConfig {
    type Error = anyhow::Error;

    fn try_from(blkcfg: BlockConfig) -> Result<Self, Self::Error> {
        let disk_config: DiskConfig = DiskConfig {
            path: Some(blkcfg.path_on_host.as_str().into()),
            readonly: blkcfg.is_readonly,
            num_queues: DEFAULT_DISK_QUEUES,
            queue_size: DEFAULT_DISK_QUEUE_SIZE,
            ..Default::default()
        };

        Ok(disk_config)
    }
}

#[derive(Debug)]
pub struct ShareFsSettings {
    cfg: ShareFsConfig,
    vm_path: String,
}

impl ShareFsSettings {
    pub fn new(cfg: ShareFsConfig, vm_path: String) -> Self {
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
            allow_duplicate_mac: false,
            use_generic_irq: None,
            use_shared_irq: None,
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
