// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

/// Hob related functionality.
use vm_memory::{ByteValued, Bytes, GuestAddress, GuestMemoryError, GuestMemoryMmap};

/// HOB Type
#[repr(u16)]
#[derive(Copy, Clone, Debug)]
enum HobType {
    /// Hand Off
    Handoff = 0x1,
    /// Resource Descriptor
    ResourceDescriptor = 0x3,
    /// Guid Extension
    GuidExtension = 0x4,
    /// Unused
    Unused = 0xfffe,
    /// End Of HOB List
    EndOfHobList = 0xffff,
}
/// Default
impl Default for HobType {
    fn default() -> Self {
        HobType::Unused
    }
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

#[repr(u32)]
#[derive(Clone, Copy, Default, Debug)]
pub enum PayloadImageType {
    #[default]
    ExecutablePayload,
    BzImage,
    RawVmLinux,
}
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct PayloadInfo {
    pub image_type: PayloadImageType,
    pub entry_point: u64,
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

unsafe impl ByteValued for HobHeader {}
unsafe impl ByteValued for HobHandoffInfoTable {}
unsafe impl ByteValued for HobResourceDescriptor {}
unsafe impl ByteValued for TdPayloadDescription {}
unsafe impl ByteValued for HobEnd {}

/// TD HOB
pub struct TdHob {
    start_offset: u64,
    current_offset: u64,
}

fn align_hob(v: u64) -> u64 {
    (v + 7) / 8 * 8
}

impl TdHob {
    /// update offset to align with 8 bytes
    fn update_offset<T>(&mut self) {
        self.current_offset = align_hob(self.current_offset + std::mem::size_of::<T>() as u64)
    }

    /// start wirting hot list
    pub fn start(offset: u64) -> TdHob {
        // Leave a gap to place the HandoffTable at the start as it can only be filled in later
        let mut hob = TdHob {
            start_offset: offset,
            current_offset: offset,
        };
        hob.update_offset::<HobHandoffInfoTable>();
        hob
    }

    /// finish writing hot list
    pub fn finish(&mut self, mem: &GuestMemoryMmap) -> Result<(), GuestMemoryError> {
        // Write end
        let end = HobEnd::new();
        mem.write_obj(end, GuestAddress(self.current_offset))?;
        self.update_offset::<HobEnd>();

        // Write handoff, delayed as it needs end of HOB list
        let efi_end_of_hob_list = self.current_offset;
        let handoff = HobHandoffInfoTable::new(efi_end_of_hob_list);
        mem.write_obj(handoff, GuestAddress(self.start_offset))
    }

    /// Add resource to TD HOB
    pub fn add_resource(
        &mut self,
        mem: &GuestMemoryMmap,
        physical_start: u64,
        resource_length: u64,
        resource_type: u32,
        resource_attribute: u32,
    ) -> Result<(), GuestMemoryError> {
        let resource_descriptor = HobResourceDescriptor::new(
            resource_type,
            resource_attribute,
            physical_start,
            resource_length,
        );

        mem.write_obj(resource_descriptor, GuestAddress(self.current_offset))?;
        self.update_offset::<HobResourceDescriptor>();
        Ok(())
    }

    /// Add memory resource
    pub fn add_memory_resource(
        &mut self,
        mem: &GuestMemoryMmap,
        physical_start: u64,
        resource_length: u64,
        ram: bool,
    ) -> Result<(), GuestMemoryError> {
        self.add_resource(
            mem,
            physical_start,
            resource_length,
            if ram {
                0x7 // EFI_RESOURCE_MEMORY_UNACCEPT
            } else {
                0x0 // EFI_RESOURCE_SYSTEM_MEMORY
            },
            // TODO:
            // QEMU currently fills it in like this:
            // EFI_RESOURCE_ATTRIBUTE_PRESENT | EFI_RESOURCE_ATTRIBUTE_INITIALIZED|EFI_RESOURCE_ATTRIBUTE_ENCRYPTED  | EFI_RESOURCE_ATTRIBUTE_TESTED
            // which differs from the spec (due to TDVF implementation issue?)
            0x07,
        )
    }

    /// Add mmio resource
    pub fn add_mmio_resource(
        &mut self,
        mem: &GuestMemoryMmap,
        physical_start: u64,
        resource_length: u64,
    ) -> Result<(), GuestMemoryError> {
        self.add_resource(
            mem,
            physical_start,
            resource_length,
            0x1,   // EFI_RESOURCE_MEMORY_MAPPED_IO
            0x403, // EFI_RESOURCE_ATTRIBUTE_PRESENT | EFI_RESOURCE_ATTRIBUTE_INITIALIZED | EFI_RESOURCE_ATTRIBUTE_UNCACHEABLE
        )
    }

    /// Add payload
    pub fn add_payload(
        &mut self,
        mem: &GuestMemoryMmap,
        payload_info: PayloadInfo,
    ) -> Result<(), GuestMemoryError> {
        let payload = TdPayloadDescription::new(payload_info);
        mem.write_obj(payload, GuestAddress(self.current_offset))?;
        self.update_offset::<TdPayloadDescription>();
        Ok(())
    }
}
