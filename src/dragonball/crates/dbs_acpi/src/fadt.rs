// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::sdt::{GenericAddress, Sdt};
use vm_memory::ByteValued;

#[repr(C, packed)]
#[derive(Default, Copy, Clone)]
#[allow(non_snake_case)]
pub struct FadtBody {
    pub FirmwareCtrl: u32,
    pub Dsdt: u32,
    pub Reserved: u8,
    pub PreferredPowerManagementProfile: u8,
    pub SCI_Interrupt: u16,
    pub SMI_CommandPort: u32,
    pub AcpiEnable: u8,
    pub AcpiDisable: u8,
    pub S4BIOS_REQ: u8,
    pub PSTATE_Control: u8,
    pub PM1aEventBlock: u32,
    pub PM1bEventBlock: u32,
    pub PM1aControlBlock: u32,
    pub PM1bControlBlock: u32,
    pub PM2ControlBlock: u32,
    pub PMTimerBlock: u32,
    pub GPE0Block: u32,
    pub GPE1Block: u32,
    pub PM1EventLength: u8,
    pub PM1ControlLength: u8,
    pub PM2ControlLength: u8,
    pub PMTimerLength: u8,
    pub GPE0Length: u8,
    pub GPE1Length: u8,
    pub GPE1Base: u8,
    pub CStateControl: u8,
    pub WorstC2Latency: u16,
    pub WorstC3Latency: u16,
    pub FlushSize: u16,
    pub FlushStride: u16,
    pub DutyOffset: u8,
    pub DutyWidth: u8,
    pub DayAlarm: u8,
    pub MonthAlarm: u8,
    pub Century: u8,
    pub BootArchitectureFlags: u16,
    pub Reserved2: u8,
    pub Flags: u32,
    pub ResetReg: GenericAddress,
    pub ResetValue: u8,
    pub ArmBootArch: u16,
    pub FadtMinorVersion: u8,
    pub X_FirmwareControl: u64,
    pub X_Dsdt: u64,
    pub X_PM1aEventBlock: GenericAddress,
    pub X_PM1bEventBlock: GenericAddress,
    pub X_PM1aControlBlock: GenericAddress,
    pub X_PM1bControlBlock: GenericAddress,
    pub X_PM2ControlBlock: GenericAddress,
    pub X_PMTimerBlock: GenericAddress,
    pub X_GPE0Block: GenericAddress,
    pub X_GPE1Block: GenericAddress,
    pub SleepControlReg: GenericAddress,
    pub SleepStatusReg: GenericAddress,
    pub HypervisorVendorIdentity: u64,
}

unsafe impl ByteValued for FadtBody {}

impl FadtBody {
    pub fn new() -> Self {
        FadtBody {
            SCI_Interrupt: 9,

            PM1aEventBlock: 0xb000,
            PM1aControlBlock: 0xb004,
            PMTimerBlock: 0xb008,
            GPE0Block: 0xb020,

            PM1EventLength: 4,
            PM1ControlLength: 2,
            PMTimerLength: 4,
            GPE0Length: 2,

            BootArchitectureFlags: 1,
            Flags: (1 << 0) | (1 << 8) | (1 << 9) | (1 << 10),

            X_PM1aEventBlock: GenericAddress {
                address_space_id: 1,
                register_bit_width: 32,
                register_bit_offset: 0,
                access_size: 3,
                address: 0xb000,
            },
            X_PM1aControlBlock: GenericAddress {
                address_space_id: 1,
                register_bit_width: 16,
                register_bit_offset: 0,
                access_size: 2,
                address: 0xb004,
            },
            X_PMTimerBlock: GenericAddress {
                address_space_id: 1,
                register_bit_width: 32,
                register_bit_offset: 0,
                access_size: 3,
                address: 0xb008,
            },
            ..Default::default()
        }
    }
}

pub fn create_fadt_table() -> Sdt {
    let mut fadt = Sdt::new(*b"FACP", 36, 6);
    fadt.append_slice(FadtBody::new().as_slice());

    fadt
}
