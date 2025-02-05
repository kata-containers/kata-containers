// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use byteorder::{ByteOrder, LittleEndian};
use dbs_interrupt::{
    DeviceInterruptManager, DeviceInterruptMode, InterruptIndex, InterruptManager,
    InterruptSourceGroup,
};
use log::{debug, error, warn};

use crate::configuration::{PciCapability, PciCapabilityId};
use crate::fill_config_data;

// MSI control masks
pub(crate) const MSI_CTL_ENABLE: u16 = 0x1;
const MSI_CTL_MULTI_MSG_ENABLE: u16 = 0x70;
pub(crate) const MSI_CTL_64_BITS: u16 = 0x80;
const MSI_CTL_PER_VECTOR: u16 = 0x100;

// MSI message offsets
const MSI_MSG_CTL_OFFSET: u64 = 0x2;
const MSI_MSG_ADDR_LO_OFFSET: u64 = 0x4;

// MSI message masks
const MSI_MSG_ADDR_LO_MASK: u32 = 0xffff_fffc;
// Bit 4 to bit 6 illustrates the MSI number that is actually enabled.
// The value is the exponent of 2.
const MSI_ENABLED_OFFSET: u8 = 4;
const MSI_ENABLED_MASK: u16 = 0x7;
// Bit 1 to bit 3 illustrates the MSI number that is supported.
// The value is the exponent of 2.
const MSI_SUPPORTED_OFFSET: u8 = 1;
const MSI_SUPPORTED_MASK: u16 = 0x7;
// Mask the MSI mask bit.
const MSI_MASK_BIT_MASK: u32 = 0x1;
// If the MSI vector is masked, mask bit value is 1.
const MSI_VECTOR_MASKED_VALUE: u32 = 1;

/// Get number of vectors enabled from PCI MSI control register.
pub fn msi_num_enabled_vectors(msg_ctl: u16) -> u16 {
    let field = (msg_ctl >> MSI_ENABLED_OFFSET) & MSI_ENABLED_MASK;

    // since we mask down to 3 bit for number of msi enabled, and 3 bit
    // should not be larger than 5, if that happens, meaning that there
    // is some unknown behavior so we return 0.
    if field > 5 {
        return 0;
    }

    1 << field
}

/// Get number of vectors enabled from PCI MSI control register.
pub fn msi_num_supported_vectors(msg_ctl: u16) -> u16 {
    let field = (msg_ctl >> MSI_SUPPORTED_OFFSET) & MSI_SUPPORTED_MASK;

    if field > 5 {
        return 0;
    }

    1 << field
}

/// Struct to maintain information for PCI Message Signalled Interrupt Capability.
///
/// This struct is the shadow copy of the PCI MSI capability. Guest device drivers read from/write
/// to this struct. There's another struct MsiState, which maintains the working state about the
/// PCI MSI controller.
///
/// Why splits it into MsiCap and MsiState?
/// There are three possible types of interrupt controller supported by a PCI device: Legacy Irq,
/// MSI and MSI-x. And the PCI specifications define the priority order as:
///     MSI-x > MSI > Legacy
/// That means the MSI capability may be enabled but doesn't take effect due to that the MSI-x
/// capability is enabled too, which has higher priority than MSI. In that case, the MsiCap
/// represent register state in PCI configuration space and is in ENABLED state, but the MsiState
/// maintain the really working state and is in DISABLED state.
#[derive(Clone, Default, PartialEq)]
pub struct MsiCap {
    // Next pointer and Capability ID
    // capability id (high 8 bit) | next capability pointer (low 8 bit)
    cap_id_next: u16,
    // Message Control Register
    //   0:     MSI enable.
    //   3-1;   Multiple message capable.
    //   6-4:   Multiple message enable.
    //   7:     64 bits address capable.
    //   8:     Per-vector masking capable.
    //   15-9:  Reserved.
    msg_ctl: u16,
    // Message Address (LSB)
    //   1-0:  Reserved.
    //   31-2: Message address.
    msg_addr_lo: u32,
    // Message Upper Address (MSB)
    //   31-0: Message address.
    msg_addr_hi: u32,
    // Message Data
    //   15-0: Message data.
    msg_data: u16,
    // Mask Bits
    //   31-0: Mask bits.
    mask_bits: u32,
    // Pending Bits
    //   31-0: Pending bits.
    _pending_bits: u32,
    group: Option<Arc<Box<dyn InterruptSourceGroup>>>,
}

