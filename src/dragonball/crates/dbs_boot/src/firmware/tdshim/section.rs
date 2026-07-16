// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::TdvfError;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap};

/// TDVF descriptor
#[repr(C, packed)]
pub struct TdvfDescriptor {
    signature: [u8; 4],
    length: u32,
    version: u32,
    num_sections: u32,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Default, Debug)]
/// TDVF section
pub struct TdvfSection {
    /// Data offset
    pub data_offset: u32,
    /// Raw data size
    pub data_size: u32,
    /// Guest memory address
    pub address: u64,
    /// Memory data size
    pub size: u64,
    /// TDVF section type
    pub r#type: TdvfSectionType,
    /// TDVF attributes
    pub attributes: u32,
}

#[repr(u32)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
/// TDVF section type
pub enum TdvfSectionType {
    /// BFV section type
    Bfv,
    /// CFV section type
    Cfv,
    /// TD HOB
    TdHob,
    /// Temp memory
    TempMem,
    /// Permanent memory
    PermMem,
    /// Payload
    Payload,
    /// Payload Parameters
    PayloadParam,
    /// Reserved
    #[default]
    Reserved = 0xffffffff,
}

/// Parse TDVF sections and return a list of categorized sections
///
/// #Arguments
/// * `file` - The tdshim image file.
pub fn parse_tdvf_sections(file: &mut File) -> Result<Vec<TdvfSection>, TdvfError> {
    // The 32-bit offset to the TDVF metadata is located 32 bytes from
    // the end of the file.
    // See "TDVF Metadata Pointer" in "TDX Virtual Firmware Design Guide
    file.seek(SeekFrom::End(-0x20))
        .map_err(TdvfError::TdshimFileError)?;

    let mut descriptor_offset: [u8; 4] = [0; 4];
    file.read_exact(&mut descriptor_offset)
        .map_err(TdvfError::TdshimFileError)?;
    let descriptor_offset = u32::from_le_bytes(descriptor_offset) as u64;

    file.seek(SeekFrom::Start(descriptor_offset))
        .map_err(TdvfError::TdshimFileError)?;

    let mut descriptor: TdvfDescriptor = unsafe { std::mem::zeroed() };
    // Safe as we read exactly the size of the descriptor header
    file.read_exact(unsafe {
        std::slice::from_raw_parts_mut(
            &mut descriptor as *mut _ as *mut u8,
            std::mem::size_of::<TdvfDescriptor>(),
        )
    })
    .map_err(TdvfError::TdshimFileError)?;

    if &descriptor.signature != b"TDVF" {
        return Err(TdvfError::TdvfDescriptorError(
            "Invalid descriptor signature",
        ));
    }

    if descriptor.length as usize
        != std::mem::size_of::<TdvfDescriptor>()
            + std::mem::size_of::<TdvfSection>() * descriptor.num_sections as usize
    {
        return Err(TdvfError::TdvfDescriptorError("Invalid descriptor length"));
    }

    if descriptor.version != 1 {
        return Err(TdvfError::TdvfDescriptorError("Invalid descriptor version"));
    }

    let mut sections = Vec::new();
    sections.resize_with(descriptor.num_sections as usize, TdvfSection::default);

    // Safe as we read exactly the advertised sections
    file.read_exact(unsafe {
        std::slice::from_raw_parts_mut(
            sections.as_mut_ptr() as *mut u8,
            descriptor.num_sections as usize * std::mem::size_of::<TdvfSection>(),
        )
    })
    .map_err(TdvfError::TdshimFileError)?;

    Ok(sections)
}

/// Load a TDVF section to guest memory
///
/// #Arguments
/// * `file` - The tdshim image file.
/// * `section` - The metadata of target section.
/// * `mem` - Guest memory to load TDVF section to.
pub fn load_tdvf_section(
    file: &mut File,
    section: &TdvfSection,
    mem: &GuestMemoryMmap,
) -> Result<(), TdvfError> {
    file.seek(SeekFrom::Start(section.data_offset as u64))
        .map_err(TdvfError::TdshimFileError)?;

    mem.read_volatile_from(
        GuestAddress(section.address),
        file,
        section.data_size as usize,
    )
    .map_err(TdvfError::LoadTdvfSectionError)?;

    Ok(())
}
