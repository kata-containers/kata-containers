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
    use crate::utils;
    use anyhow::{anyhow, Result};
    use slog::{info, o, warn};
    use std::io::BufRead;
    use std::io::BufReader;

    const CPUINFO_DELIMITER: &str = "processor ";
    const CPUINFO_FEATURES_TAG: &str = "features";
    const CPU_FEATURES_REQ: &[&str] = &["sie"];

    macro_rules! sl {
        () => {
            slog_scope::logger().new(o!("subsystem" => "s390x"))
        };
    }

    #[allow(dead_code)]
    pub const ARCH_CPU_VENDOR_FIELD: &str = check::GENERIC_CPU_VENDOR_FIELD;
    #[allow(dead_code)]
    pub const ARCH_CPU_MODEL_FIELD: &str = "machine";

    // check cpu
    fn check_cpu() -> Result<()> {
        info!(sl!(), "check CPU: s390x");

        let cpu_info =
            kata_sys_util::cpu::get_single_cpu_info(check::PROC_CPUINFO, CPUINFO_DELIMITER)?;

        let cpu_features = kata_sys_util::cpu::get_cpu_flags(&cpu_info, CPUINFO_FEATURES_TAG)
            .map_err(|e| {
                anyhow!(
                    "Error parsing CPU features, file {:?}, {:?}",
                    check::PROC_CPUINFO,
                    e
                )
            })?;

        let missing_cpu_features = check::check_cpu_flags(&cpu_features, CPU_FEATURES_REQ)?;
        if !missing_cpu_features.is_empty() {
            warn!(sl!(), "Missing CPU flags {:?}", missing_cpu_features);
        }

        Ok(())
    }

    pub fn check(_args: &str) -> Result<()> {
        info!(sl!(), "check: s390x");

        let _cpu_result = check_cpu();

        // TODO: add additional checks, e.g, kernel modules as in go runtime
        // TODO: collect outcome of tests to determine if checks pass or not

        Ok(())
    }

    // List of check functions
    static CHECK_LIST: &[CheckItem] = &[CheckItem {
        name: CheckType::Cpu,
        descr: "This parameter performs the cpu check",
        fp: check,
        perm: PermissionType::NonPrivileged,
    }];

    pub fn get_checks() -> Option<&'static [CheckItem<'static>]> {
        Some(CHECK_LIST)
    }

    pub fn host_is_vmcontainer_capable() -> Result<bool> {
        let mut count = 0;
        if check_cpu().is_err() {
            count += 1;
        };

        // TODO: Add additional checks for kernel modules

        if count == 0 {
            return Ok(true);
        };

        Err(anyhow!("System is not capable of running a VM"))
    }

    #[allow(dead_code)]
    pub fn check_cmd_line(
        kernel_cmdline_path: &str,
        search_param: &str,
        search_values: &[&str],
    ) -> Result<bool> {
        let f = std::fs::File::open(kernel_cmdline_path)?;
        let reader = BufReader::new(f);

        let check_fn = if search_values.is_empty() {
            |param: &str, search_param: &str, _search_values: &[&str]| {
                param.eq_ignore_ascii_case(search_param)
            }
        } else {
            |param: &str, search_param: &str, search_values: &[&str]| {
                let split: Vec<&str> = param.splitn(2, '=').collect();
                if split.len() < 2 || split[0] != search_param {
                    return false;
                }

                for value in search_values {
                    if value.eq_ignore_ascii_case(split[1]) {
                        return true;
                    }
                }
                false
            }
        };

        for line in reader.lines() {
            for field in line?.split_whitespace() {
                if check_fn(field, search_param, search_values) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn get_cpu_details() -> Result<(String, String)> {
        utils::get_generic_cpu_details(check::PROC_CPUINFO)

        // TODO: In case of error from get_generic_cpu_details, implement functionality
        // to get cpu details specific to s390x architecture similar
        // to the goloang implementation of function getS390xCPUDetails()
    }
}
