// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::inner::CloudHypervisorInner;
use crate::device::driver::vhost_user_blk::VhostUserBlkDevice;
use crate::device::pci_path::PciPath;
use crate::device::DeviceType;
use crate::utils::open_named_tuntap;
use crate::utils::{check_unix_socket_path_len, link_rootless_vmm_socket};
use crate::virtio_blk_modern::BlockSourceFormat;
use crate::HybridVsockDevice;
use crate::NetworkConfig;
use crate::NetworkDevice;
use crate::ProtectionDeviceConfig;
use crate::ShareFsConfig;
use crate::ShareFsDevice;
use crate::VfioDevice;
use crate::VhostUserConfig;
use crate::VmmState;
use crate::{BlockConfigModern, BlockDeviceModern};
use anyhow::{anyhow, ensure, Context, Result};
use ch_config::ch_api::cloud_hypervisor_vm_device_add;
use ch_config::ch_api::{
    cloud_hypervisor_vm_blockdev_add, cloud_hypervisor_vm_device_remove,
    cloud_hypervisor_vm_fs_add, cloud_hypervisor_vm_info, cloud_hypervisor_vm_netdev_add_with_fds,
    cloud_hypervisor_vm_vsock_add, PciDeviceInfo, VmRemoveDeviceData,
};
use ch_config::convert::{DEFAULT_DISK_QUEUES, DEFAULT_DISK_QUEUE_SIZE, DEFAULT_NUM_PCI_SEGMENTS};
use ch_config::DiskConfig;
use ch_config::ImageType;
use ch_config::{
    net_util::MacAddr, DeviceConfig, FsConfig, NetConfig, ProtectionDevConfig, VsockConfig,
};
use kata_sys_util::netns::NetnsGuard;
use kata_types::config::hypervisor::RateLimiterConfig;
use kata_types::rootless::is_rootless;
use kata_types::vmdk::{validate_vmdk_layout, VmdkConfig};

use safe_path::scoped_join;
use std::convert::TryFrom;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::fd::AsRawFd;
use std::os::fd::IntoRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use virtio_bindings::bindings::virtio_blk::VIRTIO_BLK_ID_BYTES;

const VIRTIO_FS: &str = "virtio-fs";

// Cloud Hypervisor answers vm.remove-device as soon as it has asked the
// guest to eject the device. The device, and with it the connection to a
// vhost-user backend, only goes away once the guest has acknowledged the
// eject, which normally takes well under a second.
const DEVICE_REMOVAL_TIMEOUT: Duration = Duration::from_secs(10);
const DEVICE_REMOVAL_POLL_INTERVAL: Duration = Duration::from_millis(50);

