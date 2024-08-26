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
