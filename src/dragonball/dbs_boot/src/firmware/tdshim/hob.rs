// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::TdvfError;
use crate::firmware::*;

use vm_memory::{ByteValued, Bytes, GuestAddress, GuestMemoryMmap};

/// HOB Type
#[repr(u16)]
#[derive(Copy, Clone, Debug, Default)]
enum HobType {
    /// Hand Off
    Handoff = 0x1,
    /// Resource Descriptor
    ResourceDescriptor = 0x3,
    /// Guid Extension
    GuidExtension = 0x4,
    /// Unused
    #[default]
    Unused = 0xfffe,
    /// End Of HOB List
    EndOfHobList = 0xffff,
}

/// HOB header
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct HobHeader {
    r#type: HobType,
    length: u16,
    reserved: u32,
}

/// HOB hand off info table
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct HobHandoffInfoTable {
    header: HobHeader,
    version: u32,
    boot_mode: u32,
    efi_memory_top: u64,
    efi_memory_bottom: u64,
    efi_free_memory_top: u64,
    efi_free_memory_bottom: u64,
    efi_end_of_hob_list: u64,
}

impl HobHandoffInfoTable {
    pub fn new(efi_end_of_hob_list: u64) -> Self {
        HobHandoffInfoTable {
            header: HobHeader {
                r#type: HobType::Handoff,
                length: std::mem::size_of::<HobHandoffInfoTable>() as u16,
                reserved: 0,
            },
            version: 0x9,
            boot_mode: 0,
            efi_memory_top: 0,
            efi_memory_bottom: 0,
            efi_free_memory_top: 0,
            efi_free_memory_bottom: 0,
            efi_end_of_hob_list,
        }
    }
}

/// HOB resource descriptor
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct HobResourceDescriptor {
    header: HobHeader,
    efi_guid_type: EfiGuid,
    resource_type: u32,
    resource_attribute: u32,
    physical_start: u64,
    resource_length: u64,
}

impl HobResourceDescriptor {
    fn new(
        resource_type: u32,
        resource_attribute: u32,
        physical_start: u64,
        resource_length: u64,
    ) -> Self {
        HobResourceDescriptor {
            header: HobHeader {
                r#type: HobType::ResourceDescriptor,
                length: std::mem::size_of::<HobResourceDescriptor>() as u16,
                reserved: 0,
            },
            efi_guid_type: EfiGuid::resource(),
            resource_type,
            resource_attribute,
            physical_start,
            resource_length,
        }
    }
}

/// HOB end
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct HobEnd {
    header: HobHeader,
}

impl HobEnd {
    fn new() -> Self {
        HobEnd {
            header: HobHeader {
                r#type: HobType::EndOfHobList,
                length: std::mem::size_of::<HobEnd>() as u16,
                reserved: 0,
            },
        }
    }
}

/// Efi Guid
#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq)]
struct EfiGuid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl EfiGuid {
    /// RESOURCE_HOB_GUID
    fn resource() -> Self {
        EfiGuid::default()
    }

    /// HOB_PAYLOAD_INFO_GUID
    /// 0xb96fa412, 0x461f, 0x4be3, {0x8c, 0xd, 0xad, 0x80, 0x5a, 0x49, 0x7a, 0xc0
    fn payload() -> Self {
        EfiGuid {
            data1: 0xb96f_a412,
            data2: 0x461f,
            data3: 0x4be3,
            data4: [0x8c, 0xd, 0xad, 0x80, 0x5a, 0x49, 0x7a, 0xc0],
        }
    }

    /// ACPI_TABLE_HOB_GUID
    /// 0x6a0c5870, 0xd4ed, 0x44f4, {0xa1, 0x35, 0xdd, 0x23, 0x8b, 0x6f, 0xc, 0x8d }
    fn acpi() -> Self {
        EfiGuid {
            data1: 0x6a0c_5870,
            data2: 0xd4ed,
            data3: 0x44f4,
            data4: [0xa1, 0x35, 0xdd, 0x23, 0x8b, 0x6f, 0xc, 0x8d],
        }
    }
}

/// Payload image type
#[repr(u32)]
#[derive(Clone, Copy, Default, Debug)]
pub enum PayloadImageType {
    /// Raw executable binary
    #[default]
    ExecutablePayload,
    /// BzImage
    BzImage,
    /// Raw vmlinux kernel in ELF
    RawVmLinux,
}

/// Payload Info
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct PayloadInfo {
    /// Payload image type
    pub image_type: PayloadImageType,
    /// Reserved
    pub reserved: u32,
    /// Entry point for the payload
    pub entry_point: u64,
}

