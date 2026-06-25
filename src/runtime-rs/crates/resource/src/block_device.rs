// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use hypervisor::device::pci_path::PciPath;
use kata_types::device::{
    DRIVER_BLK_CCW_TYPE as KATA_CCW_DEV_TYPE, DRIVER_BLK_PCI_TYPE as KATA_BLK_DEV_TYPE,
    DRIVER_SCSI_TYPE as KATA_SCSI_DEV_TYPE,
};

/// Return the source value to pass to the agent for a hypervisor block device.
pub(crate) fn agent_storage_source_from_block_config(
    driver_option: &str,
    pci_path: Option<&PciPath>,
    scsi_addr: Option<&str>,
    ccw_addr: Option<&str>,
    virt_path: &str,
) -> Result<String> {
    match driver_option {
        KATA_BLK_DEV_TYPE => pci_path
            .map(ToString::to_string)
            .ok_or_else(|| anyhow!("block driver is blk but no pci path exists")),
        KATA_SCSI_DEV_TYPE => scsi_addr
            .map(ToString::to_string)
            .ok_or_else(|| anyhow!("block driver is scsi but no scsi address exists")),
        KATA_CCW_DEV_TYPE => ccw_addr
            .map(ToString::to_string)
            .ok_or_else(|| anyhow!("block driver is ccw but no ccw address exists")),
        _ => Ok(virt_path.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scsi_source_uses_scsi_address() {
        let source = agent_storage_source_from_block_config(
            KATA_SCSI_DEV_TYPE,
            None,
            Some("2:0:0:1"),
            None,
            "/dev/vda",
        )
        .unwrap();

        assert_eq!(source, "2:0:0:1");
    }

    #[test]
    fn scsi_source_requires_scsi_address() {
        let err = agent_storage_source_from_block_config(
            KATA_SCSI_DEV_TYPE,
            None,
            None,
            None,
            "/dev/vda",
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "block driver is scsi but no scsi address exists"
        );
    }
}
