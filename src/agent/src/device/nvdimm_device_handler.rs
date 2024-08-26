// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::{DeviceContext, DeviceHandler, DeviceInfo, SpecUpdate, BLOCK};
use crate::linux_abi::ACPI_DEV_PATH;
use crate::sandbox::Sandbox;
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
use anyhow::{anyhow, Context, Result};
use kata_types::device::DRIVER_NVDIMM_TYPE;
use protocols::agent::Device;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::instrument;

#[derive(Debug)]
pub struct VirtioNvdimmDeviceHandler {}

#[async_trait::async_trait]
impl DeviceHandler for VirtioNvdimmDeviceHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_NVDIMM_TYPE]
    }

    #[instrument]
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate> {
        if device.vm_path.is_empty() {
            return Err(anyhow!("Invalid path for nvdimm device"));
        }
        Ok(DeviceInfo::new(device.vm_path(), true)
            .context("New device info")?
            .into())
    }
}

#[instrument]
pub async fn wait_for_pmem_device(sandbox: &Arc<Mutex<Sandbox>>, devpath: &str) -> Result<()> {
    let devname = match devpath.strip_prefix("/dev/") {
        Some(dev) => dev,
        None => {
            return Err(anyhow!(
                "Storage source '{}' must start with /dev/",
                devpath
            ))
        }
    };

    let matcher = PmemBlockMatcher::new(devname);
    let uev = wait_for_uevent(sandbox, matcher).await?;
    if uev.devname != devname {
        return Err(anyhow!(
            "Unexpected device name {} for pmem device (expected {})",
            uev.devname,
            devname
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub struct PmemBlockMatcher {
    suffix: String,
}

impl PmemBlockMatcher {
    pub fn new(devname: &str) -> PmemBlockMatcher {
        let suffix = format!(r"/block/{}", devname);

        PmemBlockMatcher { suffix }
    }
}

impl UeventMatcher for PmemBlockMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.subsystem == BLOCK
            && uev.devpath.starts_with(ACPI_DEV_PATH)
            && uev.devpath.ends_with(&self.suffix)
            && !uev.devname.is_empty()
    }
}
