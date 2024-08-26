// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::{DeviceContext, DeviceHandler, DeviceInfo, SpecUpdate, BLOCK};
use crate::linux_abi::*;
use crate::sandbox::Sandbox;
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
use anyhow::{anyhow, Context, Result};
use kata_types::device::DRIVER_SCSI_TYPE;
use protocols::agent::Device;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::instrument;

#[derive(Debug)]
pub struct ScsiDeviceHandler {}

#[async_trait::async_trait]
impl DeviceHandler for ScsiDeviceHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_SCSI_TYPE]
    }

    #[instrument]
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate> {
        let vm_path = get_scsi_device_name(ctx.sandbox, &device.id).await?;

        Ok(DeviceInfo::new(&vm_path, true)
            .context("New device info")?
            .into())
    }
}

#[instrument]
pub async fn get_scsi_device_name(
    sandbox: &Arc<Mutex<Sandbox>>,
    scsi_addr: &str,
) -> Result<String> {
    let matcher = ScsiBlockMatcher::new(scsi_addr);

    scan_scsi_bus(scsi_addr)?;
    let uev = wait_for_uevent(sandbox, matcher).await?;
    Ok(format!("{}/{}", SYSTEM_DEV_PATH, &uev.devname))
}

// FIXME: This matcher is only correct if the guest has at most one
// SCSI host.
#[derive(Debug)]
pub struct ScsiBlockMatcher {
    search: String,
}

impl ScsiBlockMatcher {
    pub fn new(scsi_addr: &str) -> ScsiBlockMatcher {
        let search = format!(r"/0:0:{}/block/", scsi_addr);

        ScsiBlockMatcher { search }
    }
}

impl UeventMatcher for ScsiBlockMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.subsystem == BLOCK && uev.devpath.contains(&self.search) && !uev.devname.is_empty()
    }
}

/// Scan SCSI bus for the given SCSI address(SCSI-Id and LUN)
#[instrument]
fn scan_scsi_bus(scsi_addr: &str) -> Result<()> {
    let tokens: Vec<&str> = scsi_addr.split(':').collect();
    if tokens.len() != 2 {
        return Err(anyhow!(
            "Unexpected format for SCSI Address: {}, expect SCSIID:LUA",
            scsi_addr
        ));
    }

    // Scan scsi host passing in the channel, SCSI id and LUN.
    // Channel is always 0 because we have only one SCSI controller.
    let scan_data = &format!("0 {} {}", tokens[0], tokens[1]);

    for entry in fs::read_dir(SYSFS_SCSI_HOST_PATH)? {
        let host = entry?.file_name();

        let host_str = host.to_str().ok_or_else(|| {
            anyhow!(
                "failed to convert directory entry to unicode for file {:?}",
                host
            )
        })?;

        let scan_path = PathBuf::from(&format!("{}/{}/{}", SYSFS_SCSI_HOST_PATH, host_str, "scan"));

        fs::write(scan_path, scan_data)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[allow(clippy::redundant_clone)]
    async fn test_scsi_block_matcher() {
        let root_bus = create_pci_root_bus_path();
        let devname = "sda";

        let mut uev_a = crate::uevent::Uevent::default();
        let addr_a = "0:0";
        uev_a.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev_a.subsystem = BLOCK.to_string();
        uev_a.devname = devname.to_string();
        uev_a.devpath = format!(
            "{}/0000:00:00.0/virtio0/host0/target0:0:0/0:0:{}/block/sda",
            root_bus, addr_a
        );
        let matcher_a = ScsiBlockMatcher::new(addr_a);

        let mut uev_b = uev_a.clone();
        let addr_b = "2:0";
        uev_b.devpath = format!(
            "{}/0000:00:00.0/virtio0/host0/target0:0:2/0:0:{}/block/sdb",
            root_bus, addr_b
        );
        let matcher_b = ScsiBlockMatcher::new(addr_b);

        assert!(matcher_a.is_match(&uev_a));
        assert!(matcher_b.is_match(&uev_b));
        assert!(!matcher_b.is_match(&uev_a));
        assert!(!matcher_a.is_match(&uev_b));
    }
}
