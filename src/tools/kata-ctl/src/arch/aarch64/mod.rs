// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "aarch64")]
pub use arch_specific::*;

mod arch_specific {
    use anyhow::Result;

    pub fn check() -> Result<()> {
        unimplemented!("Check not implemented in aarch64")
    }
}
