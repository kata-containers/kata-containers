// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "x86_64")]
pub use arch_specific::*;

mod arch_specific {
    use anyhow::Result;

    pub fn check(_global_args: clap::ArgMatches) -> Result<()> {
        println!("INFO: check: x86_64");

        Ok(())
    }
}
