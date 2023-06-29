// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use rand::Rng;
use std::os::unix::prelude::AsRawFd;
use tokio::fs::{File, OpenOptions};

#[derive(Debug)]
pub struct HybridVsockConfig {
    /// A 32-bit Context Identifier (CID) used to identify the guest.
    pub guest_cid: u32,

    /// unix domain socket path
    pub uds_path: String,
}

#[derive(Debug)]
pub struct HybridVsockDevice {
    /// Unique identifier of the device
    pub id: String,

    /// config information for HybridVsockDevice
    pub config: HybridVsockConfig,
}

#[derive(Debug)]
pub struct VsockConfig {
    /// A 32-bit Context Identifier (CID) used to identify the guest.
    pub guest_cid: u32,

    /// Vhost vsock fd. Hold to ensure CID is not used by other VM.
    pub vhost_fd: File,
}

#[derive(Debug)]
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
    pub async fn new(id: String) -> Result<Self> {
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
                Ok(_) => {
                    return Ok(VsockDevice {
                        id,
                        config: VsockConfig {
                            guest_cid: rand_cid,
                            vhost_fd,
                        },
                    });
                }
                Err(nix::Error::EADDRINUSE) => {
                    // The CID is already in use. Try another one.
                }
                Err(err) => {
                    return Err(err).context("failed to set guest CID");
                }
            }
        }

        anyhow::bail!(
            "failed to find a free vsock context ID after {} attempts",
            CID_RETRY_COUNT
        );
    }
}
