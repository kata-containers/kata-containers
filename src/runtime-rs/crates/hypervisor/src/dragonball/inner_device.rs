// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use dbs_utils::net::MacAddr;
use dragonball::api::v1::{
    BlockDeviceConfigInfo, FsDeviceConfigInfo, FsMountConfigInfo, VirtioNetDeviceConfigInfo,
    VsockDeviceConfigInfo,
};

use super::DragonballInner;
use crate::{
    device::Device, NetworkConfig, ShareFsDeviceConfig, ShareFsMountConfig, ShareFsMountType,
    ShareFsOperation, VmmState, VsockConfig,
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
    pub(crate) async fn add_device(&mut self, device: Device) -> Result<()> {
        if self.state == VmmState::NotReady {
            info!(sl!(), "VMM not ready, queueing device {}", device);

            // add the pending device by reverse order, thus the
            // start_vm would pop the devices in an right order
            // to add the devices.
            self.pending_devices.insert(0, device);
            return Ok(());
        }

        info!(sl!(), "dragonball add device {:?}", &device);
        match device {
            Device::Network(config) => self.add_net_device(&config).context("add net device"),
            Device::Vfio(_config) => {
                todo!()
            }
            Device::Block(config) => self
                .add_block_device(
                    config.path_on_host.as_str(),
                    config.id.as_str(),
                    config.is_readonly,
                    config.no_drop,
                )
                .context("add block device"),
            Device::Vsock(config) => self.add_vsock(&config).context("add vsock"),
            Device::ShareFsDevice(config) => self
                .add_share_fs_device(&config)
                .context("add share fs device"),
            Device::ShareFsMount(config) => self
                .add_share_fs_mount(&config)
                .context("add share fs mount"),
        }
    }

    pub(crate) async fn remove_device(&mut self, device: Device) -> Result<()> {
        info!(sl!(), "remove device {} ", device);

        match device {
            Device::Block(config) => {
                let drive_id = drive_index_to_id(config.index);
                self.remove_block_drive(drive_id.as_str())
                    .context("remove block drive")
            }
            Device::Vfio(_config) => {
                todo!()
            }
            _ => Err(anyhow!("unsupported device {:?}", device)),
        }
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
        let iface_cfg = VirtioNetDeviceConfigInfo {
            iface_id: config.id.clone(),
            host_dev_name: config.host_dev_name.clone(),
            guest_mac: match &config.guest_mac {
                Some(mac) => MacAddr::from_bytes(&mac.0).ok(),
                None => None,
            },
            ..Default::default()
        };

        info!(
            sl!(),
            "add {} endpoint to {}", iface_cfg.host_dev_name, iface_cfg.iface_id
        );

        self.vmm_instance
            .insert_network_device(iface_cfg)
            .context("insert network device")
    }

    fn add_vsock(&mut self, config: &VsockConfig) -> Result<()> {
        let vsock_cfg = VsockDeviceConfigInfo {
            id: String::from("root"),
            guest_cid: config.guest_cid,
            uds_path: Some(config.uds_path.clone()),
            ..Default::default()
        };

        self.vmm_instance
            .insert_vsock(vsock_cfg)
            .context("insert vsock")
    }

    fn parse_inline_virtiofs_args(&self, fs_cfg: &mut FsDeviceConfigInfo) -> Result<()> {
        let mut debug = false;
        let mut opt_list = String::new();

        fs_cfg.mode = String::from("virtio");
        fs_cfg.cache_policy = self.config.shared_fs.virtio_fs_cache.clone();
        fs_cfg.fuse_killpriv_v2 = true;

        info!(
            sl!(),
            "args: {:?}", &self.config.shared_fs.virtio_fs_extra_args
        );
        let args = &self.config.shared_fs.virtio_fs_extra_args;
        let _ = go_flag::parse_args_with_warnings::<String, _, _>(args, None, |flags| {
            flags.add_flag("d", &mut debug);
            flags.add_flag("thread-pool-size", &mut fs_cfg.thread_pool_size);
            flags.add_flag("drop-sys-resource", &mut fs_cfg.drop_sys_resource);
            flags.add_flag("o", &mut opt_list);
        })
        .with_context(|| format!("parse args: {:?}", args))?;

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
                    "no_open" => fs_cfg.no_open = true,
                    "open" => fs_cfg.no_open = false,
                    "writeback_cache" => fs_cfg.writeback_cache = true,
                    "no_writeback_cache" => fs_cfg.writeback_cache = false,
                    "writeback" => fs_cfg.writeback_cache = true,
                    "no_writeback" => fs_cfg.writeback_cache = false,
                    "xattr" => fs_cfg.xattr = true,
                    "no_xattr" => fs_cfg.xattr = false,
                    "cache_symlinks" => {} // inline virtiofs always cache symlinks
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

    fn add_share_fs_device(&self, config: &ShareFsDeviceConfig) -> Result<()> {
        let mut fs_cfg = FsDeviceConfigInfo {
            sock_path: config.sock_path.clone(),
            tag: config.mount_tag.clone(),
            num_queues: if config.queue_num > 0 {
                config.queue_size as usize
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
            ..Default::default()
        };
        self.do_add_fs_device(&config.fs_type, &mut fs_cfg)
    }

    fn do_add_fs_device(&self, fs_type: &str, fs_cfg: &mut FsDeviceConfigInfo) -> Result<()> {
        match fs_type {
            VIRTIO_FS => {
                fs_cfg.mode = String::from("vhostuser");
            }
            INLINE_VIRTIO_FS => {
                self.parse_inline_virtiofs_args(fs_cfg)?;
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
            ShareFsOperation::Mount => "mount",
            ShareFsOperation::Umount => "umount",
            ShareFsOperation::Update => "update",
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
            config: None,
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

    use crate::dragonball::DragonballInner;

    #[test]
    fn test_parse_inline_virtiofs_args() {
        let mut dragonball = DragonballInner::new();
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
        dragonball.config.shared_fs.virtio_fs_cache = "auto".to_string();
        dragonball.parse_inline_virtiofs_args(&mut fs_cfg).unwrap();

        assert!(!fs_cfg.no_open);
        assert!(fs_cfg.xattr);
        assert!(fs_cfg.fuse_killpriv_v2);
        assert!(!fs_cfg.writeback_cache);
        assert_eq!(fs_cfg.cache_policy, "auto".to_string());
        assert!(fs_cfg.drop_sys_resource);
        assert!(fs_cfg.thread_pool_size == 128);
    }
}
