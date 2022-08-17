// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;

#[cfg(target_arch = "aarch64")]
pub mod aarch64;

#[cfg(target_arch = "powerpc64le")]
pub mod powerpc64le;

#[cfg(target_arch = "s390x")]
pub mod s390x;

#[cfg(target_arch = "x86_64")]
pub mod x86_64;

pub fn check(global_args: clap::ArgMatches) -> Result<()> {
    #[cfg(target_arch = "aarch64")]
    let result = aarch64::check();

    #[cfg(target_arch = "powerpc64le")]
    let result = powerpc64le::check();

    #[cfg(target_arch = "s390x")]
    let result = s390x::check();

    #[cfg(target_arch = "x86_64")]
    let result = x86_64::check(global_args);

    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "powerpc64le",
        target_arch = "s390x",
        target_arch = "x86_64"
    )))]
    compile_error!("unknown architecture");

    result
}
