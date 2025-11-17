// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

/// TDVF related functionality.
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use thiserror::Error;

/// TDVF related errors.
#[derive(Error, Debug)]
pub enum TdvfError {
    /// Failed to read TDVF descriptor.
    #[error("Failed read TDVF descriptor: {0}")]
    ReadDescriptor(#[source] std::io::Error),
    /// Failed to read TDVF descriptor offset.
    #[error("Failed read TDVF descriptor offset: {0}")]
    ReadDescriptorOffset(#[source] std::io::Error),
    /// Invalid descriptor signature.
    #[error("Invalid descriptor signature")]
    InvalidDescriptorSignature,
    /// Invalid descriptor size.
    #[error("Invalid descriptor size")]
    InvalidDescriptorSize,
    /// Invalid descriptor version.
    #[error("Invalid descriptor version")]
    InvalidDescriptorVersion,
}

/// TDVF_DESCRIPTOR
#[repr(packed)]
pub struct TdvfDescriptor {
    signature: [u8; 4],
    length: u32,
    version: u32,
    num_sections: u32, // NumberOfSectionEntry
}

#[repr(packed)]
#[derive(Clone, Copy, Default, Debug)]
/// DVF_SECTION
pub struct TdvfSection {
    /// Data offset
    pub data_offset: u32,
    /// RawDataSize
    pub data_size: u32,
    /// MemoryAddress
    pub address: u64,
    /// MemoryDataSize
    pub size: u64,
    /// TDVF Section Type
    pub r#type: TdvfSectionType,
    /// TDVF Attribute
    pub attributes: u32,
}

#[repr(u32)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
/// TDVF Section Type
pub enum TdvfSectionType {
    /// BFV section type
    Bfv,
    /// CFV section type
    Cfv,
    /// TD Hob offser
    TdHob,
    /// Temp Memory
    TempMem,
    /// PermMem
    PermMem,
    /// Payload
    Payload,
    /// Payload Param
    PayloadParam,
    /// Reserved
    #[default]
    Reserved = 0xffffffff,
}

/// Parse tdx section.
///
/// #Arguments
/// * `file` - The tdshim image file.
pub fn parse_tdvf_sections(file: &mut File) -> std::result::Result<Vec<TdvfSection>, TdvfError> {
    // The 32-bit offset to the TDVF metadata is located 32 bytes from
    // the end of the file.
    // See "TDVF Metadata Pointer" in "TDX Virtual Firmware Design Guide
    file.seek(SeekFrom::End(-0x20))
        .map_err(TdvfError::ReadDescriptorOffset)?;

    let mut descriptor_offset: [u8; 4] = [0; 4];
    file.read_exact(&mut descriptor_offset)
        .map_err(TdvfError::ReadDescriptorOffset)?;
    let descriptor_offset = u32::from_le_bytes(descriptor_offset) as u64;

    file.seek(SeekFrom::Start(descriptor_offset))
        .map_err(TdvfError::ReadDescriptor)?;

    let mut descriptor: TdvfDescriptor = unsafe { std::mem::zeroed() };
    // Safe as we read exactly the size of the descriptor header
    file.read_exact(unsafe {
        std::slice::from_raw_parts_mut(
            &mut descriptor as *mut _ as *mut u8,
            std::mem::size_of::<TdvfDescriptor>(),
        )
    })
    .map_err(TdvfError::ReadDescriptor)?;

    if &descriptor.signature != b"TDVF" {
        return Err(TdvfError::InvalidDescriptorSignature);
    }

    if descriptor.length as usize
        != std::mem::size_of::<TdvfDescriptor>()
            + std::mem::size_of::<TdvfSection>() * descriptor.num_sections as usize
    {
        return Err(TdvfError::InvalidDescriptorSize);
    }

    if descriptor.version != 1 {
        return Err(TdvfError::InvalidDescriptorVersion);
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
    .map_err(TdvfError::ReadDescriptor)?;

    Ok(sections)
}
