// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryFrom;
use std::path::PathBuf;

use super::{build_dragonball_network_config, DragonballInner};
use crate::device::pci_path::PciPath;
use crate::VhostUserConfig;
use crate::{
    device::DeviceType, HybridVsockConfig, NetworkConfig, ShareFsConfig, ShareFsMountConfig,
    ShareFsMountOperation, ShareFsMountType, VfioDevice, VmmState, JAILER_ROOT,
};
use anyhow::{anyhow, Context, Result};
use dbs_utils::net::MacAddr;
use dragonball::api::v1::VhostUserConfig as DragonballVhostUserConfig;
use dragonball::api::v1::{
    BlockDeviceConfigInfo, FsDeviceConfigInfo, FsMountConfigInfo, NetworkInterfaceConfig,
    VsockDeviceConfigInfo,
};
use dragonball::device_manager::{
    blk_dev_mgr::BlockDeviceType,
    vfio_dev_mgr::{HostDeviceConfig, VfioPciDeviceConfig},
};

const MB_TO_B: u32 = 1024 * 1024;
const DEFAULT_VIRTIO_FS_NUM_QUEUES: i32 = 1;
const DEFAULT_VIRTIO_FS_QUEUE_SIZE: i32 = 1024;

const VIRTIO_FS: &str = "virtio-fs";
const INLINE_VIRTIO_FS: &str = "inline-virtio-fs";

pub(crate) fn drive_index_to_id(index: u64) -> String {
    format!("drive_{}", index)
}

impl DragonballInner {
    pub(crate) async fn add_device(&mut self, device: DeviceType) -> Result<DeviceType> {
        if self.state == VmmState::NotReady {
            info!(sl!(), "VMM not ready, queueing device {}", device);

            // add the pending device by reverse order, thus the
            // start_vm would pop the devices in an right order
            // to add the devices.
            self.pending_devices.insert(0, device.clone());
            return Ok(device);
        }

        info!(sl!(), "dragonball add device {:?}", &device);
        match device {
            DeviceType::Network(network) => {
                self.add_net_device(&network.config)
                    .context("add net device")?;
                Ok(DeviceType::Network(network))
            }
            DeviceType::Vfio(mut hostdev) => {
                self.add_vfio_device(&mut hostdev)
                    .context("add vfio device")?;

                Ok(DeviceType::Vfio(hostdev))
            }
            DeviceType::Block(block) => {
                self.add_block_device(
                    block.config.path_on_host.as_str(),
                    block.device_id.as_str(),
                    block.config.is_readonly,
                    block.config.no_drop,
                )
                .context("add block device")?;
                Ok(DeviceType::Block(block))
            }
            DeviceType::VhostUserBlk(block) => {
                self.add_block_device(
                    block.config.socket_path.as_str(),
                    block.device_id.as_str(),
                    block.is_readonly,
                    block.no_drop,
                )
                .context("add vhost user based block device")?;
                Ok(DeviceType::VhostUserBlk(block))
            }
            DeviceType::HybridVsock(hvsock) => {
                self.add_hvsock(&hvsock.config).context("add vsock")?;
                Ok(DeviceType::HybridVsock(hvsock))
            }
            DeviceType::ShareFs(sharefs) => {
                self.add_share_fs_device(&sharefs.config)
                    .context("add share fs device")?;
                Ok(DeviceType::ShareFs(sharefs))
            }
            DeviceType::VhostUserNetwork(dev) => {
                self.add_vhost_user_net_device(&dev.config)
                    .context("add vhost-user-net device")?;
                Ok(DeviceType::VhostUserNetwork(dev))
            }
            DeviceType::Vsock(_) => todo!(),
        }
    }

