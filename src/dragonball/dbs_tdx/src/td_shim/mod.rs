// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod metadata;
pub use metadata::*;

pub mod hob;
pub use hob::*;

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

    /// Failed to write Hob list.
    #[error("Failed to write TD Hob list: {0}")]
    WriteHobList(#[source] vm_memory::GuestMemoryError),

    /// Failed to seek TdShim
    #[error("Failed to seek in TdShim file: {0}")]
    TdShimSeek(#[source] std::io::Error),

    /// Failed to load TDVF sections to guest memory
    #[error("Failed to load TdShim section to guest memory: {0}")]
    LoadTdShimSection(#[source] vm_memory::GuestMemoryError),

    /// Missing TDVF section
    #[error("Missing TdShim section: {0}")]
    MissingTdShimSection(&'static str),

    /// Error loading TdShim payload
    #[error("Error loading TdShim payload")]
    LoadTdShimPayload,
}
