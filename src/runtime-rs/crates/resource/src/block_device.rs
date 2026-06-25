// Copyright 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use hypervisor::{
    device::pci_path::PciPath, BlockConfig, BlockConfigModern, KATA_BLK_DEV_TYPE,
    KATA_CCW_DEV_TYPE, KATA_SCSI_DEV_TYPE,
};

pub(crate) trait BlockConfigStorageSource {
    fn driver_option(&self) -> &str;
    fn pci_path(&self) -> Option<&PciPath>;
    fn scsi_addr(&self) -> Option<&str>;
    fn ccw_addr(&self) -> Option<&str>;
    fn virt_path(&self) -> &str;
}

macro_rules! impl_block_config_storage_source {
    ($config:ty) => {
        impl BlockConfigStorageSource for $config {
            fn driver_option(&self) -> &str {
                &self.driver_option
            }

            fn pci_path(&self) -> Option<&PciPath> {
                self.pci_path.as_ref()
            }

            fn scsi_addr(&self) -> Option<&str> {
                self.scsi_addr.as_deref()
            }

            fn ccw_addr(&self) -> Option<&str> {
                self.ccw_addr.as_deref()
            }

            fn virt_path(&self) -> &str {
                &self.virt_path
            }
        }
    };
}

impl_block_config_storage_source!(BlockConfig);
impl_block_config_storage_source!(BlockConfigModern);

/// Return the source value to pass to the agent for a hypervisor block device.
pub(crate) fn agent_storage_source_from_block_config(
    config: &impl BlockConfigStorageSource,
) -> Result<String> {
    match config.driver_option() {
        KATA_BLK_DEV_TYPE => config
            .pci_path()
            .map(ToString::to_string)
            .ok_or_else(|| anyhow!("block driver is blk but no pci path exists")),
        KATA_SCSI_DEV_TYPE => config
            .scsi_addr()
            .map(ToString::to_string)
            .ok_or_else(|| anyhow!("block driver is scsi but no scsi address exists")),
        KATA_CCW_DEV_TYPE => config
            .ccw_addr()
            .map(ToString::to_string)
            .ok_or_else(|| anyhow!("block driver is ccw but no ccw address exists")),
        _ => Ok(config.virt_path().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hypervisor::KATA_MMIO_BLK_DEV_TYPE;
    use std::convert::TryFrom;

    #[test]
    fn blk_source_uses_pci_path() {
        let config = BlockConfig {
            driver_option: KATA_BLK_DEV_TYPE.to_string(),
            pci_path: Some(PciPath::try_from("01/0a/05").unwrap()),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        let source = agent_storage_source_from_block_config(&config).unwrap();

        assert_eq!(source, "01/0a/05");
    }

    #[test]
    fn blk_source_requires_pci_path() {
        let config = BlockConfig {
            driver_option: KATA_BLK_DEV_TYPE.to_string(),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        let err = agent_storage_source_from_block_config(&config).unwrap_err();

        assert_eq!(
            err.to_string(),
            "block driver is blk but no pci path exists"
        );
    }

    #[test]
    fn scsi_source_uses_scsi_address() {
        let config = BlockConfig {
            driver_option: KATA_SCSI_DEV_TYPE.to_string(),
            scsi_addr: Some("2:1".to_string()),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        let source = agent_storage_source_from_block_config(&config).unwrap();

        assert_eq!(source, "2:1");
    }

    #[test]
    fn modern_scsi_source_uses_scsi_address() {
        let config = BlockConfigModern {
            driver_option: KATA_SCSI_DEV_TYPE.to_string(),
            scsi_addr: Some("3:2".to_string()),
            virt_path: "/dev/vdb".to_string(),
            ..Default::default()
        };

        let source = agent_storage_source_from_block_config(&config).unwrap();

        assert_eq!(source, "3:2");
    }

    #[test]
    fn scsi_source_requires_scsi_address() {
        let config = BlockConfig {
            driver_option: KATA_SCSI_DEV_TYPE.to_string(),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        let err = agent_storage_source_from_block_config(&config).unwrap_err();

        assert_eq!(
            err.to_string(),
            "block driver is scsi but no scsi address exists"
        );
    }

    #[test]
    fn ccw_source_uses_ccw_address() {
        let config = BlockConfig {
            driver_option: KATA_CCW_DEV_TYPE.to_string(),
            ccw_addr: Some("0.0.0005".to_string()),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        let source = agent_storage_source_from_block_config(&config).unwrap();

        assert_eq!(source, "0.0.0005");
    }

    #[test]
    fn ccw_source_requires_ccw_address() {
        let config = BlockConfig {
            driver_option: KATA_CCW_DEV_TYPE.to_string(),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        let err = agent_storage_source_from_block_config(&config).unwrap_err();

        assert_eq!(
            err.to_string(),
            "block driver is ccw but no ccw address exists"
        );
    }

    #[test]
    fn fallback_source_uses_virt_path() {
        let config = BlockConfig {
            driver_option: KATA_MMIO_BLK_DEV_TYPE.to_string(),
            virt_path: "/dev/vda".to_string(),
            ..Default::default()
        };

        let source = agent_storage_source_from_block_config(&config).unwrap();

        assert_eq!(source, "/dev/vda");
    }
}
