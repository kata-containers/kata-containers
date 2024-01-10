// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use rand::Rng;
use std::os::unix::prelude::AsRawFd;
use tokio::fs::{File, OpenOptions};

use async_trait::async_trait;

use crate::{
    device::{topology::PCIeTopology, Device, DeviceType},
    Hypervisor as hypervisor,
};

// This is the first usable vsock context ID. All the vsocks
// can use the same ID, since it's only used in the guest.
pub const DEFAULT_GUEST_VSOCK_CID: u32 = 0x3;

#[derive(Clone, Debug, Default)]
pub struct HybridVsockConfig {
    /// A 32-bit Context Identifier (CID) used to identify the guest.
    pub guest_cid: u32,

    /// unix domain socket path
    pub uds_path: String,
}

#[derive(Clone, Debug, Default)]
pub struct HybridVsockDevice {
    /// Unique identifier of the device
    pub id: String,

    /// config information for HybridVsockDevice
    pub config: HybridVsockConfig,
}

impl HybridVsockDevice {
    pub fn new(device_id: &String, config: &HybridVsockConfig) -> Self {
        Self {
            id: format!("vsock-{}", device_id),
            config: config.clone(),
        }
    }
}

#[async_trait]
impl Device for HybridVsockDevice {
    async fn attach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<()> {
        h.add_device(DeviceType::HybridVsock(self.clone()))
            .await
            .context("add hybrid vsock device.")?;

        return Ok(());
    }

    async fn detach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        _h: &dyn hypervisor,
    ) -> Result<Option<u64>> {
        // no need to do detach, just return Ok(None)
        Ok(None)
    }

    async fn update(&mut self, _h: &dyn hypervisor) -> Result<()> {
        // There's no need to do update for hvsock device
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::HybridVsock(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        // hybrid vsock devices will not be attached multiple times, Just return Ok(false)

        Ok(false)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        // hybrid vsock devices will not be detached multiple times, Just return Ok(false)

        Ok(false)
    }
}

#[derive(Clone, Debug)]
pub struct VsockConfig {
    /// A 32-bit Context Identifier (CID) used to identify the guest.
    pub guest_cid: u32,
}

#[derive(Clone, Debug)]
pub struct VsockDevice {
    /// Unique identifier of the device
    pub id: String,

    /// config information for VsockDevice
    pub config: VsockConfig,
}

const VHOST_VSOCK_DEVICE: &str = "/dev/vhost-vsock";

// From <linux/vhost.h>
// Generate a wrapper function for VHOST_VSOCK_SET_GUEST_CID ioctl.
// It set guest CID for vsock fd, and return error if CID is already
// in use.
const VHOST_VIRTIO_IOCTL: u8 = 0xAF;
const VHOST_VSOCK_SET_GUEST_CID: u8 = 0x60;
nix::ioctl_write_ptr!(
    vhost_vsock_set_guest_cid,
    VHOST_VIRTIO_IOCTL,
    VHOST_VSOCK_SET_GUEST_CID,
    u64
);

const CID_RETRY_COUNT: u32 = 50;

impl VsockDevice {
    pub async fn new(id: String, config: &VsockConfig) -> Result<Self> {
        Ok(Self {
            id,
            config: config.clone(),
        })
    }

    pub async fn init_config(&mut self) -> Result<File> {
        let (guest_cid, vhost_fd) = generate_vhost_vsock_cid()
            .await
            .context("generate vhost vsock cid failed")?;
        self.config.guest_cid = guest_cid;
        Ok(vhost_fd)
    }
}

#[async_trait]
impl Device for VsockDevice {
    async fn attach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<()> {
        h.add_device(DeviceType::Vsock(self.clone()))
            .await
            .context("add vsock device.")?;

        return Ok(());
    }

    async fn detach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        _h: &dyn hypervisor,
    ) -> Result<Option<u64>> {
        // no need to do detach, just return Ok(None)
        Ok(None)
    }

    async fn update(&mut self, _h: &dyn hypervisor) -> Result<()> {
        // There's no need to do update for vsock device
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::Vsock(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        // vsock devices will not be attached multiple times, Just return Ok(false)

        Ok(false)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        // vsock devices will not be detached multiple times, Just return Ok(false)

        Ok(false)
    }
}

pub async fn generate_vhost_vsock_cid() -> Result<(u32, File)> {
    let vhost_fd = OpenOptions::new()
        .read(true)
        .write(true)
        .open(VHOST_VSOCK_DEVICE)
        .await
        .context(format!(
            "failed to open {}, try to run modprobe vhost_vsock.",
            VHOST_VSOCK_DEVICE
        ))?;
    let mut rng = rand::thread_rng();

    // Try 50 times to find a context ID that is not in use.
    for _ in 0..CID_RETRY_COUNT {
        // First usable CID above VMADDR_CID_HOST (see vsock(7))
        let first_usable_cid = 3;
        let rand_cid = rng.gen_range(first_usable_cid..=(u32::MAX));
        let guest_cid =
            unsafe { vhost_vsock_set_guest_cid(vhost_fd.as_raw_fd(), &(rand_cid as u64)) };
        match guest_cid {
            Ok(_) => return Ok((rand_cid, vhost_fd)),
            Err(nix::Error::EADDRINUSE) => {
                // The CID is already in use. Try another one.
                continue;
            }
            Err(err) => {
                return Err(err).context("failed to set guest CID");
            }
        };
    }

    anyhow::bail!(
        "failed to find a free vsock context ID after {} attempts",
        CID_RETRY_COUNT
    );
}
