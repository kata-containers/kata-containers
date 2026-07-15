// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::sdt::Sdt;
use vm_memory::ByteValued;

const IOAPIC_START: u32 = 0xfec0_0000;
const APIC_START: u32 = 0xfee0_0000;

const MADT_CPU_ENABLE_FLAG: usize = 0;

#[repr(u8)]
#[derive(Default, Copy, Clone)]
pub enum MadtEntryType {
    #[default]
    LocalApic,
    Ioapic,
    InterruptSourceOverride,
    LocalX2Apic = 9,
}

#[repr(C, packed)]
#[derive(Default, Copy, Clone)]
pub struct MadtBody {
    pub apic_address: u32,
    pub flags: u32,
}

impl MadtBody {
    pub fn new(apic_address: u32, flags: u32) -> Self {
        Self {
            apic_address,
            flags,
        }
    }
}

#[repr(C, packed)]
#[derive(Default, Copy, Clone)]
pub struct MadtEntryLocalApic {
    pub r#type: MadtEntryType,
    pub length: u8,
    pub processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

impl MadtEntryLocalApic {
    pub fn new(processor_id: u8, flags: u32) -> Self {
        Self {
            r#type: MadtEntryType::LocalApic,
            length: 8,
            processor_id,
            apic_id: processor_id,
            flags,
        }
    }
}

#[repr(C, packed)]
#[derive(Default, Copy, Clone)]
pub struct MadtEntryIoapic {
    pub r#type: MadtEntryType,
    pub length: u8,
    pub ioapic_id: u8,
    pub reserved: u8,
    pub ioapic_address: u32,
    pub gsi_base: u32,
}

impl MadtEntryIoapic {
    pub fn new(ioapic_id: u8, ioapic_address: u32, gsi_base: u32) -> Self {
        Self {
            r#type: MadtEntryType::Ioapic,
            length: 12,
            ioapic_id,
            reserved: 0,
            ioapic_address,
            gsi_base,
        }
    }
}

#[repr(C, packed)]
#[derive(Default, Copy, Clone)]
pub struct MadtEntryIntrSrcOverride {
    pub r#type: MadtEntryType,
    pub length: u8,
    pub bus_source: u8,
    pub irq_source: u8,
    pub gsi: u32,
    pub flags: u16,
}

impl MadtEntryIntrSrcOverride {
    pub fn new(bus_source: u8, irq_source: u8, gsi: u32, flags: u16) -> Self {
        Self {
            r#type: MadtEntryType::InterruptSourceOverride,
            length: 10,
            bus_source,
            irq_source,
            gsi,
            flags,
        }
    }
}

#[repr(C, packed)]
#[derive(Default, Copy, Clone)]
pub struct MadtEntryLocalX2Apic {
    r#type: MadtEntryType,
    length: u8,
    reserved: u16,
    x2apic_id: u32,
    flags: u32,
    processor_id: u32,
}

impl MadtEntryLocalX2Apic {
    pub fn new(processor_id: u32, flags: u32) -> Self {
        Self {
            r#type: MadtEntryType::LocalX2Apic,
            length: 16,
            reserved: 0,
            // TODO: Calculate x2apic id from processor id
            x2apic_id: processor_id,
            flags,
            processor_id,
        }
    }
}

unsafe impl ByteValued for MadtBody {}
unsafe impl ByteValued for MadtEntryLocalApic {}
unsafe impl ByteValued for MadtEntryIoapic {}
unsafe impl ByteValued for MadtEntryIntrSrcOverride {}
unsafe impl ByteValued for MadtEntryLocalX2Apic {}

pub fn create_madt_table(max_vcpus: u8, boot_vcpus: u8) -> Sdt {
    let mut madt = Sdt::new(*b"APIC", 36, 5);
    madt.append_slice(MadtBody::new(APIC_START, 0).as_slice());

    for cpu_id in 0..max_vcpus {
        madt.append_slice(
            MadtEntryLocalApic::new(
                cpu_id,
                if cpu_id < boot_vcpus {
                    1 << MADT_CPU_ENABLE_FLAG
                } else {
                    0
                },
            )
            .as_slice(),
        );
    }

    madt.append_slice(MadtEntryIoapic::new(0, IOAPIC_START, 0).as_slice());

    madt.append_slice(MadtEntryIntrSrcOverride::new(0, 2, 2, 0).as_slice());

    madt
}
