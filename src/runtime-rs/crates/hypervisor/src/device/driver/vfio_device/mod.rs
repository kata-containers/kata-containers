// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod core;
mod device;

pub use core::{discover_vfio_group_device, VfioDevice};
pub use device::VfioDeviceBase;
pub use device::VfioDeviceModern;
pub use device::VfioDeviceModernHandle;

use std::fs;
use std::path::Path;

use anyhow::Result;

const DEV_VFIO_CTL: &str = "/dev/vfio/vfio";
const DEV_IOMMU: &str = "/dev/iommu";
const DEV_VFIO_DEVICES: &str = "/dev/vfio/devices";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfioBackendChoice {
    /// legacy VFIO group/container: /dev/vfio/vfio + /dev/vfio/<group>
    LegacyGroup,
    /// iommufd backend: /dev/iommu + /dev/vfio/devices/vfioX
    Iommufd,
}

#[derive(Debug, Default, Clone)]
pub struct VfioHostCaps {
    pub has_vfio_ctl: bool,  // /dev/vfio/vfio exists
    pub has_iommufd: bool,   // /dev/iommu exists
    pub has_vfio_cdev: bool, // /dev/vfio/devices exists and contains vfio*
}

pub fn detect_vfio_host_caps() -> VfioHostCaps {
    let has_vfio_ctl = Path::new(DEV_VFIO_CTL).exists();
    let has_iommufd = Path::new(DEV_IOMMU).exists();

    let has_vfio_cdev = match fs::read_dir(DEV_VFIO_DEVICES) {
        Ok(rd) => rd
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("vfio")),
        Err(_) => false,
    };

    VfioHostCaps {
        has_vfio_ctl,
        has_iommufd,
        has_vfio_cdev,
    }
}

pub fn choose_vfio_backend(caps: &VfioHostCaps) -> Result<VfioBackendChoice> {
    // Prefer iommufd when fully supported
    if caps.has_iommufd && caps.has_vfio_cdev {
        return Ok(VfioBackendChoice::Iommufd);
    }

    // Fallback to legacy VFIO container/group
    if caps.has_vfio_ctl {
        return Ok(VfioBackendChoice::LegacyGroup);
    }

    Err(anyhow::anyhow!(
        "No usable VFIO backend: caps={:?}. Need (/dev/iommu + /dev/vfio/devices/vfio*) \
         for iommufd, or /dev/vfio/vfio for legacy.",
        caps
    ))
}
