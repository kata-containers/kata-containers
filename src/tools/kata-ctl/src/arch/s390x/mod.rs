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
    use nix::unistd::Uid;
    use slog::{info, o, warn};
    use std::collections::HashMap;
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

    #[allow(dead_code)]
    fn retrieve_cpu_facilities() -> Result<HashMap<i32, bool>> {
        let f = std::fs::File::open(check::PROC_CPUINFO)?;
        let mut reader = BufReader::new(f);
        let mut contents = String::new();
        let facilities_field = "facilities";
        let mut facilities = HashMap::new();

        while reader.read_line(&mut contents)? > 0 {
            let fields: Vec<&str> = contents.split_whitespace().collect();
            if fields.len() < 2 {
                contents.clear();
                continue;
            }

            if !fields[0].starts_with(facilities_field) {
                contents.clear();
                continue;
            }

            let mut start = 1;
            if fields[1] == ":" {
                start = 2;
            }

            for field in fields.iter().skip(start) {
                let bit = field.parse::<i32>()?;
                facilities.insert(bit, true);
            }
            return Ok(facilities);
        }

        Ok(facilities)
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
                return param.eq_ignore_ascii_case(search_param);
            }
        } else {
            |param: &str, search_param: &str, search_values: &[&str]| {
                let split: Vec<&str> = param.splitn(2, "=").collect();
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

    #[allow(dead_code)]
    // Guest protection is not supported on ARM64.
    pub fn available_guest_protection() -> Result<check::GuestProtection, check::ProtectionError> {
        if !Uid::effective().is_root() {
            return Err(check::ProtectionError::NoPerms)?;
        }

        let facilities = retrieve_cpu_facilities().map_err(|err| {
            check::ProtectionError::CheckFailed(format!(
                "Error retrieving cpu facilities file : {}",
                err.to_string()
            ))
        })?;

        // Secure Execution
        // https://www.kernel.org/doc/html/latest/virt/kvm/s390-pv.html
        let se_cpu_facility_bit: i32 = 158;
        if !facilities.contains_key(&se_cpu_facility_bit) {
            return Ok(check::GuestProtection::NoProtection);
        }

        let cmd_line_values = vec!["1", "on", "y", "yes"];
        let se_cmdline_param = "prot_virt";

        let se_cmdline_present =
            check_cmd_line("/proc/cmdline", se_cmdline_param, &cmd_line_values)
                .map_err(|err| check::ProtectionError::CheckFailed(err.to_string()))?;

        if !se_cmdline_present {
            return Err(check::ProtectionError::InvalidValue(String::from(
                "Protected Virtualization is not enabled on kernel command line!",
            )));
        }

        Ok(check::GuestProtection::Se)
    }
}
