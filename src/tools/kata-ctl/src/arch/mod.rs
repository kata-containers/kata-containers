// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "aarch64")]
pub mod aarch64;
#[cfg(target_arch = "aarch64")]
pub use aarch64 as arch_specific;

#[cfg(target_arch = "powerpc64le")]
pub mod powerpc64le;
#[cfg(target_arch = "powerpc64le")]
pub use powerpc64le as arch_specific;

#[cfg(target_arch = "s390x")]
pub mod s390x;
#[cfg(target_arch = "s390x")]
pub use s390x as arch_specific;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64 as arch_specific;

#[cfg(not(any(
    target_arch = "aarch64",
    target_arch = "powerpc64le",
    target_arch = "s390x",
    target_arch = "x86_64"
)))]
compile_error!("unknown architecture");
