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

    // Firecracker does not support post-boot drive removal, so instead of
    // detaching the drive we patch it back to the empty placeholder file it
    // was created with at boot, which makes Firecracker close its handle on
    // the real backing device so the host can destroy it. Without this, a
    // terminated container's backing device (e.g. a devmapper snapshot)
    // stays open in the VMM while the generic device manager recycles the
    // drive index, and the next container reusing the slot fails.
    pub(crate) async fn unplug_block_device(&mut self, id: u64) -> Result<()> {
        if id > 0 {
            self.patch_drive_to_placeholder(&id.to_string()).await?;
        }
        Ok(())
    }

    pub(crate) async fn remove_device(&mut self, device: DeviceType) -> Result<()> {
        info!(sl(), "Remove Device {} ", device);
        if self.state != VmmState::VmRunning {
            // Nothing was attached to a running VMM, so there is nothing to
            // release; sandbox teardown unmounts any jailed resources.
            return Ok(());
        }
        match device {
            DeviceType::BlockModern(block_mod) => {
                let block = block_mod.lock().await.clone();
                self.unplug_block_device(block.config.index)
                    .await
                    .context("unplug block device")
            }
            // Firecracker cannot remove network or vsock devices from a
            // running VMM and they hold no per-container host resources.
            _ => Ok(()),
        }
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