impl CloudHypervisorInner {
    pub(crate) async fn add_device(&mut self, device: DeviceType) -> Result<DeviceType> {
        if let DeviceType::Network(net) = &device {
            ensure!(
                (1..=self.network_queue_limit()).contains(&net.config.queue_num),
                "network queue pairs must be normalized before attaching {}",
                net.config.host_dev_name
            );
        }
        if self.state != VmmState::VmRunning {
            // If the VM is not running, add the device to the pending list to
            // be handled later.
            //
            // Note that:
            //
            // - ShareFs (virtiofsd) is only needed in a non-DM scenario
            //   for the container rootfs.
            //
            // - A DeviceType::BlockModern requested before the VM is running
            //   has to be cold-plugged, meaning it is turned into an entry of
            //   VmConfig.disks (see 'convert.rs'). This is required for devices
            //   the guest needs early on in its boot, such as the initdata
            //   image. Container rootfs block devices are unaffected as they
            //   are added *after* the VM has started and hence hot-plugged.
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
                DeviceType::Vfio(_) => self.pending_devices.insert(0, device.clone()),
                DeviceType::Protection(_) => self.pending_devices.insert(0, device.clone()),
                DeviceType::BlockModern(_) => self.pending_devices.insert(0, device.clone()),
                // The guest finds a vhost-user-blk disk by the PCI path that
                // Cloud Hypervisor assigns when hot-plugging it, so it cannot
                // be queued for boot like a BlockModern device.
                DeviceType::VhostUserBlk(dev) => {
                    return Err(anyhow!(
                        "cannot add vhost-user-blk device {} before the VM is running",
                        dev.device_id
                    ));
                }
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
            DeviceType::BlockModern(block) => self.handle_block_device(block).await,
            DeviceType::VhostUserBlk(vhu_blk) => self.handle_vhost_user_blk_device(vhu_blk).await,
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
            DeviceType::Vfio(vfiodev) => self.inner_remove_device(vfiodev.device_id.as_str()).await,
            DeviceType::BlockModern(blockdev) => {
                let device_id = blockdev.lock().await.device_id.clone();
                self.inner_remove_device(device_id.as_str()).await
            }
            DeviceType::VhostUserBlk(vhu_blk) => {
                self.remove_vhost_user_blk_device(&vhu_blk.device_id, DEVICE_REMOVAL_TIMEOUT)
                    .await
            }
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

        let num_queues = device.config.queue_num as usize;
        let queue_size = u16::try_from(device.config.queue_size)?;

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

        let response = cloud_hypervisor_vm_fs_add(&self.api_socket, fs_config).await?;

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

        let device_config = DeviceConfig {
            path: PathBuf::from(sysfsdev),
            iommu: false.into(),
            ..Default::default()
        };

        let response = cloud_hypervisor_vm_device_add(&self.api_socket, device_config).await?;

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

    async fn inner_remove_device(&mut self, device_id: &str) -> Result<()> {
        let clh_device_id = self.device_ids.get(device_id);

        if clh_device_id.is_none() {
            return Err(anyhow!(
                "Device id for cloud-hypervisor not found while removing device"
            ));
        }

        let clh_device_id = clh_device_id.unwrap();
        let rm_data = VmRemoveDeviceData {
            id: clh_device_id.clone(),
        };

        let response = cloud_hypervisor_vm_device_remove(&self.api_socket, rm_data).await?;

        if let Some(detail) = response {
            debug!(sl!(), "device remove response: {:?}", detail);
        }

        self.device_ids.remove(device_id);

        Ok(())
    }

    async fn handle_vhost_user_blk_device(
        &mut self,
        mut device: VhostUserBlkDevice,
    ) -> Result<DeviceType> {
        let mut disk_config = DiskConfig::try_from(&device.config)
            .with_context(|| format!("vhost-user-blk device {}", device.device_id))?;

        // Cloud Hypervisor rejects a disk with more queues than boot vCPUs.
        let max_queues = self.disk_queue_limit();
        if disk_config.num_queues > max_queues {
            info!(
                sl!(),
                "Reducing vhost-user-blk queues from {} to {}", disk_config.num_queues, max_queues
            );
            disk_config.num_queues = max_queues;
        }

        info!(
            sl!(),
            "hot-plugging vhost-user-blk device";
            "device" => &device.device_id,
            "socket" => disk_config.vhost_socket.as_deref().unwrap_or_default(),
            "num_queues" => disk_config.num_queues,
            "queue_size" => disk_config.queue_size,
        );

        let response = cloud_hypervisor_vm_blockdev_add(&self.api_socket, disk_config)
            .await
            .with_context(|| format!("add vhost-user-blk device {}", device.device_id))?
            .ok_or_else(|| {
                anyhow!(
                    "no device information returned for vhost-user-blk device {}",
                    device.device_id
                )
            })?;

        let dev_info: PciDeviceInfo = serde_json::from_str(&response)
            .with_context(|| format!("invalid vm.add-disk response {response:?}"))?;

        let pci_path = match Self::clh_pci_info_to_path(&dev_info.bdf) {
            Ok(pci_path) => pci_path,
            Err(e) => {
                // Don't leave a disk in the guest that nothing keeps track of.
                let rm_data = VmRemoveDeviceData {
                    id: dev_info.id.clone(),
                };
                if let Err(rm_err) =
                    cloud_hypervisor_vm_device_remove(&self.api_socket, rm_data).await
                {
                    warn!(
                        sl!(),
                        "failed to remove vhost-user-blk device {}: {:?}", dev_info.id, rm_err
                    );
                }
                return Err(e);
            }
        };

        info!(
            sl!(),
            "vhost-user-blk device hot-plugged";
            "device" => &device.device_id,
            "clh_id" => &dev_info.id,
            "bdf" => &dev_info.bdf,
        );

        self.device_ids
            .insert(device.device_id.clone(), dev_info.id);
        device.config.pci_path = Some(pci_path);

        Ok(DeviceType::VhostUserBlk(device))
    }

    async fn remove_vhost_user_blk_device(
        &mut self,
        device_id: &str,
        timeout: Duration,
    ) -> Result<()> {
        let clh_id = self
            .device_ids
            .get(device_id)
            .cloned()
            .ok_or_else(|| anyhow!("vhost-user-blk device {device_id} is not attached"))?;

        // The device went away with the VM, whose API socket may be closed
        // already: this happens when a container is cleaned up after the
        // sandbox has been stopped.
        if self.state != VmmState::VmRunning {
            info!(
                sl!(),
                "VM is not running, forgetting vhost-user-blk device";
                "device" => device_id,
                "clh_id" => &clh_id,
            );
            self.device_ids.remove(device_id);
            return Ok(());
        }

        // A removal that timed out is retried later on, by which time the guest
        // may have completed the eject: Cloud Hypervisor would then answer 404.
        if !self.device_present(&clh_id).await? {
            info!(
                sl!(),
                "vhost-user-blk device already removed";
                "device" => device_id,
                "clh_id" => &clh_id,
            );
            self.device_ids.remove(device_id);
            return Ok(());
        }

        self.inner_remove_device(device_id)
            .await
            .with_context(|| format!("remove vhost-user-blk device {device_id}"))?;

        // Only return once the device is gone, which closes the vhost-user
        // connection: a backend such as SPDK refuses to delete a controller
        // that still has a connected front-end.
        if let Err(e) = self.wait_for_device_removal(&clh_id, timeout).await {
            // Keep track of the device so that removing it can be retried.
            self.device_ids.insert(device_id.to_string(), clh_id);
            return Err(e);
        }

        info!(
            sl!(),
            "vhost-user-blk device removed";
            "device" => device_id,
            "clh_id" => &clh_id,
        );

        Ok(())
    }

    fn disk_queue_limit(&self) -> usize {
        (self.config.cpu_info.default_vcpus.ceil() as usize).max(1)
    }

    async fn device_present(&self, clh_id: &str) -> Result<bool> {
        let vm_info = cloud_hypervisor_vm_info(&self.api_socket).await?;

        Ok(vm_info
            .device_tree
            .is_some_and(|tree| tree.contains_key(clh_id)))
    }

    async fn wait_for_device_removal(&self, clh_id: &str, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;

        loop {
            if !self.device_present(clh_id).await? {
                return Ok(());
            }

            if Instant::now() >= deadline {
                return Err(anyhow!(
                    "device {clh_id} was not removed from the VM within {timeout:?}"
                ));
            }

            tokio::time::sleep(DEVICE_REMOVAL_POLL_INTERVAL).await;
        }
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

        let vsock_config = VsockConfig {
            cid: hvsock_config.guest_cid,
            socket: hvsock_config.uds_path.into(),
            ..Default::default()
        };

        let response = cloud_hypervisor_vm_vsock_add(&self.api_socket, vsock_config).await?;

        if let Some(detail) = response {
            debug!(sl!(), "hvsock add response: {:?}", detail);
        }

        Ok(DeviceType::HybridVsock(device))
    }

    fn make_disk_config(&self, config: &BlockConfigModern) -> Result<DiskConfig> {
        if let BlockSourceFormat::Vmdk(vmdk) = &config.source {
            write_vmdk_descriptor(Path::new(&config.path_on_host), vmdk)?;
        }

        let mut disk_config = DiskConfig::try_from(config.clone())?;

        disk_config.direct = config
            .is_direct
            .unwrap_or(self.config.blockdev_info.block_device_cache_direct);

        disk_config.rate_limiter_config = RateLimiterConfig::new(
            self.config.blockdev_info.disk_rate_limiter_bw_max_rate,
            self.config.blockdev_info.disk_rate_limiter_ops_max_rate,
            self.config
                .blockdev_info
                .disk_rate_limiter_bw_one_time_burst,
            self.config
                .blockdev_info
                .disk_rate_limiter_ops_one_time_burst,
        );

        Ok(disk_config)
    }

    fn is_vm_boot_file(&self, path: &str) -> bool {
        let boot_info = &self.config.boot_info;

        (!boot_info.image.is_empty() && path == boot_info.image)
            || (!boot_info.initrd.is_empty() && path == boot_info.initrd)
    }

    async fn handle_block_device(
        &mut self,
        device: Arc<Mutex<BlockDeviceModern>>,
    ) -> Result<DeviceType> {
        // Build the cloud-hypervisor DiskConfig from a snapshot of the device config.
        let (device_id, config) = {
            let dev = device.lock().await;
            (dev.device_id.clone(), dev.config.clone())
        };

        let disk_config = self.make_disk_config(&config)?;

        let response = cloud_hypervisor_vm_blockdev_add(&self.api_socket, disk_config).await?;

        if let Some(detail) = response {
            debug!(sl!(), "blockdev add response: {:?}", detail);

            let dev_info: PciDeviceInfo =
                serde_json::from_str(detail.as_str()).map_err(|e| anyhow!(e))?;
            self.device_ids.insert(device_id.clone(), dev_info.id);

            // Persist the cloud-hypervisor assigned PCI path back into the device
            // so it can be used later for removal / hot-unplug.
            let mut block_dev = device.lock().await;
            block_dev.config.pci_path = Some(Self::clh_pci_info_to_path(dev_info.bdf.as_str())?);
        }

        Ok(DeviceType::BlockModern(device))
    }

    async fn handle_network_device(&mut self, device: NetworkDevice) -> Result<DeviceType> {
        let netdev = device.clone();

        let mut clh_net_config = NetConfig::try_from(device.config)?;
        // When using fds to pass the tap device to cloud-hypervisor, tap and id fields should be None
        clh_net_config.tap = None;
        clh_net_config.id = None;
        let files = open_named_tuntap(&netdev.config.host_dev_name, netdev.config.queue_num as u32)
            .context("open named tuntap")?;

        let fds = files.iter().map(|f| f.as_raw_fd()).collect();

        let response =
            cloud_hypervisor_vm_netdev_add_with_fds(&self.api_socket, clh_net_config, fds).await?;

        if let Some(detail) = response {
            debug!(sl!(), "netdev add response: {:?}", detail);
        }

        Ok(DeviceType::Network(netdev))
    }

    pub(crate) async fn get_shared_devices(
        &mut self,
    ) -> Result<(
        Option<Vec<FsConfig>>,
        Option<Vec<NetConfig>>,
        Option<Vec<DeviceConfig>>,
        Option<ProtectionDevConfig>,
        Option<Vec<DiskConfig>>,
    )> {
        let mut shared_fs_devices = Vec::<FsConfig>::new();
        let mut network_devices = Vec::<NetConfig>::new();
        let mut host_devices = Vec::<DeviceConfig>::new();
        let mut protection_device = ProtectionDevConfig::default();
        let mut boot_disks = Vec::<DiskConfig>::new();

        while let Some(dev) = self.pending_devices.pop() {
            match dev {
                DeviceType::ShareFs(dev) => {
                    let settings = ShareFsSettings::new(dev.config, self.vm_path.clone());

                    let mut fs_cfg = FsConfig::try_from(settings)?;
                    if is_rootless() {
                        fs_cfg.socket = link_rootless_vmm_socket(&self.id, &fs_cfg.socket)
                            .context("link rootless virtiofsd socket")?;
                    }
                    check_unix_socket_path_len(&fs_cfg.socket)?;

                    shared_fs_devices.push(fs_cfg);
                }
                DeviceType::Network(net_device) => {
                    let network_queues_pairs = net_device.config.queue_num;

                    let mut net_config = NetConfig::try_from(net_device.config.clone())?;
                    // When using fds to pass the tap device to cloud-hypervisor, tap and id fields should be None
                    net_config.tap = None;
                    net_config.id = None;

                    info!(
                        sl!(),
                        "network device queue pairs {:?}", network_queues_pairs
                    );

                    // we need ensure opening network device happens in netns.
                    let netns = self.netns.clone().unwrap_or_default();
                    let _netns_guard = NetnsGuard::new(&netns).context("new netns guard")?;
                    let fds = open_named_tuntap(
                        &net_device.config.host_dev_name,
                        network_queues_pairs as u32,
                    )
                    .context("open named tuntap")?
                    .into_iter()
                    .map(|f| f.into_raw_fd())
                    .collect();
                    net_config.fds = Some(fds);
                    network_devices.push(net_config);
                }
                DeviceType::Vfio(vfio_device) => {
                    // A device with multi-funtions, or a IOMMU group with one more
                    // devices, the Primary device is selected to be passed to VM.
                    // And the the first one is Primary device.
                    // safe here, devices is not empty.
                    let primary_device = vfio_device.devices.first().ok_or(anyhow!(
                        "Primary device list empty for vfio device {:?}",
                        vfio_device
                    ))?;

                    let primary_device = primary_device.clone();
                    let sysfsdev = primary_device.sysfs_path.clone();
                    let device_config = DeviceConfig {
                        path: PathBuf::from(sysfsdev),
                        iommu: false.into(),
                        ..Default::default()
                    };
                    info!(
                        sl!(),
                        "get host_devices primary device {:?}", primary_device
                    );
                    host_devices.push(device_config);
                }
                DeviceType::Protection(pdev) => {
                    let config = pdev.config;
                    match config {
                        ProtectionDeviceConfig::SevSnp(sevsnp_cfg) => {
                            if sevsnp_cfg.is_snp {
                                protection_device.host_data = sevsnp_cfg.host_data;
                            }
                        }
                        _ => info!(sl!(), "CH: unsupported protection device type"),
                    }
                }
                DeviceType::BlockModern(block_device) => {
                    let config = block_device.lock().await.config.clone();

                    if self.is_vm_boot_file(&config.path_on_host) {
                        // Already handled through the VmConfig payload/disks.
                        continue;
                    }

                    info!(sl!(), "cold-plugging block device {:?}", &config);

                    boot_disks.push(self.make_disk_config(&config)?);
                }
                _ => continue,
            }
        }

        Ok((
            Some(shared_fs_devices),
            Some(network_devices),
            Some(host_devices),
            Some(protection_device),
            Some(boot_disks),
        ))
    }
}

impl TryFrom<NetworkConfig> for NetConfig {
    type Error = anyhow::Error;

    fn try_from(cfg: NetworkConfig) -> Result<Self, Self::Error> {
        if let Some(mac) = cfg.guest_mac {
            let net_config = NetConfig {
                tap: Some(cfg.host_dev_name.clone()),
                id: Some(cfg.virt_iface_name.clone()),
                // Kata counts RX/TX pairs; CLH counts individual virtqueues.
                num_queues: cfg.queue_num * 2,
                queue_size: cfg.queue_size as u16,
                mac: MacAddr { bytes: mac.0 },
                ..Default::default()
            };

            return Ok(net_config);
        }

        Err(anyhow!("Missing mac address for network device"))
    }
}

impl TryFrom<BlockConfigModern> for DiskConfig {
    type Error = anyhow::Error;

    fn try_from(blkcfg: BlockConfigModern) -> Result<Self, Self::Error> {
        // Cloud Hypervisor rejects serials longer than the virtio-blk ID.
        if blkcfg.serial_override.len() > VIRTIO_BLK_ID_BYTES as usize {
            return Err(anyhow!(
                "virtio-blk serial must not exceed {VIRTIO_BLK_ID_BYTES} bytes"
            ));
        }

        // Cloud Hypervisor necessitates setting backing_files for VMDK.
        let (image_type, backing_files) = match blkcfg.source {
            BlockSourceFormat::Raw => (ImageType::Raw, false),
            BlockSourceFormat::Vmdk(_) => (ImageType::FlatVmdk, true),
        };

        let disk_config: DiskConfig = DiskConfig {
            path: Some(blkcfg.path_on_host.as_str().into()),
            serial: if blkcfg.serial_override.is_empty() {
                None
            } else {
                Some(blkcfg.serial_override)
            },
            readonly: blkcfg.is_readonly,
            // Boot-time devices such as extensions leave queue settings unset.
            // Cloud Hypervisor requires nonzero values even for cold-plugged disks.
            num_queues: match blkcfg.num_queues {
                0 => DEFAULT_DISK_QUEUES,
                n => n,
            },
            queue_size: match blkcfg.queue_size {
                0 => DEFAULT_DISK_QUEUE_SIZE,
                sz => u16::try_from(sz)?,
            },
            // CLH fallocates writable disks to their full logical size when sparse is false.
            sparse: true,
            image_type,
            backing_files,
            ..Default::default()
        };

        Ok(disk_config)
    }
}

impl TryFrom<&VhostUserConfig> for DiskConfig {
    type Error = anyhow::Error;

    fn try_from(cfg: &VhostUserConfig) -> Result<Self, Self::Error> {
        let socket = vhost_user_socket_path(&cfg.socket_path);
        ensure!(
            Path::new(socket).is_absolute(),
            "vhost-user socket path {:?} is not absolute",
            cfg.socket_path
        );
        check_unix_socket_path_len(Path::new(socket))?;

        Ok(DiskConfig {
            // The disk is served by the vhost-user back-end: Cloud Hypervisor
            // rejects a disk path given together with a vhost socket.
            path: None,
            vhost_user: true,
            vhost_socket: Some(socket.to_string()),
            num_queues: match cfg.num_queues {
                0 => DEFAULT_DISK_QUEUES,
                n => n,
            },
            queue_size: match cfg.queue_size {
                0 => DEFAULT_DISK_QUEUE_SIZE,
                sz => u16::try_from(sz)
                    .with_context(|| format!("vhost-user-blk queue size {sz} is too large"))?,
            },
            ..Default::default()
        })
    }
}

// SPDK volumes name their vhost-user socket "spdk://<path>" or
// "spool://<path>", while Cloud Hypervisor needs the plain path.
fn vhost_user_socket_path(socket_path: &str) -> &str {
    socket_path
        .split_once("://")
        .map_or(socket_path, |(_, path)| path)
}

// Write the descriptor text file for a VMDK layout to the specified path
fn write_vmdk_descriptor(descriptor_path: &Path, vmdk: &VmdkConfig) -> Result<()> {
    let descriptor = validate_vmdk_layout(vmdk)?
        .render_descriptor_with(|extent| Ok(extent.path_on_host.clone()))?;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(descriptor_path)
        .with_context(|| {
            format!(
                "create VMDK descriptor {} for cloud-hypervisor",
                descriptor_path.display()
            )
        })?;
    file.write_all(descriptor.as_bytes()).with_context(|| {
        format!(
            "write VMDK descriptor {} for cloud-hypervisor",
            descriptor_path.display()
        )
    })?;

    Ok(())
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

        let num_queues = cfg.queue_num as usize;
        let queue_size = u16::try_from(cfg.queue_size)?;

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
    use crate::utils::MAX_UNIX_SOCKET_PATH_LEN;
    use crate::Address;
    use crate::VhostUserType;
    use ch_config::ch_api::ApiSocket;
    use ch_config::{DeviceNode, State, VmInfo};
    use rstest::rstest;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::net::UnixStream;
    use std::thread::JoinHandle;

    #[derive(Debug)]
    struct ApiRequest {
        method: String,
        endpoint: String,
        body: String,
    }

    // Stand-in for the Cloud Hypervisor HTTP API: answers the requests
    // received on `server` with `responses`, in order, and returns every
    // request it got. A request it has no response left for is recorded and
    // the connection closed, so that the client fails instead of hanging.
    fn spawn_mock_api(server: UnixStream, responses: Vec<String>) -> JoinHandle<Vec<ApiRequest>> {
        std::thread::spawn(move || {
            server
                .set_read_timeout(Some(Duration::from_secs(30)))
                .unwrap();
            let mut reader = BufReader::new(server.try_clone().unwrap());
            let mut writer = server;
            let mut responses = responses.into_iter();
            let mut requests = vec![];

            loop {
                let mut line = String::new();
                if !matches!(reader.read_line(&mut line), Ok(n) if n > 0) {
                    break;
                }
                // e.g. "PUT /api/v1/vm.add-disk HTTP/1.1"
                let mut request_line = line.split_whitespace();
                let method = request_line.next().unwrap().to_string();
                let endpoint = request_line.next().unwrap().replace("/api/v1/", "");

                let mut content_length = 0;
                loop {
                    let mut header = String::new();
                    reader.read_line(&mut header).unwrap();
                    if header == "\r\n" {
                        break;
                    }
                    if let Some(len) = header.strip_prefix("Content-Length: ") {
                        content_length = len.trim().parse().unwrap();
                    }
                }
                let mut body = vec![0; content_length];
                reader.read_exact(&mut body).unwrap();

                requests.push(ApiRequest {
                    method,
                    endpoint,
                    body: String::from_utf8(body).unwrap(),
                });

                match responses.next() {
                    Some(response) => writer.write_all(response.as_bytes()).unwrap(),
                    None => break,
                }
            }

            requests
        })
    }

    // Cloud Hypervisor sends the status code without a reason phrase.
    fn http_response(status: u16, body: &str) -> String {
        if body.is_empty() {
            format!("HTTP/1.1 {status} \r\n\r\n")
        } else {
            format!(
                "HTTP/1.1 {status} \r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
        }
    }

    fn vm_info_response(device_ids: &[&str]) -> String {
        let mut info = VmInfo {
            state: State::Running,
            device_tree: Some(
                device_ids
                    .iter()
                    .map(|id| {
                        let node = DeviceNode {
                            id: id.to_string(),
                            ..Default::default()
                        };
                        (id.to_string(), node)
                    })
                    .collect(),
            ),
            ..Default::default()
        };
        info.config.cpus.max_phys_bits = 46;

        http_response(200, &serde_json::to_string(&info).unwrap())
    }

    fn running_clh(api_socket: UnixStream) -> CloudHypervisorInner {
        let mut clh = CloudHypervisorInner {
            api_socket: ApiSocket::new(Some(api_socket)),
            state: VmmState::VmRunning,
            ..Default::default()
        };
        clh.config.cpu_info.default_vcpus = 4.0;
        clh
    }

    fn vhost_user_blk(device_id: &str, socket_path: &str) -> VhostUserBlkDevice {
        VhostUserBlkDevice::new(
            device_id.to_string(),
            VhostUserConfig {
                socket_path: socket_path.to_string(),
                device_type: VhostUserType::Blk,
                driver_option: "blk".to_string(),
                num_queues: 4,
                queue_size: 256,
                ..Default::default()
            },
        )
    }

    const ADD_DISK_RESPONSE: &str = r#"{"id":"_disk2","bdf":"0000:00:06.0"}"#;

    #[tokio::test]
    async fn test_vhost_user_blk_hotplug() {
        let (client, server) = UnixStream::pair().unwrap();
        let api = spawn_mock_api(server, vec![http_response(200, ADD_DISK_RESPONSE)]);
        let mut clh = running_clh(client);

        let device = vhost_user_blk("vhu-blk-1", "spdk:///run/spdk/vhost-blk.0");
        let added = clh
            .add_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap();

        let DeviceType::VhostUserBlk(added) = added else {
            panic!("unexpected device {:?}", added);
        };
        assert_eq!(
            added.config.pci_path,
            Some(PciPath::try_from("06").unwrap())
        );
        assert_eq!(
            clh.device_ids.get("vhu-blk-1").map(String::as_str),
            Some("_disk2")
        );

        drop(clh);
        let requests = api.join().unwrap();
        assert_eq!(requests.len(), 1, "{:?}", requests);
        assert_eq!(requests[0].method, "PUT");
        assert_eq!(requests[0].endpoint, "vm.add-disk");
        // A vhost-user disk needs a socket and must not have a path.
        let disk: DiskConfig = serde_json::from_str(&requests[0].body).unwrap();
        assert_eq!(
            disk,
            DiskConfig {
                path: None,
                vhost_user: true,
                vhost_socket: Some("/run/spdk/vhost-blk.0".to_string()),
                num_queues: 4,
                queue_size: 256,
                ..Default::default()
            }
        );
    }

    #[tokio::test]
    async fn test_vhost_user_blk_hot_unplug() {
        let (client, server) = UnixStream::pair().unwrap();
        let api = spawn_mock_api(
            server,
            vec![
                http_response(200, ADD_DISK_RESPONSE),
                vm_info_response(&["_disk2", "_virtio-pci-_disk2"]),
                http_response(204, ""),
                // The guest has not acknowledged the eject yet ...
                vm_info_response(&["_disk2", "_virtio-pci-_disk2"]),
                // ... and now it has.
                vm_info_response(&[]),
            ],
        );
        let mut clh = running_clh(client);

        let device = vhost_user_blk("vhu-blk-1", "spdk:///run/spdk/vhost-blk.0");
        let added = clh
            .add_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap();
        clh.remove_device(added).await.unwrap();
        assert!(clh.device_ids.is_empty());

        drop(clh);
        let requests = api.join().unwrap();
        let endpoints: Vec<&str> = requests.iter().map(|r| r.endpoint.as_str()).collect();
        assert_eq!(
            endpoints,
            [
                "vm.add-disk",
                "vm.info",
                "vm.remove-device",
                "vm.info",
                "vm.info"
            ]
        );
        assert_eq!(requests[1].method, "GET");
        assert_eq!(requests[2].method, "PUT");
        let remove: VmRemoveDeviceData = serde_json::from_str(&requests[2].body).unwrap();
        assert_eq!(remove.id, "_disk2");
    }

    #[tokio::test]
    async fn test_vhost_user_blk_already_removed() {
        let (client, server) = UnixStream::pair().unwrap();
        let api = spawn_mock_api(
            server,
            vec![
                http_response(200, ADD_DISK_RESPONSE),
                // The guest completed the eject after an earlier attempt timed out.
                vm_info_response(&[]),
            ],
        );
        let mut clh = running_clh(client);

        let device = vhost_user_blk("vhu-blk-1", "/run/spdk/vhost-blk.0");
        let added = clh
            .add_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap();
        clh.remove_device(added).await.unwrap();
        assert!(clh.device_ids.is_empty());

        drop(clh);
        let requests = api.join().unwrap();
        let endpoints: Vec<&str> = requests.iter().map(|r| r.endpoint.as_str()).collect();
        assert_eq!(endpoints, ["vm.add-disk", "vm.info"]);
    }

    #[tokio::test]
    async fn test_vhost_user_blk_hot_unplug_timeout() {
        let (client, server) = UnixStream::pair().unwrap();
        let mut responses = vec![
            http_response(200, ADD_DISK_RESPONSE),
            vm_info_response(&["_disk2"]),
            http_response(204, ""),
        ];
        responses.extend((0..20).map(|_| vm_info_response(&["_disk2"])));
        let api = spawn_mock_api(server, responses);
        let mut clh = running_clh(client);

        let device = vhost_user_blk("vhu-blk-1", "/run/spdk/vhost-blk.0");
        clh.add_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap();
        let err = clh
            .remove_vhost_user_blk_device("vhu-blk-1", Duration::from_millis(120))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("was not removed"), "{:?}", err);
        // Still tracked, so that removing it can be retried.
        assert_eq!(
            clh.device_ids.get("vhu-blk-1").map(String::as_str),
            Some("_disk2")
        );

        drop(clh);
        let requests = api.join().unwrap();
        assert!(requests.len() >= 3, "{:?}", requests);
    }

    #[tokio::test]
    async fn test_vhost_user_blk_remove_after_vm_stop() {
        let (client, server) = UnixStream::pair().unwrap();
        let api = spawn_mock_api(server, vec![http_response(200, ADD_DISK_RESPONSE)]);
        let mut clh = running_clh(client);

        let device = vhost_user_blk("vhu-blk-1", "/run/spdk/vhost-blk.0");
        let added = clh
            .add_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap();

        // stop_vm() marks the VM as not running before shutting it down.
        clh.state = VmmState::NotReady;
        clh.remove_device(added).await.unwrap();
        assert!(clh.device_ids.is_empty());

        drop(clh);
        let requests = api.join().unwrap();
        assert_eq!(requests.len(), 1, "{:?}", requests);
        assert_eq!(requests[0].endpoint, "vm.add-disk");
    }

    #[rstest]
    #[case(1.0, 4, 1)]
    #[case(2.0, 4, 2)]
    #[case(1.5, 4, 2)]
    #[case(8.0, 4, 4)]
    #[tokio::test]
    async fn test_vhost_user_blk_queues_limited_by_vcpus(
        #[case] vcpus: f32,
        #[case] requested: usize,
        #[case] expected: usize,
    ) {
        let (client, server) = UnixStream::pair().unwrap();
        let api = spawn_mock_api(server, vec![http_response(200, ADD_DISK_RESPONSE)]);
        let mut clh = running_clh(client);
        clh.config.cpu_info.default_vcpus = vcpus;

        let mut device = vhost_user_blk("vhu-blk-1", "/run/spdk/vhost-blk.0");
        device.config.num_queues = requested;
        clh.add_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap();

        drop(clh);
        let requests = api.join().unwrap();
        let disk: DiskConfig = serde_json::from_str(&requests[0].body).unwrap();
        assert_eq!(disk.num_queues, expected);
    }

    #[tokio::test]
    async fn test_vhost_user_blk_before_boot() {
        let mut clh = CloudHypervisorInner::default();
        let device = vhost_user_blk("vhu-blk-1", "/run/spdk/vhost-blk.0");

        let err = clh
            .add_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("before the VM is running"),
            "{:?}",
            err
        );
        assert!(clh.pending_devices.is_empty());
    }

    #[tokio::test]
    async fn test_vhost_user_blk_unexpected_pci_address() {
        let (client, server) = UnixStream::pair().unwrap();
        let api = spawn_mock_api(
            server,
            vec![
                http_response(200, r#"{"id":"_disk2","bdf":"0000:01:00.0"}"#),
                http_response(204, ""),
            ],
        );
        let mut clh = running_clh(client);

        let device = vhost_user_blk("vhu-blk-1", "/run/spdk/vhost-blk.0");
        let err = clh
            .add_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("Unexpected PCI address"),
            "{:?}",
            err
        );
        assert!(clh.device_ids.is_empty());

        drop(clh);
        // The disk the guest got is removed again.
        let requests = api.join().unwrap();
        let endpoints: Vec<&str> = requests.iter().map(|r| r.endpoint.as_str()).collect();
        assert_eq!(endpoints, ["vm.add-disk", "vm.remove-device"]);
        assert!(requests[1].body.contains("_disk2"));
    }

    #[tokio::test]
    async fn test_vhost_user_blk_add_disk_failure() {
        let (client, server) = UnixStream::pair().unwrap();
        let api = spawn_mock_api(
            server,
            vec![http_response(
                500,
                r#"["Error from API","The disk could not be added to the VM","Timed out waiting for vhost-user connection"]"#,
            )],
        );
        let mut clh = running_clh(client);

        let device = vhost_user_blk("vhu-blk-1", "/run/spdk/vhost-blk.0");
        let err = clh
            .add_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap_err();
        assert!(
            format!("{err:#}").contains("add vhost-user-blk device vhu-blk-1"),
            "{:#}",
            err
        );
        assert!(clh.device_ids.is_empty());

        drop(clh);
        assert_eq!(api.join().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_vhost_user_blk_remove_unknown_device() {
        let mut clh = CloudHypervisorInner {
            state: VmmState::VmRunning,
            ..Default::default()
        };
        let device = vhost_user_blk("vhu-blk-1", "/run/spdk/vhost-blk.0");

        let err = clh
            .remove_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("is not attached"), "{:?}", err);
    }

    #[rstest]
    #[case("spdk:///run/spdk/vhost-blk.0", 0, 0, Some(("/run/spdk/vhost-blk.0", 1, 128)))]
    #[case("spool:///run/spool/blk.sock", 2, 1024, Some(("/run/spool/blk.sock", 2, 1024)))]
    #[case("/run/vhost-blk.sock", 1, 64, Some(("/run/vhost-blk.sock", 1, 64)))]
    #[case("spdk://vhost-blk.0", 1, 128, None)]
    #[case("", 1, 128, None)]
    #[case("/run/vhost-blk.sock", 1, 65536, None)]
    fn test_vhost_user_disk_config(
        #[case] socket_path: &str,
        #[case] num_queues: usize,
        #[case] queue_size: u32,
        #[case] expected: Option<(&str, usize, u16)>,
    ) {
        let cfg = VhostUserConfig {
            socket_path: socket_path.to_string(),
            num_queues,
            queue_size,
            ..Default::default()
        };

        match (DiskConfig::try_from(&cfg), expected) {
            (Ok(disk), Some((socket, num_queues, queue_size))) => {
                assert!(disk.vhost_user);
                assert!(disk.path.is_none());
                assert_eq!(disk.vhost_socket.as_deref(), Some(socket));
                assert_eq!(disk.num_queues, num_queues);
                assert_eq!(disk.queue_size, queue_size);
            }
            (Err(_), None) => {}
            (result, expected) => panic!("got {:?}, expected {:?}", result, expected),
        }
    }

    #[test]
    fn test_vhost_user_disk_config_socket_path_too_long() {
        let cfg = VhostUserConfig {
            socket_path: format!("spdk:///{}", "s".repeat(MAX_UNIX_SOCKET_PATH_LEN)),
            ..Default::default()
        };
        assert!(DiskConfig::try_from(&cfg).is_err());
    }

    #[tokio::test]
    async fn test_block_device_removal_forgets_device_id() {
        let (client, server) = UnixStream::pair().unwrap();
        let api = spawn_mock_api(server, vec![http_response(204, "")]);
        let mut clh = running_clh(client);
        clh.device_ids
            .insert("blk-1".to_string(), "_disk3".to_string());

        let device = BlockDeviceModern {
            device_id: "blk-1".into(),
            ..Default::default()
        };
        clh.remove_device(DeviceType::BlockModern(Arc::new(Mutex::new(device))))
            .await
            .unwrap();
        assert!(clh.device_ids.is_empty());

        drop(clh);
        let requests = api.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].endpoint, "vm.remove-device");
    }