impl MsiCap {
    /// Create a new PCI MSI capability structure.
    pub fn new(next: u8, mut msg_ctl: u16) -> Self {
        let cap_id_next = (next as u16) << 8 | PciCapabilityId::MessageSignalledInterrupts as u16;

        // By default MSI capability is disabled, and driver needs to explicitly turn it on.
        msg_ctl &= !MSI_CTL_ENABLE;

        MsiCap {
            cap_id_next,
            msg_ctl,
            ..Default::default()
        }
    }

    /// Set InterruptSourceGroup object associated with the MSI capability.
    pub fn set_group(&mut self, group: Option<Arc<Box<dyn InterruptSourceGroup>>>) {
        self.group = group;
    }

    /// Check whether the PCI MSI capability has been enabled.
    pub fn enabled(&self) -> bool {
        self.msg_ctl & MSI_CTL_ENABLE == MSI_CTL_ENABLE
    }

    /// Get number of vectors supported.
    pub fn num_vectors(&self) -> u16 {
        msi_num_supported_vectors(self.msg_ctl)
    }

    /// Get number of vectors enabled.
    pub fn num_enabled_vectors(&self) -> u16 {
        msi_num_enabled_vectors(self.msg_ctl)
    }

    /// Check whether 64-bit message address is supported or not.
    pub fn addr_64_bits(&self) -> bool {
        self.msg_ctl & MSI_CTL_64_BITS == MSI_CTL_64_BITS
    }

    /// Check whether per vector masking extension is supported or not.
    pub fn per_vector_mask(&self) -> bool {
        self.msg_ctl & MSI_CTL_PER_VECTOR == MSI_CTL_PER_VECTOR
    }

    /// Check whether the `vector` is masked or not.
    pub fn vector_masked(&self, vector: u16) -> bool {
        if !self.per_vector_mask() {
            return false;
        }

        (self.mask_bits >> vector) & MSI_MASK_BIT_MASK == MSI_VECTOR_MASKED_VALUE
    }

    /// Get size of the PCI MSI capability structure, including the capability header.
    pub fn size(&self) -> u32 {
        // basic size of PCI MSI capability structure
        let mut size: u32 = 0xa;

        // If the 64 bit mode is supported, there will be an extra 32 bit
        // to illustrate higher 32 bit address.
        if self.addr_64_bits() {
            size += 0x4;
        }

        // extra space for msi mask bit
        if self.per_vector_mask() {
            size += 0xa;
        }

        size
    }

