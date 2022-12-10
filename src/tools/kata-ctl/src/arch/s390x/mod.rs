// Copyright (c) 2022 Intel Corporation
// Copyright (c) 2022 IBM Corp.
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "s390x")]
pub use arch_specific::*;

mod arch_specific {
    use crate::check;
    use crate::types::*;
    use anyhow::{anyhow, Result};

    const CPUINFO_DELIMITER: &str = "processor ";
    const CPUINFO_FEATURES_TAG: &str = "features";
    const CPU_FEATURES_REQ: &[&str] = &["sie"];

    #[allow(dead_code)]
    pub const ARCH_CPU_VENDOR_FIELD: &str = check::GENERIC_CPU_VENDOR_FIELD;
    #[allow(dead_code)]
    pub const ARCH_CPU_MODEL_FIELD: &str = "machine";

    // check cpu
    fn check_cpu() -> Result<()> {
        println!("INFO: check CPU: s390x");

        let cpu_info = check::get_single_cpu_info(check::PROC_CPUINFO, CPUINFO_DELIMITER)?;

        let cpu_features = check::get_cpu_flags(&cpu_info, CPUINFO_FEATURES_TAG).map_err(|e| {
            anyhow!(
                "Error parsing CPU features, file {:?}, {:?}",
                check::PROC_CPUINFO,
                e
            )
        })?;

        let missing_cpu_features = check::check_cpu_flags(&cpu_features, CPU_FEATURES_REQ)?;
        if !missing_cpu_features.is_empty() {
            eprintln!("WARNING: Missing CPU flags {:?}", missing_cpu_features);
        }

        Ok(())
    }

    pub fn check(_args: &str) -> Result<()> {
        println!("INFO: check: s390x");

        let _cpu_result = check_cpu();

        // TODO: add additional checks, e.g, kernel modules as in go runtime
        // TODO: collect outcome of tests to determine if checks pass or not

        Ok(())
    }

    // List of check functions
    static CHECK_LIST: &[CheckItem] = &[CheckItem {
        name: CheckType::CheckCpu,
        descr: "This parameter performs the cpu check",
        fp: check,
        perm: PermissionType::NonPrivileged,
    }];

    pub fn get_checks() -> Option<&'static [CheckItem<'static>]> {
        Some(CHECK_LIST)
    }
}