    #[actix_rt::test]
    async fn test_cold_plug_guest_extension_images() {
        let mut ch = CloudHypervisorInner::default();
        ch.config.boot_info.image = "/rootfs.img".into();

        for (path, serial) in [
            ("/rootfs.img", ""),
            ("/coco.img", "extension-coco"),
            ("/devkit.img", "extension-devkit"),
            ("/initdata.img", ""),
        ] {
            let device = BlockDeviceModern {
                device_id: path.into(),
                attach_count: 0,
                config: BlockConfigModern {
                    path_on_host: path.into(),
                    serial_override: serial.into(),
                    is_readonly: true,
                    ..Default::default()
                },
            };
            ch.add_device(DeviceType::BlockModern(Arc::new(Mutex::new(device))))
                .await
                .unwrap();
        }

        let (_, _, _, _, disks) = ch.get_shared_devices().await.unwrap();
        let disks = disks.unwrap();
        // The boot rootfs is configured separately, so only extensions and
        // initdata belong in the shared disks returned here.
        assert!(disks
            .iter()
            .all(|disk| disk.path.as_ref() != Some(&PathBuf::from("/rootfs.img"))));
        assert_eq!(disks.len(), 3);
        for (disk, (path, serial)) in disks.iter().zip([
            ("/coco.img", Some("extension-coco")),
            ("/devkit.img", Some("extension-devkit")),
            ("/initdata.img", None),
        ]) {
            assert_eq!(disk.path, Some(PathBuf::from(path)));
            assert!(disk.readonly);
            assert_eq!(disk.serial.as_deref(), serial);
            assert_eq!(disk.num_queues, DEFAULT_DISK_QUEUES);
            assert_eq!(disk.queue_size, DEFAULT_DISK_QUEUE_SIZE);
            let json = serde_json::to_value(disk).unwrap();
            if let Some(serial) = serial {
                assert_eq!(json["serial"], serial);
            } else {
                assert!(json.get("serial").is_none());
            }
        }
        assert!(ch.pending_devices.is_empty());
    }

