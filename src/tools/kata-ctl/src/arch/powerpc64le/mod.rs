// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::types::*;
#[cfg(target_arch = "powerpc64le")]
pub use arch_specific::*;

mod arch_specific {
    use anyhow::Result;

    pub const ARCH_CPU_VENDOR_FIELD: &str = "";
    pub const ARCH_CPU_MODEL_FIELD: &str = "model";

    pub fn check() -> Result<()> {
        unimplemented!("Check not implemented in powerpc64le");
    }

    pub fn get_checks() -> Option<&'static [CheckItem<'static>]> {
        None
    }
}
