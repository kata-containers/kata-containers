// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(missing_docs)]

#[cfg(target_arch = "x86_64")]
/// Structs and utilities for tdshim
pub mod tdshim;

pub const EFI_RESOURCE_SYSTEM_MEMORY: u32 = 0x00;
pub const EFI_RESOURCE_MEMORY_MAPPED_IO: u32 = 0x01;
pub const EFI_RESOURCE_MEMORY_UNACCEPTED: u32 = 0x07;

pub const EFI_RESOURCE_ATTRIBUTE_PRESENT: u32 = 0x0000_0001;
pub const EFI_RESOURCE_ATTRIBUTE_INITIALIZED: u32 = 0x0000_0002;
pub const EFI_RESOURCE_ATTRIBUTE_TESTED: u32 = 0x0000_0004;
pub const EFI_RESOURCE_ATTRIBUTE_UNCACHEABLE: u32 = 0x0000_0400;

/// Firmware types
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FirmwareType {
    /// Tdshim
    #[cfg(target_arch = "x86_64")]
    Tdshim,
}
