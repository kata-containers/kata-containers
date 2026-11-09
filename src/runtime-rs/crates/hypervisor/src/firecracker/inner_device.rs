//Copyright (c) 2019-2022 Alibaba Cloud
//Copyright (c) 2019-2022 Ant Group
//Copyright (c) 2023 Nubificus Ltd
//
//SPDX-License-Identifier: Apache-2.0

use super::FcInner;
use crate::firecracker::{
    inner_hypervisor::{FC_AGENT_SOCKET_NAME, ROOT},
    sl,
};
use crate::VmmState;
use crate::{device::DeviceType, HybridVsockConfig, VsockConfig};
use anyhow::{anyhow, Context, Result};
use serde_json::json;

use super::block_source::{cleanup_vmdk, prepare_vmdk, VMDK_DESCRIPTOR, VMDK_PLACEHOLDER};
use super::fc_api::DISK_POOL_SIZE;
use crate::device::util::get_virt_drive_name;
use crate::virtio_blk_modern::{BlockConfigModern, BlockSourceFormat};
use std::path::{Path, PathBuf};

impl FcInner {
    pub(crate) async fn add_device(&mut self, device: DeviceType) -> Result<()> {
        if self.state == VmmState::NotReady {
            info!(sl(), "VMM not ready, queueing device {}", device);

            self.pending_devices.insert(0, device);

            return Ok(());
        }

        debug!(sl(), "Add Device {} ", &device);

        match device {
            DeviceType::BlockModern(block_mod) => {
                let block = block_mod.lock().await.clone();
                if let BlockSourceFormat::Vmdk(vmdk) = &block.config.source {
                    let config = &block.config;
                    let guest_path = self.vmdk_guest_path(config)?;
                    let directory = self.vmdk_directory(config.index);
                    prepare_vmdk(vmdk, &directory)?;
                    let path = if self.jailed {
                        format!("/vmdk-{}/{VMDK_DESCRIPTOR}", config.index)
                    } else {
                        directory.join(VMDK_DESCRIPTOR).display().to_string()
                    };
                    if let Err(error) = self
                        .patch_drive(&format!("vmdk{}", config.index), &path)
                        .await
                    {
                        cleanup_vmdk(&directory)?;
                        return Err(error).context("patch VMDK drive");
                    }
                    block_mod.lock().await.config.virt_path = guest_path;
                    return Ok(());
                }
                self.hotplug_block_device(block.config.path_on_host.as_str(), block.config.index)
                    .await
                    .context("add block device")
            }
            DeviceType::Network(network) => self
                .add_net_device(&network.config, network.device_id)
                .await
                .context("add net device"),
            DeviceType::HybridVsock(hvsock) => {
                self.add_hvsock(&hvsock.config).await.context("add vsock")
            }
            DeviceType::Vsock(vsock) => self.add_vsock(&vsock.config).await.context("add vsock"),
            _ => Err(anyhow!("unhandled device: {:?}", device)),
        }
    }

    // Since Firecracker doesn't support sharefs, we patch block devices on pre-start inserted
    // dummy drives
    pub(crate) async fn hotplug_block_device(&mut self, path: &str, id: u64) -> Result<()> {
        if id > 0 {
            self.patch_container_rootfs(&id.to_string(), path).await?;
        }
        Ok(())
    }

