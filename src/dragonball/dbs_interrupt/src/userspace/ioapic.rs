// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use bilge::prelude::*;

use std::convert::TryFrom;

use super::InterruptIndex;

/// Base physical address of IOAPIC range
pub const IOAPIC_BASE: u32 = 0xfec0_0000;
/// Size of IOAPIC range
pub const IOAPIC_SIZE: u32 = 0x1000;

/// Base address for IOAPIC index register (direct)
pub const IOAPIC_IOREGSEL_BASE: u32 = IOAPIC_BASE;
/// Base address for IOAPIC data registers (direct)
pub const IOAPIC_IOWIN_BASE: u32 = IOAPIC_BASE + 0x10;

/// Index for IOAPIC ID register (indirect)
pub const IOAPIC_IOAPICID_INDEX: u8 = 0x00;
/// Index for IOAPIC version register (indirect)
pub const IOAPIC_IOAPICVER_INDEX: u8 = 0x01;
/// Index for IOAPIC arbitration register (indirect)
pub const IOAPIC_IOAPICARB_INDEX: u8 = 0x02;
/// Start index for IOAPIC redirection table (indirect)
pub const IOAPIC_REDIR_TABLE_START_INDEX: u8 = 0x10;
/// End index for IOAPIC redirection table (indirect)
pub const IOAPIC_REDIR_TABLE_END_INDEX: u8 = 0x3f;

/// Max number of userspace IOAPIC redirection entries
pub const IOAPIC_MAX_NR_REDIR_ENTRIES: InterruptIndex =
    ((IOAPIC_REDIR_TABLE_END_INDEX - IOAPIC_REDIR_TABLE_START_INDEX + 1) >> 1) as InterruptIndex;
/// Default number of userspace IOAPIC redirection entries
pub const IOAPIC_DEFAULT_NR_REDIR_ENTRIES: InterruptIndex = IOAPIC_MAX_NR_REDIR_ENTRIES;

/// Default IOAPIC version
pub const IOAPIC_DEFAULT_VERSION: u8 = 0x20;

/// MSI message base address
pub const MSI_BASE_ADDR: u12 = u12::from_u16(0xfee);

/// Bits for IOREGSEL register
#[bitsize(32)]
#[derive(DebugBits, FromBits, Default, Clone)]
pub(super) struct IoRegSel {
    /// Bits 7:0. Selected indirect register index to visit
    pub(super) register_index: u8,
    /// Bits 31:8. Reserved
    pub(super) reserved: u24,
}

/// Bits for IOAPICID register
#[bitsize(32)]
#[derive(DebugBits, FromBits, Default, Clone)]
pub(super) struct IoapicId {
    /// Bits 13:0. Reserved
    pub(super) reserved_2: u14,
    /// Bit 14. Level trigger status
    pub(super) lts: bool,
    /// Bit 15. Delivery type
    pub(super) delivery_type: bool,
    /// Bits 23:16. Reserved
    pub(super) reserved_1: u8,
    /// Bits 31:24. IOAPIC ID
    pub(super) id: u8,
}

/// Bits for IOAPICVER register
#[bitsize(32)]
#[derive(DebugBits, FromBits, Default, Clone)]
pub(super) struct IoapicVer {
    /// Bits 7:0. IOAPIC version
    pub(super) version: u8,
    /// Bits 14:8. Reserved
    pub(super) reserved_2: u7,
    /// Bit 15. Support for Pin Assertion Register
    pub(super) prq: bool,
    /// Bits 23:16. Max index for redirection entry (Number of redirection entries - 1)
    pub(super) entries: u8,
    /// Bits 31:24. Reserved
    pub(super) reserved_1: u8,
}

/// Bits for IOAPICARB register
#[bitsize(32)]
#[derive(DebugBits, FromBits, Default, Clone)]
pub(super) struct IoapicArb {
    /// Bits 23:0. Reserved
    pub(super) reserved_2: u24,
    /// Bits 27:24. Arbitration ID
    pub(super) arbitration: u4,
    /// Bits 31:28. Reserved
    pub(super) reserved_1: u4,
}

