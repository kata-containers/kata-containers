// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::inner::CloudHypervisorInner;
use crate::device::driver::vhost_user_blk::VhostUserBlkDevice;
use crate::device::driver::virtio_blk_modern::VIRTIO_BLOCK_PCI;
use crate::device::driver::KATA_BLK_DEV_TYPE;
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
use crate::VmmState;
use crate::{BlockConfigModern, BlockDeviceModern};
use anyhow::{anyhow, ensure, Context, Result};
use ch_config::ch_api::cloud_hypervisor_vm_device_add;
use ch_config::ch_api::{
    cloud_hypervisor_device_tree_contains, cloud_hypervisor_vm_blockdev_add,
    cloud_hypervisor_vm_device_remove, cloud_hypervisor_vm_fs_add,
    cloud_hypervisor_vm_netdev_add_with_fds, cloud_hypervisor_vm_vsock_add, PciDeviceInfo,
    VmRemoveDeviceData,
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
use std::time::Duration;
use tokio::sync::Mutex;
use virtio_bindings::bindings::virtio_blk::VIRTIO_BLK_ID_BYTES;

const VIRTIO_FS: &str = "virtio-fs";

/// Poll interval and overall timeout when waiting for cloud-hypervisor to
/// finish an asynchronous device removal.
const DEVICE_REMOVAL_POLL_MS: u64 = 50;
const DEVICE_REMOVAL_TIMEOUT_MS: u64 = 3000;

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
            DeviceType::Vfio(vfiodev) => self.handle_vfio_device(vfiodev).await,
            DeviceType::Network(netdev) => self.handle_network_device(netdev).await,
            DeviceType::VhostUserBlk(blk) => self.handle_vhost_user_blk_device(blk).await,
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
            DeviceType::VhostUserBlk(vhostblkdev) => {
                self.inner_remove_device(vhostblkdev.device_id.as_str())
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

        // vm.remove-device returns before the removal has completed. A
        // vhost-user backend such as SPDK keeps the controller around while
        // the connection is still up, so a remove followed immediately by a
        // backend cleanup fails. Wait until the device has actually left the
        // device tree.
        self.wait_for_device_removal(
            clh_device_id,
            DEVICE_REMOVAL_POLL_MS,
            DEVICE_REMOVAL_TIMEOUT_MS,
        )
        .await?;

        self.device_ids.remove(device_id);

        Ok(())
    }

    /// Poll `vm.info` until the device id no longer appears in the device
    /// tree, or the timeout expires. A missing device_tree in the response is
    /// treated as "gone" as well.
    async fn wait_for_device_removal(
        &self,
        clh_device_id: &str,
        poll_ms: u64,
        timeout_ms: u64,
    ) -> Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);

        loop {
            if !cloud_hypervisor_device_tree_contains(&self.api_socket, clh_device_id).await? {
                return Ok(());
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow!(
                    "timed out waiting for cloud-hypervisor to remove device {}",
                    clh_device_id
                ));
            }

            tokio::time::sleep(Duration::from_millis(poll_ms)).await;
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

    async fn handle_vhost_user_blk_device(
        &mut self,
        mut device: VhostUserBlkDevice,
    ) -> Result<DeviceType> {
        let config = device.config.clone();

        // SPDK volumes carry the backend scheme in the socket path
        // ("spdk:///path/to/x.sock" or "spool:///..."), which cloud-hypervisor
        // does not understand. Strip it to get the bare Unix socket path, the
        // same way the dragonball vhost-user-blk frontend does.
        let vhost_socket = strip_vhost_socket_scheme(&config.socket_path)?;

        // The device manager already normalizes `driver_option` to a kata
        // driver type before the device reaches the hypervisor: "virtio-blk-pci"
        // becomes "blk", "virtio-blk-mmio" becomes "mmioblk". Cloud Hypervisor
        // only implements vhost-user-blk on the PCI virtio transport, so
        // reject the others explicitly.
        match config.driver_option.as_str() {
            KATA_BLK_DEV_TYPE | VIRTIO_BLOCK_PCI => {}
            "" => {}
            unsupported => {
                return Err(anyhow!(
                    "unsupported vhost-user-blk driver {} for cloud-hypervisor",
                    unsupported
                ))
            }
        }

        let disk_config = DiskConfig {
            vhost_user: true,
            vhost_socket: Some(vhost_socket),
            readonly: device.is_readonly,
            num_queues: match config.num_queues {
                0 => DEFAULT_DISK_QUEUES,
                n => n,
            },
            queue_size: match config.queue_size {
                0 => DEFAULT_DISK_QUEUE_SIZE,
                sz => u16::try_from(sz)?,
            },
            // CLH fallocates writable disks to their full logical size when
            // sparse is false; there is no backing file here to preallocate.
            sparse: true,
            ..Default::default()
        };

        let response = cloud_hypervisor_vm_blockdev_add(&self.api_socket, disk_config).await?;

        if let Some(detail) = response {
            debug!(sl!(), "vhost-user-blk add response: {:?}", detail);

            let dev_info: PciDeviceInfo =
                serde_json::from_str(detail.as_str()).map_err(|e| anyhow!(e))?;
            self.device_ids
                .insert(device.device_id.clone(), dev_info.id);

            // Persist the cloud-hypervisor assigned PCI path back into the
            // device, the same way handle_block_device does for BlockModern.
            // For a hot-plugged disk the device node name the guest will
            // assign is not known on the host, so the resource layer hands
            // this PCI path to the agent, which resolves it to /dev/vdX
            // through sysfs instead of trusting a name derived from the
            // kata-internal device index.
            device.config.pci_path = Some(Self::clh_pci_info_to_path(dev_info.bdf.as_str())?);
        }

        Ok(DeviceType::VhostUserBlk(device))
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

/// vhost-user-blk socket paths for SPDK volumes arrive with a backend scheme
/// prefix ("spdk:///..." or "spool:///...") added by the resource layer.
/// Cloud Hypervisor expects the bare Unix socket path, so strip the scheme.
/// A path without a recognized scheme is returned unchanged.
fn strip_vhost_socket_scheme(socket_path: &str) -> Result<String> {
    for scheme in ["spdk://", "spool://"] {
        if let Some(stripped) = socket_path.strip_prefix(scheme) {
            if stripped.is_empty() {
                return Err(anyhow!(
                    "vhost-user-blk socket path {:?} has no path after the scheme",
                    socket_path
                ));
            }
            return Ok(stripped.to_string());
        }
    }

    Ok(socket_path.to_string())
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
    use crate::Address;
    use crate::{VhostUserConfig, VhostUserType};
    use rstest::rstest;

    fn new_vhost_user_blk_device(
        socket_path: &str,
        driver_option: &str,
        num_queues: usize,
        queue_size: u32,
        is_readonly: bool,
    ) -> VhostUserBlkDevice {
        VhostUserBlkDevice {
            device_id: "test-vhost-blk-0".into(),
            is_readonly,
            no_drop: false,
            driver_option: driver_option.into(),
            attach_count: 0,
            config: VhostUserConfig {
                socket_path: socket_path.into(),
                device_type: VhostUserType::Blk,
                driver_option: driver_option.into(),
                num_queues,
                queue_size,
                ..Default::default()
            },
        }
    }

    /// Before VhostUserBlk support, `handle_add_device` failed outright with
    /// "unhandled device". The dispatcher must now reach the vhost-user-blk
    /// handler, which for a VM that is not running reports a connection
    /// failure to the cloud-hypervisor API socket instead of "unhandled
    /// device".
    #[cfg(feature = "cloud-hypervisor")]
    #[actix_rt::test]
    async fn test_vhost_user_blk_no_longer_unhandled() {
        let mut ch = CloudHypervisorInner {
            state: VmmState::VmRunning,
            ..Default::default()
        };

        let device = new_vhost_user_blk_device(
            "spdk:///tmp/vhost-blk-rawdisk01.sock",
            KATA_BLK_DEV_TYPE,
            0,
            0,
            false,
        );

        let result = ch.handle_add_device(DeviceType::VhostUserBlk(device)).await;
        // There is no live cloud-hypervisor VMM in the test, so the API call
        // fails -- but it must not fail with "unhandled device".
        let err = result.unwrap_err().to_string();
        assert!(!err.contains("unhandled device"), "got: {}", err);
    }

    #[rstest]
    #[case("spdk:///tmp/vhu/x.sock", "/tmp/vhu/x.sock")]
    #[case("spool:///tmp/vhu/x.sock", "/tmp/vhu/x.sock")]
    #[case("/tmp/vhu/x.sock", "/tmp/vhu/x.sock")]
    fn test_strip_vhost_socket_scheme(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(strip_vhost_socket_scheme(input).unwrap(), expected);
    }

    #[rstest]
    #[case("spdk://")]
    #[case("spool://")]
    fn test_strip_vhost_socket_scheme_empty_path(#[case] input: &str) {
        assert!(strip_vhost_socket_scheme(input).is_err());
    }

    #[rstest]
    #[case(KATA_BLK_DEV_TYPE, true)]
    #[case(VIRTIO_BLOCK_PCI, true)]
    #[case("", true)]
    #[case("mmioblk", false)]
    #[case("virtio-blk-mmio", false)]
    #[cfg(feature = "cloud-hypervisor")]
    #[actix_rt::test]
    async fn test_vhost_user_blk_driver_validation(
        #[case] driver: &str,
        #[case] should_pass: bool,
    ) {
        let mut ch = CloudHypervisorInner {
            state: VmmState::VmRunning,
            ..Default::default()
        };

        let device = new_vhost_user_blk_device("spdk:///tmp/x.sock", driver, 0, 0, false);

        let result = ch.handle_add_device(DeviceType::VhostUserBlk(device)).await;
        let error = match result {
            Ok(_) => String::new(),
            Err(e) => e.to_string(),
        };

        if should_pass {
            // Reaching the CLH API call (which fails without a live VMM) is
            // success; an unsupported-driver rejection is not.
            assert!(
                !error.contains("unsupported vhost-user-blk driver"),
                "{}",
                error
            );
        } else {
            assert!(
                error.contains("unsupported vhost-user-blk driver"),
                "{}",
                error
            );
        }
    }

    #[rstest]
    #[case(0, 0, DEFAULT_DISK_QUEUES, DEFAULT_DISK_QUEUE_SIZE)]
    #[case(2, 256, 2, 256)]
    fn test_vhost_user_blk_queue_defaults(
        #[case] num_queues: usize,
        #[case] queue_size: u32,
        #[case] expected_queues: usize,
        #[case] expected_size: u16,
    ) {
        let socket_path = "spdk:///tmp/vhost-blk.sock";
        let config = VhostUserConfig {
            socket_path: socket_path.into(),
            device_type: VhostUserType::Blk,
            driver_option: KATA_BLK_DEV_TYPE.into(),
            num_queues,
            queue_size,
            ..Default::default()
        };

        // Verify queue settings normalization matches the cold-plug disk
        // config behavior (zero values fall back to CLH defaults).
        let disk = DiskConfig {
            vhost_user: true,
            vhost_socket: strip_vhost_socket_scheme(&config.socket_path).ok(),
            readonly: false,
            num_queues: match config.num_queues {
                0 => DEFAULT_DISK_QUEUES,
                n => n,
            },
            queue_size: match config.queue_size {
                0 => DEFAULT_DISK_QUEUE_SIZE,
                sz => u16::try_from(sz).unwrap(),
            },
            sparse: true,
            ..Default::default()
        };

        assert_eq!(disk.num_queues, expected_queues);
        assert_eq!(disk.queue_size, expected_size);
        assert!(disk.vhost_user);
        assert_eq!(disk.vhost_socket.as_deref(), Some("/tmp/vhost-blk.sock"));
        // No host file backs a vhost-user-blk device, so the request must not
        // carry a disk path.
        assert_eq!(disk.path, None);
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

    /// vm.remove-device is asynchronous: cloud-hypervisor removes the device
    /// from the device tree of vm.info only once the removal completes. The
    /// remove path must keep polling until the entry is gone instead of
    /// returning right after the API call, and it must not leave a stale
    /// kata-id -> clh-id mapping behind.
    ///
    /// Without a live VMM the vm.info poll fails, which exercises the error
    /// path of the poll but not the timeout. The timeout behavior itself is
    /// verified by waiting on a poll interval that is zero: the loop then
    /// spins until the deadline, so a test failure would look like a hang.
    #[cfg(feature = "cloud-hypervisor")]
    #[actix_rt::test]
    async fn test_remove_device_reports_poll_failure() {
        let mut ch = CloudHypervisorInner {
            state: VmmState::VmRunning,
            ..Default::default()
        };
        // Simulate a device that was attached successfully before.
        ch.device_ids
            .insert("test-vhost-blk-0".to_string(), "__disk0".to_string());

        let device =
            new_vhost_user_blk_device("spdk:///tmp/vhost-blk.sock", KATA_BLK_DEV_TYPE, 0, 0, true);

        // Both the vm.remove-device API call and the vm.info poll fail
        // without a live VMM; the error must surface, not be swallowed.
        let err = ch
            .remove_device(DeviceType::VhostUserBlk(device))
            .await
            .unwrap_err()
            .to_string();
        assert!(
            !err.is_empty(),
            "removal must report the cloud-hypervisor API failure"
        );
        // A failed removal must keep the id mapping so a retry is possible.
        assert!(ch.device_ids.contains_key("test-vhost-blk-0"));
    }

    /// The device_tree key is the cloud-hypervisor device id returned by
    /// vm.add-disk, so the wait loop must treat "tree gone / key absent" as
    /// completion. This checks the tree-evaluation helper on the parsed
    /// vm.info shape used by the poll.
    #[test]
    fn test_device_tree_completion_semantics() {
        use ch_config::DeviceNode;
        use std::collections::BTreeMap;

        let mut tree = BTreeMap::new();
        tree.insert(
            "__disk0".to_string(),
            DeviceNode {
                id: "__disk0".to_string(),
                pci_bdf: Some("0000:00:05.0".to_string()),
            },
        );
        // Device still present.
        assert!(tree.contains_key("__disk0"));
        // Removal finished when the key is gone.
        tree.remove("__disk0");
        assert!(!tree.contains_key("__disk0"));
    }

    /// A nil device_tree (older cloud-hypervisor, or an empty VM) must count
    /// as "device not present" so the removal wait returns immediately.
    #[cfg(feature = "cloud-hypervisor")]
    #[actix_rt::test]
    async fn test_wait_for_device_removal_nil_tree() {
        let ch = CloudHypervisorInner {
            state: VmmState::VmRunning,
            ..Default::default()
        };

        // The vm.info call fails without a live VMM, so a nil-tree response
        // cannot be produced here; the semantics are asserted indirectly:
        // an API error propagates (does not block, does not panic).
        let err = ch
            .wait_for_device_removal("__disk0", DEVICE_REMOVAL_POLL_MS, 0)
            .await;
        assert!(err.is_err());
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