    pub(crate) async fn remove_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl(), "Remove Device {} ", device);
        if !self.config.blockdev_info.enable_vmdk {
            return Ok(());
        }
        if let DeviceType::BlockModern(block) = device {
            let config = block.lock().await.config.clone();
            let (name, path) = if matches!(config.source, BlockSourceFormat::Vmdk(_)) {
                let path = if self.jailed {
                    format!("/{VMDK_PLACEHOLDER}/{VMDK_DESCRIPTOR}")
                } else {
                    Path::new(&self.vm_path)
                        .join(ROOT)
                        .join(VMDK_PLACEHOLDER)
                        .join(VMDK_DESCRIPTOR)
                        .display()
                        .to_string()
                };
                (format!("vmdk{}", config.index), path)
            } else if config.index > 0 {
                let name = format!("drive{}", config.index);
                if self.jailed {
                    match self.umount_jail_resource(&name) {
                        Ok(()) => (),
                        Err(error) if error.downcast_ref() == Some(&nix::errno::Errno::EINVAL) => {}
                        Err(error) => return Err(error),
                    }
                }
                let placeholder = if self.jailed {
                    format!("/{name}")
                } else {
                    Path::new(&self.vm_path)
                        .join(ROOT)
                        .join(&name)
                        .display()
                        .to_string()
                };
                (name, placeholder)
            } else {
                return Ok(());
            };
            if !self.vmm_exited()? {
                if let Err(error) = self.patch_drive(&name, &path).await {
                    if !self.vmm_exited()? {
                        return Err(error);
                    }
                }
            }
            if matches!(config.source, BlockSourceFormat::Vmdk(_)) {
                cleanup_vmdk(&self.vmdk_directory(config.index))?;
            }
        }
        Ok(())
    }

    fn vmm_exited(&mut self) -> Result<bool> {
        if let Some(child) = self.fc_process.get_mut().as_mut() {
            return child
                .try_wait()
                .map(|status| status.is_some())
                .context("check Firecracker exit status");
        }
        Ok(self.pid.is_some_and(|pid| {
            matches!(
                nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None),
                Err(nix::errno::Errno::ESRCH)
            )
        }))
    }

    pub(super) fn vmdk_directory(&self, index: u64) -> PathBuf {
        Path::new(&self.vm_path)
            .join(ROOT)
            .join(format!("vmdk-{index}"))
    }

    fn vmdk_guest_path(&self, config: &BlockConfigModern) -> Result<String> {
        if !self.config.blockdev_info.enable_vmdk {
            return Err(anyhow!("Firecracker VMDK requires enable_vmdk = true"));
        }
        if !config.is_readonly {
            return Err(anyhow!(
                "Firecracker VMDK requires a read-only block device"
            ));
        }
        if config.index == 0 || config.index >= u64::from(DISK_POOL_SIZE) {
            return Err(anyhow!("Firecracker VMDK index exceeds the drive pool"));
        }
        let name = get_virt_drive_name((u64::from(DISK_POOL_SIZE) - 1 + config.index) as i32)?;
        Ok(format!("/dev/{name}"))
    }

    pub(crate) async fn update_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl(), "update device {:?}", &device);
        Ok(())
    }

    pub(crate) async fn add_hvsock(&mut self, config: &HybridVsockConfig) -> Result<()> {
        let rel_uds_path = match self.jailed {
            false => [self.vm_path.as_str(), FC_AGENT_SOCKET_NAME].join("/"),
            true => FC_AGENT_SOCKET_NAME.to_string(),
        };
        let body_vsock: String = json!({
            "vsock_id": String::from(ROOT),
            "guest_cid": config.guest_cid,
            "uds_path": rel_uds_path,
        })
        .to_string();

        info!(sl(), "HybridVsock configure: {:?}", &body_vsock);

        self.request_with_retry(hyper::Method::PUT, "/vsock", body_vsock)
            .await?;
        Ok(())
    }

    pub(crate) async fn add_vsock(&mut self, config: &VsockConfig) -> Result<()> {
        let rel_uds_path = match self.jailed {
            false => [self.vm_path.as_str(), FC_AGENT_SOCKET_NAME].join("/"),
            true => FC_AGENT_SOCKET_NAME.to_string(),
        };
        let body_vsock: String = json!({
            "vsock_id": String::from(ROOT),
            "guest_cid": config.guest_cid,
            "uds_path": rel_uds_path,
        })
        .to_string();

        info!(sl(), "HybridVsock configure: {:?}", &body_vsock);

        self.request_with_retry(hyper::Method::PUT, "/vsock", body_vsock)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtio_blk_modern::BlockDeviceModernHandle;
    use std::fs;
    use tokio::process::Command;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn detach_retries_unmounted_slots_and_cleans_up_exited_vmm() {
        if !nix::unistd::geteuid().is_root() {
            return;
        }
        for vmdk in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let (sender, _) = mpsc::channel(1);
            let mut fc = FcInner::new(sender);
            assert!(!fc.vmm_exited().unwrap());
            fc.vm_path = temp.path().display().to_string();
            fc.asock_path = temp.path().join("fc.sock").display().to_string();
            fc.jailed = true;
            fc.config.blockdev_info.enable_vmdk = true;
            fs::create_dir(temp.path().join(ROOT)).unwrap();
            let source = temp.path().join("source");
            fs::write(&source, [1_u8; 512]).unwrap();
            let mut config = BlockConfigModern {
                index: 1,
                ..Default::default()
            };
            if vmdk {
                let mut layout = kata_types::vmdk::VmdkConfig::default();
                layout.push_extent(source.to_str().unwrap(), 1, 0);
                prepare_vmdk(&layout, &fc.vmdk_directory(1)).unwrap();
                config.source = BlockSourceFormat::Vmdk(layout);
            } else {
                let slot = temp.path().join(ROOT).join("drive1");
                fs::write(&slot, [0_u8; 512]).unwrap();
                nix::mount::mount(
                    Some(&source),
                    &slot,
                    None::<&str>,
                    nix::mount::MsFlags::MS_BIND,
                    None::<&str>,
                )
                .unwrap();
            }
            let block = BlockDeviceModernHandle::new("test".into(), config);
            let device = DeviceType::BlockModern(block.arc());
            let child = Command::new("sleep")
                .arg("60")
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            fc.pid = child.id();
            *fc.fc_process.get_mut() = Some(child);

            let error = fc
                .remove_device(device.clone())
                .await
                .unwrap_err()
                .to_string();
            if vmdk {
                assert!(fc.vmdk_directory(1).join("extent-0").exists());
            } else {
                assert_eq!(
                    fs::read(temp.path().join(ROOT).join("drive1")).unwrap(),
                    [0_u8; 512]
                );
                assert_eq!(
                    fc.remove_device(device.clone())
                        .await
                        .unwrap_err()
                        .to_string(),
                    error
                );
            }
            fc.fc_process
                .get_mut()
                .as_mut()
                .unwrap()
                .kill()
                .await
                .unwrap();
            fc.remove_device(device.clone()).await.unwrap();
            assert!(!fc.vmdk_directory(1).exists());
            fc.wait_vm().await.unwrap();
            fc.remove_device(device).await.unwrap();
        }
    }

    #[test]
    fn vmdk_slots_require_opt_in_readonly_and_valid_index() {
        let (sender, _) = mpsc::channel(1);
        let mut fc = FcInner::new(sender);
        let mut config = BlockConfigModern {
            index: 1,
            is_readonly: true,
            ..Default::default()
        };
        assert!(fc.vmdk_guest_path(&config).is_err());
        fc.config.blockdev_info.enable_vmdk = true;
        assert_eq!(fc.vmdk_guest_path(&config).unwrap(), "/dev/vdg");
        config.index = u64::from(DISK_POOL_SIZE - 1);
        assert_eq!(fc.vmdk_guest_path(&config).unwrap(), "/dev/vdk");
        for index in [0, u64::from(DISK_POOL_SIZE), u64::MAX] {
            config.index = index;
            assert!(fc.vmdk_guest_path(&config).is_err());
        }
        config.index = 1;
        config.is_readonly = false;
        assert!(fc.vmdk_guest_path(&config).is_err());
    }
}
