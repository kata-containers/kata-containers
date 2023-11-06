// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::convert::TryFrom;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum VmConfigError {
    #[error("empty sandbox path")]
    EmptySandboxPath,

    #[error("cannot specify image and initrd")]
    MultipleBootFiles,

    #[error("missing boot image (no rootfs image or initrd)")]
    NoBootFile,

    #[error("CPU config error: {0}")]
    CPUError(CpusConfigError),

    #[error("Pmem config error: {0}")]
    PmemError(PmemConfigError),

    #[error("Payload config error: {0}")]
    PayloadError(PayloadConfigError),

    #[error("Disk config error: {0}")]
    DiskError(DiskConfigError),

    #[error("Memory config error: {0}")]
    MemoryError(MemoryConfigError),

    // The 2nd arg is actually a std::io::Error but that doesn't implement
    // PartialEq, so we convert it to a String.
    #[error("Failed to create sandbox path ({0}: {1}")]
    SandboxError(String, String),

    #[error("VSOCK config error: {0}")]
    VsockError(VsockConfigError),

    #[error("TDX requires virtio-blk VM rootfs driver")]
    TDXVMRootfsNotVirtioBlk,

    #[error("TDX requires virtio-blk container rootfs block device driver")]
    TDXContainerRootfsNotVirtioBlk,

    // LIMITATION: Current CH TDX limitation.
    #[error("TDX requires an image=, not an initrd=")]
    TDXDisallowsInitrd,
}

#[derive(Error, Debug, PartialEq)]
pub enum PmemConfigError {
    #[error("Need rootfs image for PmemConfig")]
    MissingImage,
}

#[derive(Error, Debug, PartialEq)]
pub enum DiskConfigError {
    #[error("Need path for DiskConfig")]
    MissingPath,

    #[error("Found unexpected path for DiskConfig with TDX: {0}")]
    UnexpectedPathForTDX(String),
}

#[derive(Error, Debug, PartialEq)]
pub enum CpusConfigError {
    #[error("Boot vCPUs cannot be zero or negative")]
    BootVCPUsTooSmall,

    #[error("Too many boot vCPUs specified: {0}")]
    BootVCPUsTooBig(<u8 as TryFrom<i32>>::Error),

    #[error("Max vCPUs cannot be zero or negative")]
    MaxVCPUsTooSmall,

    #[error("Too many max vCPUs specified: {0}")]
    MaxVCPUsTooBig(<u8 as TryFrom<u32>>::Error),

    #[error("Boot vCPUs cannot be larger than max vCPUs")]
    BootVPUsGtThanMaxVCPUs,
}

#[derive(Error, Debug, PartialEq)]
pub enum PayloadConfigError {
    #[error("No kernel specified")]
    NoKernel,

    #[error("No initrd/initramfs specified")]
    NoInitrd,

    #[error("Need firmware for TDX")]
    TDXFirmwareMissing,
}

#[derive(Error, Debug, PartialEq)]
pub enum MemoryConfigError {
    #[error("No default memory specified")]
    NoDefaultMemory,

    #[error("Default memory size > available RAM")]
    DefaultMemSizeTooBig,

    #[error("Cannot convert default memory to bytes: {0}")]
    BadDefaultMemSize(u32),

    #[error("Cannot calculate hotplug memory size from default memory: {0}")]
    BadMemSizeForHotplug(u64),

    #[error("Cannot align hotplug memory size from pmem: {0}")]
    BadPmemAlign(u64),

    #[error("Failed to query system memory information: {0}")]
    SysInfoFail(#[source] nix::errno::Errno),
}

#[derive(Error, Debug, PartialEq)]
pub enum VsockConfigError {
    #[error("Missing VSOCK socket path")]
    NoVsockSocketPath,
}