    #[rstest]
    #[case(0, 0, DEFAULT_DISK_QUEUES, DEFAULT_DISK_QUEUE_SIZE)]
    #[case(2, 256, 2, 256)]
    #[case(0, 256, DEFAULT_DISK_QUEUES, 256)]
    #[case(2, 0, 2, DEFAULT_DISK_QUEUE_SIZE)]
    fn test_disk_queue_settings(
        #[case] num_queues: usize,
        #[case] queue_size: u32,
        #[case] expected_queues: usize,
        #[case] expected_size: u16,
    ) {
        let disk = DiskConfig::try_from(BlockConfigModern {
            num_queues,
            queue_size,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(disk.num_queues, expected_queues);
        assert_eq!(disk.queue_size, expected_size);
    }

    #[test]
    fn test_vmdk_disk_config_uses_backing_files() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.raw");
        let second = dir.path().join("second.raw");
        let descriptor = dir.path().join("merged_fs.vmdk");
        let mut vmdk = VmdkConfig::default();
        vmdk.push_extent(first.to_str().unwrap(), 1, 0);
        vmdk.push_extent(second.to_str().unwrap(), 2, 0);

        let disk = CloudHypervisorInner::default()
            .make_disk_config(&BlockConfigModern {
                path_on_host: descriptor.display().to_string(),
                source: BlockSourceFormat::Vmdk(vmdk),
                is_readonly: true,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(disk.path, Some(descriptor.clone()));
        assert_eq!(disk.image_type, ImageType::FlatVmdk);
        assert!(disk.backing_files);
        let contents = std::fs::read_to_string(&descriptor).unwrap();
        assert!(contents.contains(&format!("RW 1 FLAT \"{}\" 0", first.display())));
        assert!(contents.contains(&format!("RW 2 FLAT \"{}\" 0", second.display())));
    }

    #[test]
    fn test_disk_queue_size_overflow() {
        assert!(DiskConfig::try_from(BlockConfigModern {
            queue_size: u16::MAX as u32 + 1,
            ..Default::default()
        })
        .is_err());
    }

    #[rstest]
    #[case("", true)]
    #[case("extension-012345678", true)]
    #[case("extension-0123456789", true)]
    #[case("extension-01234567890", false)]
    fn test_disk_serial_length(#[case] serial: &str, #[case] is_valid: bool) {
        let result = DiskConfig::try_from(BlockConfigModern {
            serial_override: serial.into(),
            ..Default::default()
        });
        assert_eq!(result.is_ok(), is_valid);
    }

    #[rstest]
    #[case(1)]
    #[case(2)]
    #[case(4)]
    fn test_networkconfig_to_netconfig(#[case] queue_pairs: usize) {
        let mut cfg = NetworkConfig {
            host_dev_name: String::from("tap0"),
            virt_iface_name: String::from("eth0"),
            queue_size: 256,
            queue_num: queue_pairs,
            guest_mac: None,
            index: 1,
            allow_duplicate_mac: false,
            use_generic_irq: None,
            use_shared_irq: None,
            pci_path: None,
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
            num_queues: cfg.queue_num * 2,
            queue_size: cfg.queue_size as u16,
            mac: MacAddr { bytes: v },
            ..Default::default()
        };

        let net = NetConfig::try_from(cfg);
        assert!(net.is_ok());
        assert_eq!(net.unwrap(), expected);
    }

    #[tokio::test]
    async fn test_network_queue_validation() {
        let mut clh = CloudHypervisorInner::default();
        clh.config.cpu_info.default_vcpus = 4.0;
        for state in [VmmState::NotReady, VmmState::VmRunning] {
            clh.state = state;
            for queue_num in [0, 5] {
                let device = NetworkDevice {
                    config: NetworkConfig {
                        queue_num,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let error = clh
                    .add_device(DeviceType::Network(device))
                    .await
                    .unwrap_err();
                assert!(error.to_string().contains("must be normalized"));
            }
        }
    }
}
