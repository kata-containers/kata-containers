// Copyright 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use hypervisor::{
    BlockConfigModern, KATA_BLK_DEV_TYPE, KATA_CCW_DEV_TYPE, KATA_MMIO_BLK_DEV_TYPE,
    KATA_NVDIMM_DEV_TYPE, KATA_SCSI_DEV_TYPE,
};

/// Return the source value to pass to the agent for a hypervisor block device.
pub(crate) fn agent_storage_source_from_block_config(config: &BlockConfigModern) -> Result<String> {
    match config.driver_option.as_str() {
        KATA_BLK_DEV_TYPE => config
            .pci_path
            .as_ref()
            .map(ToString::to_string)
            .ok_or_else(|| anyhow!("block driver is blk but no pci path exists")),
        KATA_SCSI_DEV_TYPE => config
            .scsi_addr
            .clone()
            .ok_or_else(|| anyhow!("block driver is scsi but no scsi address exists")),
        KATA_CCW_DEV_TYPE => config
            .ccw_addr
            .clone()
            .ok_or_else(|| anyhow!("block driver is ccw but no ccw address exists")),
        KATA_MMIO_BLK_DEV_TYPE | KATA_NVDIMM_DEV_TYPE => Ok(config.virt_path.clone()),
        driver => Err(anyhow!("unsupported block driver {driver}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypervisor::device::pci_path::PciPath;
    use std::convert::TryFrom;

    #[test]
    fn blk_source_uses_pci_path() {
        let config = BlockConfigModern {
            driver_option: KATA_BLK_DEV_TYPE.to_string(),
            pci_path: Some(PciPath::try_from("01/0a/05").unwrap()),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        assert_eq!(
            agent_storage_source_from_block_config(&config).unwrap(),
            "01/0a/05"
        );
    }

    #[test]
    fn blk_source_requires_pci_path() {
        let config = BlockConfigModern {
            driver_option: KATA_BLK_DEV_TYPE.to_string(),
            ..Default::default()
        };

        assert_eq!(
            agent_storage_source_from_block_config(&config)
                .unwrap_err()
                .to_string(),
            "block driver is blk but no pci path exists"
        );
    }

    #[test]
    fn scsi_source_uses_scsi_address() {
        let config = BlockConfigModern {
            driver_option: KATA_SCSI_DEV_TYPE.to_string(),
            scsi_addr: Some("2:1".to_string()),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        assert_eq!(
            agent_storage_source_from_block_config(&config).unwrap(),
            "2:1"
        );
    }

    #[test]
    fn scsi_source_requires_scsi_address() {
        let config = BlockConfigModern {
            driver_option: KATA_SCSI_DEV_TYPE.to_string(),
            ..Default::default()
        };

        assert_eq!(
            agent_storage_source_from_block_config(&config)
                .unwrap_err()
                .to_string(),
            "block driver is scsi but no scsi address exists"
        );
    }

    #[test]
    fn ccw_source_uses_ccw_address() {
        let config = BlockConfigModern {
            driver_option: KATA_CCW_DEV_TYPE.to_string(),
            ccw_addr: Some("0.0.0005".to_string()),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        assert_eq!(
            agent_storage_source_from_block_config(&config).unwrap(),
            "0.0.0005"
        );
    }

    #[test]
    fn ccw_source_requires_ccw_address() {
        let config = BlockConfigModern {
            driver_option: KATA_CCW_DEV_TYPE.to_string(),
            ..Default::default()
        };

        assert_eq!(
            agent_storage_source_from_block_config(&config)
                .unwrap_err()
                .to_string(),
            "block driver is ccw but no ccw address exists"
        );
    }

    #[test]
    fn path_based_sources_use_virt_path() {
        for driver in [KATA_MMIO_BLK_DEV_TYPE, KATA_NVDIMM_DEV_TYPE] {
            let config = BlockConfigModern {
                driver_option: driver.to_string(),
                virt_path: "/dev/vda".to_string(),
                ..Default::default()
            };

            assert_eq!(
                agent_storage_source_from_block_config(&config).unwrap(),
                "/dev/vda"
            );
        }
    }

    #[test]
    fn unknown_driver_is_rejected() {
        let config = BlockConfigModern {
            driver_option: "unknown".to_string(),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        assert_eq!(
            agent_storage_source_from_block_config(&config)
                .unwrap_err()
                .to_string(),
            "unsupported block driver unknown"
        );
    }
}