/// Bits for IOAPIC redirection entry
#[bitsize(64)]
#[derive(DebugBits, FromBits, Default, Clone)]
pub(super) struct IoapicRedirEntry {
    /// Low word
    pub(super) low: IoapicRedirEntryLow,
    /// High word
    pub(super) high: IoapicRedirEntryHigh,
}

/// Low word of IOAPIC redirection entry
#[bitsize(32)]
#[derive(DebugBits, FromBits, Default, Clone)]
pub(super) struct IoapicRedirEntryLow {
    /// Bits 7:0. Interrupt vector
    pub(super) vector: u8,
    /// Bits 10:8. Delivery mode
    pub(super) delivery_mode: u3,
    /// Bit 11. Destination mode, 0 = physical, 1 = logical
    pub(super) dest_mode_logical: bool,
    /// Bit 12. Delivery status, 0 = idle, 1 = send pending. Read-only
    pub(super) delivery_status: bool,
    /// Bit 13. 0 = active high, 1 = active low
    pub(super) active_low: bool,
    /// Bit 14. Remote IRR level trigger status. Read-only
    pub(super) irr: bool,
    /// Bit 15. Trigger mode. 0 = edge, 1 = level
    pub(super) is_level: bool,
    /// Bit 16. 0 = interrupt unmasked, 1 = interrupt masked
    pub(super) masked: bool,
    /// Bits 31:17. Reserved
    pub(super) reserved_0: u15,
}

/// High word of IOAPIC redirection entry
#[bitsize(32)]
#[derive(DebugBits, FromBits, Default, Clone)]
pub(super) struct IoapicRedirEntryHigh {
    /// Bits 16:0. Reserved
    pub(super) reserved_1: u17,
    /// Bits 23:17. High bits (8-14) for virtual APIC ID
    pub(super) virt_destid_8_14: u7,
    /// Bits 31:24. Low bits (0-7) for APIC ID
    pub(super) destid_0_7: u8,
}

/// Low word of address field for MSI message
#[bitsize(32)]
#[derive(DebugBits, FromBits, Default, Clone)]
pub(super) struct MsiAddressLow {
    /// Bits 1:0. Reserved
    pub(super) reserved_0: u2,
    /// Bit 2. Destination mode, 0 = physical, 1 = logical
    pub(super) dest_mode_logical: bool,
    /// Bit 3. Redirection hint
    pub(super) redirect_hint: bool,
    /// Bit 4. Reserved
    pub(super) reserved_1: bool,
    /// Bits 11:5. High bits (8-14) for virtual APIC ID
    pub(super) virt_destid_8_14: u7,
    /// Bits 19:12. Low bits (0-7) for APIC ID
    pub(super) destid_0_7: u8,
    /// Bits 31:20 Base address. (Should be 0xfee)
    pub(super) base_address: u12,
}

/// High word of address field for MSI message
#[bitsize(32)]
#[derive(DebugBits, FromBits, Default, Clone)]
pub(super) struct MsiAddressHigh {
    /// Bits 7:0. Reserved
    pub(super) reserved: u8,
    /// Bits 31:8. High bits (8-31) for APIC ID
    pub(super) destid_8_31: u24,
}

/// Data field for MSI message
#[bitsize(32)]
#[derive(DebugBits, FromBits, Default, Clone)]
pub(super) struct MsiData {
    /// Bits 7:0. Interrupt vector
    pub(super) vector: u8,
    /// Bits 10:8. Delivery mode
    pub(super) delivery_mode: u3,
    /// Bit 11. Destination mode, 0 = physical, 1 = logical
    pub(super) dest_mode_logical: bool,
    /// Bits 13:12. Reserved
    pub(super) reserved_0: u2,
    /// Bit 14. 0 = active high, 1 = active low
    pub(super) active_low: bool,
    /// Bit 15. Trigger mode. 0 = edge, 1 = level
    pub(super) is_level: bool,
    /// Bits 31:16. Reserved
    pub(super) reserved_1: u16,
}
