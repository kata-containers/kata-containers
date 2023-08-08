// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use arch_gen::x86::bootparam::{__u32, __u64};
use vm_memory::bytes::Bytes;
use vm_memory::guest_memory::GuestAddress;
use vm_memory::{ByteValued, GuestMemory};

use super::layout;

/// With reference to the x86_hardware_subarch enumeration type of the
/// kernel, we newly added the X86_SUBARCH_DRAGONBALL type and defined
/// it as 0xdbdbdb01 to mark this as a guest kernel.
#[allow(dead_code)]
pub enum X86HardwareSubarch {
    X86SubarchPC = 0,
    X86SubarchLGUEST = 1,
    X86SubarchXEN = 2,
    X86SubarchIntelMID = 3,
    X86SubarchCE4100 = 4,
    X86SubarchDragonball = 0xdbdbdb01,
}

/// Recorded in subarch_data, used to verify the validity of dragonball subarch_data.
pub const DB_BOOT_PARAM_SIGNATURE: u64 = 0xdbdbb007700bbdbd;

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum Error {
    /// Error dragonball boot parameter length
    #[error("dragonball boot param exceeds max size")]
    DragonballBootParamPastMaxSize,

    /// Error dragonball boot parameter location
    #[error("dragonball boot param past ram end")]
    DragonballBootParamPastRamEnd,

    /// Error writing dragonball boot parameter
    #[error("dragonball boot param setup fail")]
    WriteDragonballBootParam,
}

