// Copyright (C) 2024 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

pub mod metadata;
pub use metadata::TdxMetadataDescriptor;

/// Error codes related to tdshim operations.
#[derive(Debug, thiserror::Error)]
pub enum TdshimError {
    /// Failed to read tdshim binary.
    #[error("failed to read tdshim binary, {0:?}")]
    ReadTdshim(#[source] std::io::Error),
    /// Failed to open tdshim binary.
    #[error("failed to open tdshim binary, {0:?}")]
    OpenTdshim(#[source] std::io::Error),
    /// Failed to get length of tdshim binary.
    #[error("failed to get length of tdshim binary, {0:?}")]
    GetLength(#[source] std::io::Error),
    /// Invalid metadata offset
    #[error("invalid metadata offset. {0:?}")]
    InvalidMetadataOffset(u32),
    /// Invalid Guid
    #[error("invalid guid.")]
    InvalidGuid,
    /// Invalid Metadata Descriptor
    #[error("invalid TdxMetadata Descriptor: {0:?}")]
    InvalidDescriptor(TdxMetadataDescriptor),
    /// Invalid Metadata length
    #[error("invalid TdxMetadata Length: {0:?}")]
    InvalidLength(u32),
    /// Invalid Metadata Section
    #[error("invalid TdxMetadata Section")]
    InvalidSection,
    /// Failed to parse guid
    #[error("failed to parse guid")]
    GuidParseError,
}

/// Specialized `Result` for tdshim related operations.
pub type Result<T> = std::result::Result<T, TdshimError>;