impl PayloadInfo {
    /// Create a new payload info struct
    pub fn new(image_type: PayloadImageType, entry_point: u64) -> Self {
        Self {
            image_type,
            reserved: 0,
            entry_point,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct TdPayloadDescription {
    header: HobHeader,
    efi_guid_type: EfiGuid,
    payload_info: PayloadInfo,
}

impl TdPayloadDescription {
    fn new(payload: PayloadInfo) -> Self {
        TdPayloadDescription {
            header: HobHeader {
                r#type: HobType::GuidExtension,
                length: std::mem::size_of::<TdPayloadDescription>() as u16,
                reserved: 0,
            },
            efi_guid_type: EfiGuid::payload(),
            payload_info: payload,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct AcpiDescription {
    header: HobHeader,
    efi_guid_type: EfiGuid,
}

impl AcpiDescription {
    fn new(length: u16) -> Self {
        AcpiDescription {
            header: HobHeader {
                r#type: HobType::GuidExtension,
                length,
                reserved: 0,
            },
            // ACPI_TABLE_HOB_GUID
            efi_guid_type: EfiGuid::acpi(),
        }
    }
}

unsafe impl ByteValued for HobHeader {}
unsafe impl ByteValued for HobHandoffInfoTable {}
unsafe impl ByteValued for HobResourceDescriptor {}
unsafe impl ByteValued for TdPayloadDescription {}
unsafe impl ByteValued for AcpiDescription {}
unsafe impl ByteValued for HobEnd {}

/// TD HOB
pub struct TdHob {
    start_offset: u64,
    current_offset: u64,
}

fn align_hob(v: u64) -> u64 {
    v.div_ceil(8) * 8
}

impl TdHob {
    /// Update offset to align with 8 bytes
    fn update_offset<T>(&mut self) {
        self.current_offset = align_hob(self.current_offset + std::mem::size_of::<T>() as u64)
    }

    /// Add resource to HOB list
    fn add_resource(
        &mut self,
        mem: &GuestMemoryMmap,
        physical_start: u64,
        resource_length: u64,
        resource_type: u32,
        resource_attribute: u32,
    ) -> Result<(), TdvfError> {
        let resource_descriptor = HobResourceDescriptor::new(
            resource_type,
            resource_attribute,
            physical_start,
            resource_length,
        );

        mem.write_obj(resource_descriptor, GuestAddress(self.current_offset))
            .map_err(TdvfError::WriteHobError)?;
        self.update_offset::<HobResourceDescriptor>();
        Ok(())
    }

    /// Start writing HOB list
    pub fn start(offset: u64) -> TdHob {
        // Leave a gap to place the HandoffTable at the start as it can only be filled in later
        let mut hob = TdHob {
            start_offset: offset,
            current_offset: offset,
        };
        hob.update_offset::<HobHandoffInfoTable>();
        hob
    }

    /// Finish writing HOB list
    pub fn finish(&mut self, mem: &GuestMemoryMmap) -> Result<(), TdvfError> {
        // Write end
        let end = HobEnd::new();
        mem.write_obj(end, GuestAddress(self.current_offset))
            .map_err(TdvfError::WriteHobError)?;
        self.update_offset::<HobEnd>();

        // Write handoff, delayed as it needs end of HOB list
        let efi_end_of_hob_list = self.current_offset;
        let handoff = HobHandoffInfoTable::new(efi_end_of_hob_list);
        mem.write_obj(handoff, GuestAddress(self.start_offset))
            .map_err(TdvfError::WriteHobError)
    }

    /// Add memory resource
    pub fn add_memory_resource(
        &mut self,
        mem: &GuestMemoryMmap,
        physical_start: u64,
        resource_length: u64,
        ram: bool,
    ) -> Result<(), TdvfError> {
        self.add_resource(
            mem,
            physical_start,
            resource_length,
            if ram {
                EFI_RESOURCE_MEMORY_UNACCEPTED
            } else {
                EFI_RESOURCE_SYSTEM_MEMORY
            },
            EFI_RESOURCE_ATTRIBUTE_PRESENT
                | EFI_RESOURCE_ATTRIBUTE_INITIALIZED
                | EFI_RESOURCE_ATTRIBUTE_TESTED,
        )
    }

    /// Add mmio resource
    pub fn add_mmio_resource(
        &mut self,
        mem: &GuestMemoryMmap,
        physical_start: u64,
        resource_length: u64,
    ) -> Result<(), TdvfError> {
        self.add_resource(
            mem,
            physical_start,
            resource_length,
            EFI_RESOURCE_MEMORY_MAPPED_IO,
            EFI_RESOURCE_ATTRIBUTE_PRESENT
                | EFI_RESOURCE_ATTRIBUTE_INITIALIZED
                | EFI_RESOURCE_ATTRIBUTE_UNCACHEABLE,
        )
    }

    /// Add payload
    pub fn add_payload(
        &mut self,
        mem: &GuestMemoryMmap,
        payload_info: PayloadInfo,
    ) -> Result<(), TdvfError> {
        let payload = TdPayloadDescription::new(payload_info);
        mem.write_obj(payload, GuestAddress(self.current_offset))
            .map_err(TdvfError::WriteHobError)?;
        self.update_offset::<TdPayloadDescription>();
        Ok(())
    }

    /// Add ACPI table
    pub fn add_acpi_table(
        &mut self,
        mem: &GuestMemoryMmap,
        table_content: &[u8],
    ) -> Result<(), TdvfError> {
        // We already know the HobGuidType size is 8 bytes multiple, but we
        // need the total size to be 8 bytes multiple. That is why the ACPI
        // table size must be 8 bytes multiple as well.
        let length = std::mem::size_of::<AcpiDescription>() as u16
            + align_hob(table_content.len() as u64) as u16;

        let hob_guid_type = AcpiDescription::new(length);

        mem.write_obj(hob_guid_type, GuestAddress(self.current_offset))
            .map_err(TdvfError::WriteHobError)?;
        let current_offset = self.current_offset + std::mem::size_of::<AcpiDescription>() as u64;

        // In case the table is quite large, let's make sure we can handle
        // retrying until everything has been correctly copied.
        let mut offset: usize = 0;
        loop {
            let bytes_written = mem
                .write(
                    &table_content[offset..],
                    GuestAddress(current_offset + offset as u64),
                )
                .map_err(TdvfError::WriteHobError)?;
            offset += bytes_written;
            if offset >= table_content.len() {
                break;
            }
        }
        self.current_offset += length as u64;

        Ok(())
    }
}
