// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#![deny(missing_docs)]

use thiserror::Error;
use vm_memory::GuestMemoryError;

mod section;
pub use section::*;

mod hob;
pub use hob::*;

/// TDVF related errors
#[derive(Error, Debug)]
pub enum TdvfError {
    /// Error reading td_shim binary
    #[error("Failed to read td_shim file: {0}")]
    TdshimFileError(#[source] std::io::Error),

    /// Error parsing TDVF descriptor
    #[error("Failed to parse TDVF descriptor: {0}")]
    TdvfDescriptorError(&'static str),

    /// Error writing HOB list
    #[error("Failed to write HOB list: {0}")]
    WriteHobError(#[source] GuestMemoryError),

    /// Error loading section to guest memory
    #[error("Failed to load TDVF section to guest memory: {0}")]
    LoadTdvfSectionError(#[source] GuestMemoryError),
}