    pub(crate) async fn remove_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl!(), "remove device {} ", device);

        match device {
            DeviceType::Network(network) => {
                // Dragonball doesn't support remove network device, just print message.
                info!(
                    sl!(),
                    "dragonball remove network device: {:?}.", network.config
                );

                Ok(())
            }
            DeviceType::Block(block) => {
                let drive_id = drive_index_to_id(block.config.index);
                self.remove_block_drive(drive_id.as_str())
                    .context("remove block drive")
            }
            DeviceType::Vfio(hostdev) => {
                let primary_device = hostdev.devices.first().unwrap().clone();
                let hostdev_id = primary_device.hostdev_id;

                self.remove_vfio_device(hostdev_id)
            }
            _ => Err(anyhow!("unsupported device {:?}", device)),
        }
    }

    pub(crate) async fn update_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl!(), "dragonball update device {:?}", &device);
        match device {
            DeviceType::ShareFs(sharefs_mount) => {
                // It's safe to unwrap mount config as mount_config is always there.
                self.add_share_fs_mount(&sharefs_mount.config.mount_config.unwrap())
                    .context("update share-fs device with mount operation.")
            }
            _ => Err(anyhow!("unsupported device {:?} to update.", device)),
        }
    }

    fn add_vfio_device(&mut self, device: &mut VfioDevice) -> Result<()> {
        // FIXME:
        // A device with multi-funtions, or a IOMMU group with one more
        // devices, the Primary device is selected to be passed to VM.
        // And the the first one is Primary device.
        // safe here, devices is not empty.
        let primary_device = device.devices.first_mut().unwrap();
        let vendor_device_id = if let Some(vdc) = primary_device.device_vendor_class.as_ref() {
            vdc.get_device_vendor_id()?
        } else {
            0
        };

        info!(
            sl!(),
            "insert host device. 
            host device id: {:?}, 
            bus_slot_func: {:?}, 
            vendor/device id: {:?}",
            primary_device.hostdev_id,
            primary_device.bus_slot_func,
            vendor_device_id,
        );

        let vfio_dev_config = VfioPciDeviceConfig {
            bus_slot_func: primary_device.bus_slot_func.clone(),
            vendor_device_id,
            ..Default::default()
        };
        let host_dev_config = HostDeviceConfig {
            hostdev_id: primary_device.hostdev_id.clone(),
            sysfs_path: primary_device.sysfs_path.clone(),
            dev_config: vfio_dev_config,
        };

        let guest_device_id = self
            .vmm_instance
            .insert_host_device(host_dev_config)
            .context("insert host device failed")?;

        // It's safe to unwrap guest_device_id as we can get a guest device id here.
        primary_device.guest_pci_path = Some(PciPath::try_from(guest_device_id.unwrap() as u32)?);

        Ok(())
    }

    fn remove_vfio_device(&mut self, hostdev_id: String) -> Result<()> {
        info!(sl!(), "remove host_device with hostdev id {:?}", hostdev_id);

        self.vmm_instance
            .prepare_remove_host_device(&hostdev_id)
            .context("prepare to remove host device failed")?;
        self.vmm_instance
            .remove_host_device(&hostdev_id)
            .context("remove host device failed")?;

        Ok(())
    }

    fn add_block_device(
        &mut self,
        path: &str,
        id: &str,
        read_only: bool,
        no_drop: bool,
    ) -> Result<()> {
        let jailed_drive = self.get_resource(path, id).context("get resource")?;
        self.cached_block_devices.insert(id.to_string());

        let blk_cfg = BlockDeviceConfigInfo {
            drive_id: id.to_string(),
            device_type: BlockDeviceType::get_type(path),
            path_on_host: PathBuf::from(jailed_drive),
            is_direct: self.config.blockdev_info.block_device_cache_direct,
            no_drop,
            is_read_only: read_only,
            ..Default::default()
        };
        self.vmm_instance
            .insert_block_device(blk_cfg)
            .context("insert block device")
    }

    fn remove_block_drive(&mut self, id: &str) -> Result<()> {
        self.vmm_instance
            .remove_block_device(id)
            .context("remove block device")?;

        if self.cached_block_devices.contains(id) && self.jailed {
            self.umount_jail_resource(id)
                .context("umount jail resource")?;
            self.cached_block_devices.remove(id);
        }
        Ok(())
    }

    fn add_net_device(&mut self, config: &NetworkConfig) -> Result<()> {
        let net_cfg = build_dragonball_network_config(&self.config, config);
        self.vmm_instance
            .insert_network_device(net_cfg)
            .context("insert network device")
    }

    /// Add vhost-user-net deivce to Dragonball
    fn add_vhost_user_net_device(&mut self, config: &VhostUserConfig) -> Result<()> {
        let guest_mac = MacAddr::parse_str(&config.mac_address).ok();
        let net_cfg = NetworkInterfaceConfig {
            num_queues: Some(config.num_queues),
            queue_size: Some(config.queue_size as u16),
            backend: dragonball::api::v1::Backend::VhostUser(DragonballVhostUserConfig {
                sock_path: config.socket_path.clone(),
            }),
            guest_mac,
            use_shared_irq: None,
            use_generic_irq: None,
        };

        self.vmm_instance
            .insert_network_device(net_cfg)
            .context("insert vhost-user-net device")
    }

    fn add_hvsock(&mut self, config: &HybridVsockConfig) -> Result<()> {
        let vsock_cfg = VsockDeviceConfigInfo {
            id: String::from(JAILER_ROOT),
            guest_cid: config.guest_cid,
            uds_path: Some(config.uds_path.clone()),
            ..Default::default()
        };
        debug!(sl!(), "HybridVsock configure: {:?}", &vsock_cfg);

        self.vmm_instance
            .insert_vsock(vsock_cfg)
            .context("insert vsock")
    }

    fn parse_inline_virtiofs_args(
        &self,
        fs_cfg: &mut FsDeviceConfigInfo,
        options: &mut Vec<String>,
    ) -> Result<()> {
        let mut debug = false;
        let mut opt_list = String::new();

        fs_cfg.mode = String::from("virtio");
        fs_cfg.cache_policy = self.config.shared_fs.virtio_fs_cache.clone();
        fs_cfg.fuse_killpriv_v2 = true;

        info!(
            sl!(),
            "args: {:?}", &self.config.shared_fs.virtio_fs_extra_args
        );
        let mut args = self.config.shared_fs.virtio_fs_extra_args.clone();
        let _ = go_flag::parse_args_with_warnings::<String, _, _>(&args, None, |flags| {
            flags.add_flag("d", &mut debug);
            flags.add_flag("thread-pool-size", &mut fs_cfg.thread_pool_size);
            flags.add_flag("drop-sys-resource", &mut fs_cfg.drop_sys_resource);
            flags.add_flag("o", &mut opt_list);
        })
        .with_context(|| format!("parse args: {:?}", args))?;

        // more options parsed for inline virtio-fs' custom config
        args.append(options);

        if debug {
            warn!(
                sl!(),
                "Inline virtiofs \"-d\" option not implemented, ignore"
            );
        }

        // Parse comma separated option list
        if !opt_list.is_empty() {
            let args: Vec<&str> = opt_list.split(',').collect();
            for arg in args {
                match arg {
                    "cache=none" => fs_cfg.cache_policy = String::from("none"),
                    "cache=auto" => fs_cfg.cache_policy = String::from("auto"),
                    "cache=always" => fs_cfg.cache_policy = String::from("always"),
                    "no_open" => fs_cfg.no_open = true,
                    "open" => fs_cfg.no_open = false,
                    "writeback_cache" => fs_cfg.writeback_cache = true,
                    "no_writeback_cache" => fs_cfg.writeback_cache = false,
                    "writeback" => fs_cfg.writeback_cache = true,
                    "no_writeback" => fs_cfg.writeback_cache = false,
                    "xattr" => fs_cfg.xattr = true,
                    "no_xattr" => fs_cfg.xattr = false,
                    "cache_symlinks" => {} // inline virtiofs always cache symlinks
                    "no_readdir" => fs_cfg.no_readdir = true,
                    "trace" => warn!(
                        sl!(),
                        "Inline virtiofs \"-o trace\" option not supported yet, ignored."
                    ),
                    _ => warn!(sl!(), "Inline virtiofs unsupported option: {}", arg),
                }
            }
        }

        debug!(sl!(), "Inline virtiofs config {:?}", fs_cfg);
        Ok(())
    }

    fn add_share_fs_device(&self, config: &ShareFsConfig) -> Result<()> {
        let mut fs_cfg = FsDeviceConfigInfo {
            sock_path: config.sock_path.clone(),
            tag: config.mount_tag.clone(),
            num_queues: if config.queue_num > 0 {
                config.queue_num as usize
            } else {
                DEFAULT_VIRTIO_FS_NUM_QUEUES as usize
            },
            queue_size: if config.queue_size > 0 {
                config.queue_size as u16
            } else {
                DEFAULT_VIRTIO_FS_QUEUE_SIZE as u16
            },
            cache_size: (self.config.shared_fs.virtio_fs_cache_size as u64)
                .saturating_mul(MB_TO_B as u64),
            xattr: true,
            ..Default::default()
        };

        let mut options = config.options.clone();
        self.do_add_fs_device(&config.fs_type, &mut fs_cfg, &mut options)
    }

    fn do_add_fs_device(
        &self,
        fs_type: &str,
        fs_cfg: &mut FsDeviceConfigInfo,
        options: &mut Vec<String>,
    ) -> Result<()> {
        match fs_type {
            VIRTIO_FS => {
                fs_cfg.mode = String::from("vhostuser");
            }
            INLINE_VIRTIO_FS => {
                // All parameters starting with --patch-fs do not need to be processed, these are the parameters required by patch fs
                options.retain(|x| !x.starts_with("--patch-fs"));
                self.parse_inline_virtiofs_args(fs_cfg, options)?;
            }
            _ => {
                return Err(anyhow!(
                    "hypervisor isn't configured with shared_fs supported"
                ));
            }
        }
        self.vmm_instance
            .insert_fs(fs_cfg)
            .map_err(|e| anyhow!("insert {} fs error. {:?}", fs_cfg.mode, e))
    }

    fn add_share_fs_mount(&mut self, config: &ShareFsMountConfig) -> Result<()> {
        let ops = match config.op {
            ShareFsMountOperation::Mount => "mount",
            ShareFsMountOperation::Umount => "umount",
            ShareFsMountOperation::Update => "update",
        };

        let fstype = match config.fstype {
            ShareFsMountType::PASSTHROUGH => "passthroughfs",
            ShareFsMountType::RAFS => "rafs",
        };

        let cfg = FsMountConfigInfo {
            ops: ops.to_string(),
            fstype: Some(fstype.to_string()),
            source: Some(config.source.clone()),
            mountpoint: config.mount_point.clone(),
            config: config.config.clone(),
            tag: config.tag.clone(),
            prefetch_list_path: config.prefetch_list_path.clone(),
            dax_threshold_size_kb: None,
        };

        self.vmm_instance.patch_fs(&cfg, config.op).map_err(|e| {
            anyhow!(
                "{:?} {} at {} error: {:?}",
                config.op,
                fstype,
                config.mount_point.clone(),
                e
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use dragonball::api::v1::FsDeviceConfigInfo;
    use tokio::sync::mpsc;

    use crate::dragonball::DragonballInner;

    #[test]
    fn test_parse_inline_virtiofs_args() {
        let (tx, _) = mpsc::channel(1);
        let mut dragonball = DragonballInner::new(tx);
        let mut fs_cfg = FsDeviceConfigInfo::default();

        // no_open and writeback_cache is the default, so test open and no_writeback_cache. "-d"
        // and "trace" are ignored for now, but should not return error.
        dragonball.config.shared_fs.virtio_fs_extra_args = vec![
            "-o".to_string(),
            "open,no_writeback_cache,xattr,trace".to_string(),
            "--thread-pool-size=128".to_string(),
            "--drop-sys-resource".to_string(),
            "-d".to_string(),
        ];

        let mut options: Vec<String> = Vec::new();
        dragonball.config.shared_fs.virtio_fs_cache = "auto".to_string();
        dragonball
            .parse_inline_virtiofs_args(&mut fs_cfg, &mut options)
            .unwrap();

        assert!(!fs_cfg.no_open);
        assert!(fs_cfg.xattr);
        assert!(fs_cfg.fuse_killpriv_v2);
        assert!(!fs_cfg.writeback_cache);
        assert_eq!(fs_cfg.cache_policy, "auto".to_string());
        assert!(fs_cfg.drop_sys_resource);
        assert!(fs_cfg.thread_pool_size == 128);
    }
}
