// Copyright (C) 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause

//! Related to Dragonball MMIO extension.

/// Device Vendor ID for virtio devices emulated by Dragonball.
/// The upper 24 bits are used as vendor id, and the lower 8 bits are used as features.
pub const MMIO_VENDOR_ID_DRAGONBALL: u32 = 0xdbfcdb00;

/// Mask for feature flags in the vendor id field
pub const DRAGONBALL_FEATURE_MASK: u32 = 0xff;

/// Assume `MMIO_INT_VRING` is always set in the interrupt status register when handling interrupts.
/// With this feature available, the device driver may optimize the way to handle interrupts.
pub const DRAGONBALL_FEATURE_INTR_USED: u32 = 0x1;

/// The device supports Message Signaled Interrupt.
pub const DRAGONBALL_FEATURE_MSI_INTR: u32 = 0x2;

/// The device implements per-queue notification register.
/// If this feature bit is set, the VIRTIO_MMIO_QUEUE_NOTIFY register becomes read-write.
/// On reading, the lower 16-bit contains doorbell base offset starting from the MMIO window base,
/// and the upper 16-bit contains scale for the offset. The notification register address for
/// virtque is:
///     offset = base + doorbell_base + doorbell_scale * queue_idx
pub const DRAGONBALL_FEATURE_PER_QUEUE_NOTIFY: u32 = 0x4;

/// PVDMA feature enabled
pub const DRAGONBALL_FEATURE_PVDMA: u32 = 0x08;

/// Default size resrved for virtio-mmio doorbell address space.
///
/// This represents the size of the mmio device reserved for doorbell which used to per queue notify,
/// we need to request resource with the `MMIO_DEFAULT_CFG_SIZE + DRAGONBALL_MMIO_DOORBELL_SIZE`
pub const DRAGONBALL_MMIO_DOORBELL_SIZE: u64 = 0x1000;

/// Default offset of the mmio doorbell
pub const DRAGONBALL_MMIO_DOORBELL_OFFSET: u64 = 0x1000;

/// Max queue num when the `fast-mmio` enabled, because we only reserved 0x200 memory region for
/// per queue notify
pub const DRAGONBALL_MMIO_MAX_QUEUE_NUM: u64 = 255;

/// Scale of the doorbell for per queue notify
pub const DRAGONBALL_MMIO_DOORBELL_SCALE: u64 = 0x04;

/// This represents the offset at which the device should call DeviceIo::write in order to write
/// to its configuration space.
pub const MMIO_CFG_SPACE_OFF: u64 = 0x100;

// The format of the 16-bit MSI Control and Status register.
// On read:
// - bit 15: 1 if MSI is supported, 0 if MSI is not supported.
// - bit 0-14: reserved, read as zero.
// On write:
// - bit 15: 1 to enable MSI, 0 to disable MSI.
// - bit 0-14: ignored.

/// Message Signaled Interrupt is supported when reading from the CSR.
pub const MMIO_MSI_CSR_SUPPORTED: u16 = 0x8000;

/// Enable MSI if this bit is set when writing to the CSR, otherwise disable MSI.
pub const MMIO_MSI_CSR_ENABLED: u16 = 0x8000;

// The format of the 16-bit write-only MSI Command register.
// - bit 12-15: command code
// - bit 0-11: command parameter

/// Mask for the command code in the MSI command register.
pub const MMIO_MSI_CMD_CODE_MASK: u16 = 0xf000;

/// Mask for the command argument in the MSI command register.
pub const MMIO_MSI_CMD_ARG_MASK: u16 = 0x0fff;

/// Command code to update MSI entry configuration.
/// The argument is the MSI vector number to update.
pub const MMIO_MSI_CMD_CODE_UPDATE: u16 = 0x1000;
/// Comamnd to mask and unmask msi interrupt
pub const MMIO_MSI_CMD_CODE_INT_MASK: u16 = 0x2000;
pub const MMIO_MSI_CMD_CODE_INT_UNMASK: u16 = 0x3000;

// Define a 16-byte area to control MMIO MSI

// MSI control/status register offset
pub const REG_MMIO_MSI_CSR: u64 = 0x0c0;
// MSI command register offset
pub const REG_MMIO_MSI_COMMAND: u64 = 0x0c2;
// MSI address_lo register offset
pub const REG_MMIO_MSI_ADDRESS_L: u64 = 0x0c4;
// MSI address_hi register offset
pub const REG_MMIO_MSI_ADDRESS_H: u64 = 0x0c8;
// MSI data register offset
pub const REG_MMIO_MSI_DATA: u64 = 0x0cc;

// RW: MSI feature enabled
pub const REG_MMIO_MSI_CSR_ENABLE: u64 = 0x8000;
// RO: Maximum queue size available
pub const REG_MMIO_MSI_CSR_QMASK: u64 = 0x07ff;
// Reserved
pub const REG_MMIO_MSI_CSR_RESERVED: u64 = 0x7800;

pub const REG_MMIO_MSI_CMD_UPDATE: u64 = 0x1;

/// Defines the offset and scale of the mmio doorbell.
///
/// Support per-virtque doorbell, so the guest kernel may directly write to the doorbells provided
/// by hardware virtio devices.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct DoorBell {
    offset: u32,
    scale: u32,
}

impl DoorBell {
    /// Creates a Doorbell.
    pub fn new(offset: u32, scale: u32) -> Self {
        Self { offset, scale }
    }

    /// Returns the offset.
    pub fn offset(&self) -> u32 {
        self.offset
    }

    /// Returns the scale.
    pub fn scale(&self) -> u32 {
        self.scale
    }

    /// Returns the offset with the specified index of virtio queue.
    pub fn queue_offset(&self, queue_index: usize) -> u64 {
        (self.offset as u64) + (self.scale as u64) * (queue_index as u64)
    }

    /// Returns the register data.
    pub fn register_data(&self) -> u32 {
        self.offset | (self.scale << 16)
    }
}

/// MSI interrupts.
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Msi {
    pub index_select: u32,
    pub address_low: u32,
    pub address_high: u32,
    pub data: u32,
}

impl Msi {
    /// Sets index select.
    pub fn set_index_select(&mut self, v: u32) {
        self.index_select = v;
    }
    /// Sets address low.
    pub fn set_address_low(&mut self, v: u32) {
        self.address_low = v;
    }
    /// Sets address high.
    pub fn set_address_high(&mut self, v: u32) {
        self.address_high = v;
    }
    /// Sets msi data.
    pub fn set_data(&mut self, v: u32) {
        self.data = v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doorbell() {
        let door = DoorBell::new(
            DRAGONBALL_MMIO_DOORBELL_OFFSET as u32,
            DRAGONBALL_MMIO_DOORBELL_SCALE as u32,
        );
        assert_eq!(door.offset(), DRAGONBALL_MMIO_DOORBELL_OFFSET as u32);
        assert_eq!(door.scale(), DRAGONBALL_MMIO_DOORBELL_SCALE as u32);
        assert_eq!(door.queue_offset(0), DRAGONBALL_MMIO_DOORBELL_OFFSET);
        assert_eq!(door.queue_offset(4), 0x1010);
        assert_eq!(door.register_data(), 0x1000 | 0x40000);
    }

    #[test]
    fn test_msi() {
        let mut msi = Msi::default();
        msi.set_index_select(1);
        msi.set_address_low(2);
        msi.set_address_high(3);
        msi.set_data(4);
        assert_eq!(
            msi,
            Msi {
                index_select: 1,
                address_low: 2,
                address_high: 3,
                data: 4
            }
        );
    }
}
