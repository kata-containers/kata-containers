// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(all(target_arch = "powerpc64", target_endian = "little"))]
pub use arch_specific::*;

mod arch_specific {
    use crate::check;
    use crate::types::CheckItem;
    use crate::utils;
    use anyhow::Result;

    pub const ARCH_CPU_VENDOR_FIELD: &str = "";
    pub const ARCH_CPU_MODEL_FIELD: &str = "model";

    #[allow(dead_code)]
    pub fn check() -> Result<()> {
        unimplemented!("Check not implemented in powerpc64");
    }

    pub fn get_checks() -> Option<&'static [CheckItem<'static>]> {
        None
    }

    pub fn get_cpu_details() -> Result<(String, String)> {
        utils::get_generic_cpu_details(check::PROC_CPUINFO)

        // TODO: In case of error from get_generic_cpu_details, implement functionality
        // to get cpu details specific to powerpc architecture similar
        // to the goloang implementation of function getCPUDetails()
    }

    pub fn host_is_vmcontainer_capable() -> Result<bool> {
        // TODO: Not implemented
        Ok(true)
    }
}
