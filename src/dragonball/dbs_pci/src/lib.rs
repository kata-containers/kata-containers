// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
// Copyright 2018 The Chromium OS Authors. All rights reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
//![deny(missing_docs)]
//!
//! Implements PCI devices and buses.
//!
//! The role and relationship about PCI related traits/structs:
//! - PCI root: a pseudo device to handle PCI configuration accesses.
//! - PCI bus: a container object to hold PCI devices and resources, corresponding to the PCI bus
//!   defined in PCI/PCIe specs.
//! - PCI root bus: a special PCI bus which has no parent PCI bus. The device 0 under PCI root bus
//!   represent the root bus itself.
//! - PCI device: the real object to emulate a PCI device. For most PCI devices, it needs to
//!   handle accesses to PCI configuration space and PCI BARs.
//! - PCI configuration: a common framework to emulator PCI configuration space header.
//! - PCI MSI/MSIx: structs to emulate PCI MSI/MSIx capabilities.

use std::sync::Arc;

use dbs_device::device_manager::IoManagerContext;
use dbs_interrupt::KvmIrqManager;

mod bus;
pub use bus::PciBus;

mod configuration;
pub use configuration::{
    BarProgrammingParams, PciBarConfiguration, PciBarPrefetchable, PciBarRegionType,
    PciBridgeSubclass, PciCapability, PciCapabilityId, PciClassCode, PciConfiguration,
    PciHeaderType, PciInterruptPin, PciMassStorageSubclass, PciMultimediaSubclass,
    PciNetworkControllerSubclass, PciProgrammingInterface, PciSerialBusSubClass, PciSubclass,
    NUM_BAR_REGS, NUM_CONFIGURATION_REGISTERS,
};

mod device;
pub use device::PciDevice;
#[cfg(target_arch = "aarch64")]
pub use device::{PciBusResources, ECAM_SPACE_LENGTH};

mod root_bus;
pub use root_bus::create_pci_root_bus;

mod root_device;
pub use root_device::PciRootDevice;

mod msi;
pub use msi::{MsiCap, MsiState};

mod msix;
pub use msix::{MsixCap, MsixState, MSIX_TABLE_ENTRY_SIZE};

mod vfio;
pub use vfio::{VfioPciDevice, VfioPciError, VENDOR_NVIDIA};

/// Error codes related to PCI root/bus/device operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Failed to activate the PCI root/bus/device.
    #[error("failed to activate PCI device, {0:?}")]
    ActivateFailure(#[source] dbs_device::device_manager::Error),
    /// Invalid resource assigned/allocated.
    #[error("invalid resource {0:?}")]
    InvalidResource(dbs_device::resources::Resource),
    /// Invalid bus id
    #[error("bus id {0} invalid")]
    InvalidBusId(u8),
    /// Errors from IoManager
    /// No resources available.
    #[error("No resources available")]
    NoResources,
    /// Zero sized PCI capability
    #[error("empty capabilities are invalid")]
    CapabilityEmpty,
    /// No space available for new PCI capability.
    #[error("capability of size {0} doesn't fit")]
    CapabilitySpaceFull(usize),
    /// PCI BAR is already in use.
    #[error("bar {0} already used")]
    BarInUse(usize),
    /// PCI BAR is invalid.
    #[error("bar {0} invalid, max {}", NUM_BAR_REGS - 1)]
    BarInvalid(usize),
    /// PCI BAR size is invalid.
    #[error("bar address {0} not a power of two")]
    BarSizeInvalid(u64),
    /// PCI BAR address is invalid.
    #[error("address {0} size {1} too big")]
    BarAddressInvalid(u64, u64),
    /// 64 bits MMIO PCI BAR is invalid.
    #[error("64 bit bar {0} invalid, requires two regs, max {}", NUM_BAR_REGS - 1)]
    BarInvalid64(usize),
    /// 64 bits MMIO PCI BAR is in use.
    #[error("64bit bar {0} already used(requires two regs)")]
    BarInUse64(usize),
    /// PCI ROM BAR is invalid.
    #[error("ROM bar {0} invalid, max {}", NUM_BAR_REGS - 1)]
    RomBarInvalid(usize),
    /// PCI ROM BAR is already in use.
    #[error("rom bar {0} already used")]
    RomBarInUse(usize),
    /// PCI ROM BAR size is invalid.
    #[error("rom bar address {0} not a power of two")]
    RomBarSizeInvalid(u64),
    /// PCI ROM BAR address is invalid.
    #[error("address {0} size {1} too big")]
    RomBarAddressInvalid(u64, u64),
}

/// Specialized `Result` for PCI related operations.
pub type Result<T> = std::result::Result<T, Error>;

pub trait PciSystemContext: Sync + Send + Clone {
    type D: IoManagerContext + Send + Sync + Clone;

    fn get_device_manager_context(&self) -> Self::D;

    fn get_interrupt_manager(&self) -> Arc<KvmIrqManager>;
}

/// Fill the buffer with all bits set for invalid PCI configuration space access.
pub fn fill_config_data(data: &mut [u8]) {
    // Return data with all bits set.
    for pos in data.iter_mut() {
        *pos = 0xff;
    }
}