    /// Handle read accesses to the PCI MSI capability structure.
    pub fn read(&mut self, offset: u64, data: &mut [u8]) -> std::io::Result<()> {
        let (msg_data_offset, addr_hi_offset, mask_bits_offset) = self.get_offset_info();

        // Please be really careful to touch code below, you should have good understanding of
        // PCI MSI capability.
        // Support reading capability id, next capability pointer, mask bits and pending bit.
        match data.len() {
            1 => match offset {
                0 => data[0] = self.cap_id_next as u8,
                1 => data[0] = (self.cap_id_next >> 8) as u8,
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() => {
                    data[0] = self.mask_bits as u8;
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 1 => {
                    data[0] = (self.mask_bits >> 8) as u8;
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 2 => {
                    data[0] = (self.mask_bits >> 16) as u8;
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 3 => {
                    data[0] = (self.mask_bits >> 24) as u8;
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 4 => {
                    self.get_pending_state(data, 0);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 5 => {
                    self.get_pending_state(data, 1);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 6 => {
                    self.get_pending_state(data, 2);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 7 => {
                    self.get_pending_state(data, 3);
                }
                _ => debug!("invalid offset {} (data length {})", offset, data.len()),
            },
            2 => match offset {
                0 => LittleEndian::write_u16(data, self.cap_id_next),
                MSI_MSG_CTL_OFFSET => LittleEndian::write_u16(data, self.msg_ctl),
                x if x == msg_data_offset => {
                    LittleEndian::write_u16(data, self.msg_data);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() => {
                    LittleEndian::write_u16(data, self.mask_bits as u16);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 2 => {
                    LittleEndian::write_u16(data, (self.mask_bits >> 16) as u16);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 4 => {
                    self.get_pending_state(data, 0);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 6 => {
                    self.get_pending_state(data, 2);
                }
                _ => debug!("invalid offset {} (data length {})", offset, data.len()),
            },
            4 => match offset {
                0x0 => LittleEndian::write_u32(
                    data,
                    self.cap_id_next as u32 | ((self.msg_ctl as u32) << 16),
                ),
                MSI_MSG_ADDR_LO_OFFSET => LittleEndian::write_u32(data, self.msg_addr_lo),
                x if addr_hi_offset.is_some() && x == addr_hi_offset.unwrap() => {
                    LittleEndian::write_u32(data, self.msg_addr_hi);
                }
                x if x == msg_data_offset => {
                    LittleEndian::write_u32(data, self.msg_data as u32);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() => {
                    LittleEndian::write_u32(data, self.mask_bits);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 4 => {
                    self.get_pending_state(data, 0);
                }
                _ => debug!("invalid offset {} (data length {})", offset, data.len()),
            },
            _ => debug!("invalid data length {:?}", data.len()),
        }

        Ok(())
    }

    /// Handle write accesses to the PCI MSI capability structure.
    pub fn write(&mut self, offset: u64, data: &[u8]) -> std::io::Result<()> {
        let (msg_data_offset, addr_hi_offset, mask_bits_offset) = self.get_offset_info();

        // Update cache without overriding the read-only bits.
        // Support writing mask_bits, MSI control information, message data, message address.
        match data.len() {
            1 => match offset {
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() => {
                    self.mask_bits = data[0] as u32 | (self.mask_bits & 0xffff_ff00);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 1 => {
                    self.mask_bits = (data[0] as u32) << 8 | (self.mask_bits & 0xffff_00ff);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 2 => {
                    self.mask_bits = (data[0] as u32) << 16 | (self.mask_bits & 0xff00_ffff);
                }
                x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 3 => {
                    self.mask_bits = (data[0] as u32) << 24 | (self.mask_bits & 0x00ff_ffff);
                }
                _ => debug!("invalid offset {} (data length {})", offset, data.len()),
            },
            2 => {
                let value = LittleEndian::read_u16(data);
                match offset {
                    MSI_MSG_CTL_OFFSET => {
                        self.msg_ctl = (self.msg_ctl
                            & !(MSI_CTL_ENABLE | MSI_CTL_MULTI_MSG_ENABLE))
                            | value & (MSI_CTL_ENABLE | MSI_CTL_MULTI_MSG_ENABLE);
                    }
                    x if x == msg_data_offset => {
                        self.msg_data = value;
                    }
                    x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() => {
                        self.mask_bits = value as u32 | (self.mask_bits & 0xffff_0000);
                    }
                    x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() + 2 => {
                        self.mask_bits = (value as u32) << 16 | (self.mask_bits & 0x0000_ffff);
                    }
                    _ => debug!("invalid offset {} (data length {})", offset, data.len()),
                }
            }
            4 => {
                let value = LittleEndian::read_u32(data);
                match offset {
                    0x0 => {
                        self.msg_ctl = (self.msg_ctl
                            & !(MSI_CTL_ENABLE | MSI_CTL_MULTI_MSG_ENABLE))
                            | ((value >> 16) as u16 & (MSI_CTL_ENABLE | MSI_CTL_MULTI_MSG_ENABLE));
                    }
                    MSI_MSG_ADDR_LO_OFFSET => {
                        self.msg_addr_lo = value & MSI_MSG_ADDR_LO_MASK;
                    }
                    x if addr_hi_offset.is_some() && x == addr_hi_offset.unwrap() => {
                        self.msg_addr_hi = value;
                    }
                    x if x == msg_data_offset => {
                        self.msg_data = value as u16;
                    }
                    x if mask_bits_offset.is_some() && x == mask_bits_offset.unwrap() => {
                        self.mask_bits = value;
                    }
                    _ => debug!("invalid offset {} (data length {})", offset, data.len()),
                }
            }
            _ => debug!("invalid data length {}", data.len()),
        }

        Ok(())
    }

    // get the pending state from the interrupt source group
    // if the interrupt is in pending state, it's related bit in pending bit will be 1.
    // For example, if the interrupt 1 is in pending state, the second lowest bit in pending bits
    // will be 1.
    fn get_pending_state(&mut self, data: &mut [u8], offset: usize) {
        for ptr in data.iter_mut() {
            *ptr = 0;
        }

        if self.enabled() && self.per_vector_mask() {
            if let Some(group) = self.group.as_ref() {
                let start = offset * 8;
                let end = std::cmp::min((offset + data.len()) * 8, group.len() as usize);

                for idx in start..end {
                    if self.vector_masked(idx as u16)
                        && group.get_pending_state(idx as InterruptIndex)
                    {
                        data[idx / 8 - offset] |= 0x1u8 << (idx % 8);
                    }
                }
            }
        }
    }

    // Calculate message data offset depending on the address being 32 or 64 bits.
    // Calculate upper address offset if the address is 64 bits.
    // Calculate mask bits offset based on the address being 32 or 64 bits
    // and based on the per vector masking being enabled or not.
    fn get_offset_info(&self) -> (u64, Option<u64>, Option<u64>) {
        if self.addr_64_bits() {
            let mask_bits = if self.per_vector_mask() {
                Some(0x10)
            } else {
                None
            };
            // 0xc = cap id (2 bytes) + next cap pointer (2) + lower address offset (4) + higher address offset (8)
            // Some(0x8) = cap id (2 bytes) + next cap pointer (2) + lower address offset (4)
            // mask_bits is None if per vector mask is not opened or 0xc according to the pci spec
            (0xc, Some(0x8), mask_bits)
        } else {
            let mask_bits = if self.per_vector_mask() {
                Some(0xc)
            } else {
                None
            };
            // 0x8 = cap id (2 bytes) + next cap pointer (2) + lower address offset (4)
            // None because there is no upper address offset in 32 bits
            // mask_bits is None if per vector mask is not opened or 0xc according to the pci spec
            (0x8, None, mask_bits)
        }
    }
}

impl PciCapability for MsiCap {
    fn len(&self) -> usize {
        self.size() as usize
    }

    fn set_next_cap(&mut self, next: u8) {
        self.cap_id_next &= 0xff;
        self.cap_id_next |= (next as u16) << 8;
    }

    fn read_u8(&mut self, offset: usize) -> u8 {
        let mut buf = [0u8; 1];

        if let Err(e) = self.read(offset as u64, &mut buf) {
            error!("failed to read PCI MSI capability structure, {}", e);
            fill_config_data(&mut buf);
        }

        buf[0]
    }

    fn read_u16(&mut self, offset: usize) -> u16 {
        let mut buf = [0u8; 2];

        if let Err(e) = self.read(offset as u64, &mut buf) {
            error!("failed to read PCI MSI capability structure, {}", e);
            fill_config_data(&mut buf);
        }

        LittleEndian::read_u16(&buf)
    }

    fn read_u32(&mut self, offset: usize) -> u32 {
        let mut buf = [0u8; 4];

        if let Err(e) = self.read(offset as u64, &mut buf) {
            error!("failed to read PCI MSI capability structure, {}", e);
            fill_config_data(&mut buf);
        }

        LittleEndian::read_u32(&buf)
    }

    fn write_u8(&mut self, offset: usize, value: u8) {
        if let Err(e) = self.write(offset as u64, &[value]) {
            error!("failed to write PCI MSI capability structure, {}", e);
        }
    }

    fn write_u16(&mut self, offset: usize, value: u16) {
        let mut buf = [0u8; 2];
        LittleEndian::write_u16(&mut buf, value);
        if let Err(e) = self.write(offset as u64, &buf) {
            error!("failed to write PCI MSI capability structure, {}", e);
        }
    }

    fn write_u32(&mut self, offset: usize, value: u32) {
        let mut buf = [0u8; 4];
        LittleEndian::write_u32(&mut buf, value);
        if let Err(e) = self.write(offset as u64, &buf) {
            error!("failed to write PCI MSI capability structure, {}", e);
        }
    }

    fn pci_capability_type(&self) -> PciCapabilityId {
        PciCapabilityId::MessageSignalledInterrupts
    }
}

/// Struct to manage PCI Message Signalled Interrupt controller working state.
#[repr(packed)]
#[derive(Clone, Copy, Default, PartialEq)]
pub struct MsiState {
    msg_ctl: u16,
    msg_addr_lo: u32,
    msg_addr_hi: u32,
    msg_data: u16,
    mask_bits: u32,
}

impl MsiState {
    /// Create a new PCI MSI capability state structure.
    pub fn new(mut msg_ctl: u16) -> Self {
        // Initialize as disabled
        msg_ctl &= !MSI_CTL_ENABLE;

        MsiState {
            msg_ctl,
            ..Default::default()
        }
    }

    /// Check whether the PCI MSI capability has been enabled.
    pub fn enabled(&self) -> bool {
        self.msg_ctl & MSI_CTL_ENABLE == MSI_CTL_ENABLE
    }

    /// Get number of vectors supported.
    pub fn num_vectors(&self) -> u16 {
        msi_num_supported_vectors(self.msg_ctl)
    }

    /// Get number of vectors enabled.
    pub fn num_enabled_vectors(&self) -> u16 {
        msi_num_enabled_vectors(self.msg_ctl)
    }

    /// Handle update to the PCI MSI capability structure.
    pub fn synchronize_state<I: InterruptManager>(
        &mut self,
        cap: &MsiCap,
        intr_manager: &mut DeviceInterruptManager<I>,
    ) -> std::io::Result<()> {
        if self.msg_ctl != cap.msg_ctl {
            self.update_msi_ctl(cap.msg_ctl, intr_manager)?;
        }
        if self.msg_addr_lo != cap.msg_addr_lo
            || self.msg_data != cap.msg_data
            || (cap.addr_64_bits() && self.msg_addr_hi != cap.msg_addr_hi)
        {
            self.msg_addr_lo = cap.msg_addr_lo;
            self.msg_addr_hi = cap.msg_addr_hi;
            self.msg_data = cap.msg_data;
            self.update_msi_msg(intr_manager)?;
        }
        if cap.per_vector_mask() && self.mask_bits != cap.mask_bits {
            self.update_msi_mask(cap.mask_bits, intr_manager)?;
        }

        Ok(())
    }

    pub fn disable<I: InterruptManager>(
        &mut self,
        intr_manager: &mut DeviceInterruptManager<I>,
    ) -> std::io::Result<()> {
        if self.enabled() {
            self.update_msi_ctl(self.msg_ctl & !MSI_CTL_ENABLE, intr_manager)?;
        }

        Ok(())
    }

    fn update_msi_ctl<I: InterruptManager>(
        &mut self,
        value: u16,
        intr_manager: &mut DeviceInterruptManager<I>,
    ) -> std::io::Result<()> {
        match (self.enabled(), value & MSI_CTL_ENABLE != 0) {
            (false, true) => {
                if msi_num_enabled_vectors(value) > self.num_vectors() {
                    warn!("guest OS enables too many MSI vectors");
                } else {
                    intr_manager.reset()?;
                    intr_manager.set_working_mode(DeviceInterruptMode::PciMsiIrq)?;
                    for idx in 0..self.num_enabled_vectors() as InterruptIndex {
                        intr_manager.set_msi_high_address(idx, self.msg_addr_hi)?;
                        intr_manager.set_msi_low_address(idx, self.msg_addr_lo)?;
                        intr_manager.set_msi_data(idx, self.msg_data as u32 + idx)?;
                        #[cfg(target_arch = "aarch64")]
                        {
                            intr_manager.set_msi_device_id(idx)?;
                        }
                    }
                    intr_manager.enable()?;

                    // Safe to unwrap() because we have just enabled interrupt successfully.
                    let group = intr_manager.get_group().unwrap();
                    for idx in 0..self.num_enabled_vectors() {
                        if (self.mask_bits >> idx) & MSI_MASK_BIT_MASK != 0 {
                            group.mask(idx as InterruptIndex)?;
                        }
                    }

                    self.msg_ctl = (self.msg_ctl & !(MSI_CTL_ENABLE | MSI_CTL_MULTI_MSG_ENABLE))
                        | (value & (MSI_CTL_ENABLE | MSI_CTL_MULTI_MSG_ENABLE));
                }
            }

            (true, false) => {
                intr_manager.reset()?;
                self.msg_ctl = (self.msg_ctl & !(MSI_CTL_ENABLE | MSI_CTL_MULTI_MSG_ENABLE))
                    | (value & (MSI_CTL_ENABLE | MSI_CTL_MULTI_MSG_ENABLE));
            }

            (true, true) => {
                if msi_num_enabled_vectors(value) != self.num_enabled_vectors() {
                    warn!("guest OS changes enabled vectors after enabling MSI");
                }
            }

            (false, false) => {
                self.msg_ctl = (self.msg_ctl & !(MSI_CTL_ENABLE | MSI_CTL_MULTI_MSG_ENABLE))
                    | (value & (MSI_CTL_ENABLE | MSI_CTL_MULTI_MSG_ENABLE));
            }
        }

        Ok(())
    }

    fn update_msi_msg<I: InterruptManager>(
        &self,
        intr_manager: &mut DeviceInterruptManager<I>,
    ) -> std::io::Result<()> {
        if self.enabled() {
            for idx in 0..self.num_enabled_vectors() as InterruptIndex {
                intr_manager.set_msi_high_address(idx, self.msg_addr_hi)?;
                intr_manager.set_msi_low_address(idx, self.msg_addr_lo)?;
                intr_manager.set_msi_data(idx, self.msg_data as u32 + idx)?;
                intr_manager.update(idx)?;
            }
        }

        Ok(())
    }

    fn update_msi_mask<I: InterruptManager>(
        &mut self,
        mask: u32,
        intr_manager: &mut DeviceInterruptManager<I>,
    ) -> std::io::Result<()> {
        if self.enabled() {
            for idx in 0..self.num_enabled_vectors() {
                match (
                    (self.mask_bits >> idx) & MSI_MASK_BIT_MASK,
                    (mask >> idx) & MSI_MASK_BIT_MASK,
                ) {
                    (0, 1) => {
                        let group = intr_manager.get_group().unwrap();
                        group.mask(idx as InterruptIndex)?;
                    }
                    (1, 0) => {
                        let group = intr_manager.get_group().unwrap();
                        group.unmask(idx as InterruptIndex)?;
                    }
                    _ => {
                        debug!(
                            "mask bit status {} and the idx to mask/unmask {} is the same",
                            self.mask_bits >> idx,
                            mask >> idx
                        );
                    }
                }
            }
        }

        self.mask_bits = mask;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    #[cfg(target_arch = "aarch64")]
    use dbs_arch::gic::create_gic;
    use dbs_device::resources::{DeviceResources, MsiIrqType, Resource};
    use dbs_interrupt::KvmIrqManager;
    use kvm_ioctls::{Kvm, VmFd};

    use super::*;

    fn create_vm_fd() -> VmFd {
        let kvm = Kvm::new().unwrap();
        kvm.create_vm().unwrap()
    }

    fn create_init_resources() -> DeviceResources {
        let mut resources = DeviceResources::new();

        resources.append(Resource::MmioAddressRange {
            base: 0xd000_0000,
            size: 0x10_0000,
        });
        resources.append(Resource::LegacyIrq(0));
        resources.append(Resource::MsiIrq {
            ty: MsiIrqType::GenericMsi,
            base: 0x200,
            size: 0x10,
        });
        resources.append(Resource::MsiIrq {
            ty: MsiIrqType::PciMsi,
            base: 0x100,
            size: 0x20,
        });
        resources.append(Resource::MsiIrq {
            ty: MsiIrqType::PciMsix,
            base: 0x300,
            size: 0x20,
        });

        resources
    }

    fn create_interrupt_manager() -> DeviceInterruptManager<Arc<KvmIrqManager>> {
        let vmfd = Arc::new(create_vm_fd());
        #[cfg(target_arch = "x86_64")]
        assert!(vmfd.create_irq_chip().is_ok());
        #[cfg(target_arch = "aarch64")]
        assert!(create_gic(vmfd.as_ref(), 1).is_ok());
        let intr_mgr = Arc::new(KvmIrqManager::new(vmfd));

        let resource = create_init_resources();
        assert!(intr_mgr.initialize().is_ok());
        DeviceInterruptManager::new(intr_mgr, &resource).unwrap()
    }

    #[test]
    fn test_msi_cap_struct() {
        let cap = MsiCap::new(
            0xa5,
            MSI_CTL_ENABLE | MSI_CTL_64_BITS | MSI_CTL_PER_VECTOR | 0x6,
        );

        assert!(cap.addr_64_bits());
        assert!(cap.per_vector_mask());
        assert!(!cap.enabled());
        assert_eq!(cap.size(), 24);
        assert_eq!(cap.num_vectors(), 8);
        assert_eq!(cap.num_enabled_vectors(), 1);
    }

    #[test]
    fn test_msi_capability() {
        let mut cap = MsiCap::new(
            0xa5,
            MSI_CTL_ENABLE | MSI_CTL_64_BITS | MSI_CTL_PER_VECTOR | 0x6,
        );

        cap.set_next_cap(0xff);
        assert_eq!(cap.read_u8(1), 0xff);
        cap.write_u16(12, 0xa5a5);
        assert_eq!(cap.read_u16(12), 0xa5a5);
        cap.write_u32(12, 0xa5a5a5a4);
        assert_eq!(cap.read_u16(12), 0xa5a4);
        assert_eq!(cap.msg_data, 0xa5a4)
    }

    #[test]
    fn test_msi_state_struct() {
        let flags = MSI_CTL_ENABLE | MSI_CTL_64_BITS | MSI_CTL_PER_VECTOR | 0x6 | 0x20;
        let mut cap = MsiCap::new(0xa5, flags);

        let buf = [0x1u8, 0x0u8, 0x0u8, 0x0u8];
        cap.write(8, &buf).unwrap();
        assert_eq!(cap.msg_addr_hi, 0x1);
        cap.write(16, &buf).unwrap();
        assert_eq!(cap.mask_bits, 0x1);
        cap.write(2, &[flags as u8, (flags >> 8) as u8]).unwrap();
        assert!(cap.enabled());
        let flags2 = flags & !MSI_CTL_ENABLE;

        let mut state = MsiState::new(MSI_CTL_64_BITS | MSI_CTL_PER_VECTOR | 0x6);
        assert!(!state.enabled());
        assert_eq!(state.num_vectors(), 8);
        assert_eq!(state.num_enabled_vectors(), 1);

        // Synchronize state from DISABLED -> ENABLED.
        let mut irq_mgr = create_interrupt_manager();
        state.synchronize_state(&cap, &mut irq_mgr).unwrap();
        assert!(state.enabled());
        assert_eq!(state.num_enabled_vectors(), 4);
        assert!(irq_mgr.is_enabled());
        assert_eq!(irq_mgr.get_working_mode(), DeviceInterruptMode::PciMsiIrq);

        // Test PENDING state
        let group = irq_mgr.get_group();
        cap.set_group(group.clone());
        let mut buf2 = [0u8];
        cap.read(20, &mut buf2).unwrap();
        assert_eq!(buf2[0], 0);
        let eventfd = group.as_ref().unwrap().notifier(0).unwrap();
        eventfd.write(1).unwrap();
        cap.read(20, &mut buf2).unwrap();
        assert_eq!(buf2[0], 0x1);

        // Unmask interrupt and test pending state again
        cap.write(16, &[0u8]).unwrap();
        buf2[0] = 0;
        cap.read(20, &mut buf2).unwrap();
        assert_eq!(buf2[0], 0x0);

        // Synchronize state from ENABLED -> DISABLED.
        cap.write(2, &[flags2 as u8, (flags2 >> 8) as u8]).unwrap();
        assert!(!cap.enabled());
        state.synchronize_state(&cap, &mut irq_mgr).unwrap();
        assert!(!state.enabled());
        assert_eq!(state.num_enabled_vectors(), 4);
        assert!(!irq_mgr.is_enabled());
    }
}
