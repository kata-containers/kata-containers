// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "powerpc64le")]
pub use arch_specific::*;

mod arch_specific {
    use anyhow::Result;

    pub fn check() -> Result<()> {
        unimplemented!("Check not implemented in powerpc64le");
    }
}
