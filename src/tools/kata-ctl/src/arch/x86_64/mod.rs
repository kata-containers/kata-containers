// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(dead_code)]

#[cfg(target_arch = "x86_64")]
pub use arch_specific::*;

mod arch_specific {
    use crate::check;
    use crate::check::{GuestProtection, ProtectionError};
    use crate::types::*;
    use anyhow::{anyhow, Result};
    use nix::unistd::Uid;
    use std::fs;
    use std::path::Path;

    const CPUINFO_DELIMITER: &str = "\nprocessor";
    const CPUINFO_FLAGS_TAG: &str = "flags";
    const CPU_FLAGS_INTEL: &[&str] = &["lm", "sse4_1", "vmx"];
    const CPU_ATTRIBS_INTEL: &[&str] = &["GenuineIntel"];
    pub const ARCH_CPU_VENDOR_FIELD: &str = check::GENERIC_CPU_VENDOR_FIELD;
    pub const ARCH_CPU_MODEL_FIELD: &str = check::GENERIC_CPU_MODEL_FIELD;

    // List of check functions
    static CHECK_LIST: &[CheckItem] = &[CheckItem {
        name: CheckType::CheckCpu,
        descr: "This parameter performs the cpu check",
        fp: check_cpu,
        perm: PermissionType::NonPrivileged,
    }];

    pub fn get_checks() -> Option<&'static [CheckItem<'static>]> {
        Some(CHECK_LIST)
    }

    // check cpu
    fn check_cpu(_args: &str) -> Result<()> {
        println!("INFO: check CPU: x86_64");

        let cpu_info = check::get_single_cpu_info(check::PROC_CPUINFO, CPUINFO_DELIMITER)?;

        let cpu_flags = check::get_cpu_flags(&cpu_info, CPUINFO_FLAGS_TAG).map_err(|e| {
            anyhow!(
                "Error parsing CPU flags, file {:?}, {:?}",
                check::PROC_CPUINFO,
                e
            )
        })?;

        // perform checks
        // TODO: Perform checks based on hypervisor type
        // TODO: Add more information to output (see kata-check in go tool); adjust formatting
        let missing_cpu_attributes = check::check_cpu_attribs(&cpu_info, CPU_ATTRIBS_INTEL)?;
        if !missing_cpu_attributes.is_empty() {
            eprintln!(
                "WARNING: Missing CPU attributes {:?}",
                missing_cpu_attributes
            );
        }
        let missing_cpu_flags = check::check_cpu_flags(&cpu_flags, CPU_FLAGS_INTEL)?;
        if !missing_cpu_flags.is_empty() {
            eprintln!("WARNING: Missing CPU flags {:?}", missing_cpu_flags);
        }

        Ok(())
    }

    fn retrieve_cpu_flags() -> Result<String> {
        let cpu_info = check::get_single_cpu_info(check::PROC_CPUINFO, CPUINFO_DELIMITER)?;

        let cpu_flags = check::get_cpu_flags(&cpu_info, CPUINFO_FLAGS_TAG).map_err(|e| {
            anyhow!(
                "Error parsing CPU flags, file {:?}, {:?}",
                check::PROC_CPUINFO,
                e
            )
        })?;

        Ok(cpu_flags)
    }

    pub const TDX_SYS_FIRMWARE_DIR: &str = "/sys/firmware/tdx_seam/";
    pub const TDX_CPU_FLAG: &str = "tdx";
    pub const SEV_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_amd/parameters/sev";
    pub const SNP_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_amd/parameters/sev_snp";

    pub fn available_guest_protection() -> Result<GuestProtection, ProtectionError> {
        if !Uid::effective().is_root() {
            return Err(ProtectionError::NoPerms);
        }

        arch_guest_protection(
            TDX_SYS_FIRMWARE_DIR,
            TDX_CPU_FLAG,
            SEV_KVM_PARAMETER_PATH,
            SNP_KVM_PARAMETER_PATH,
        )
    }

    pub fn arch_guest_protection(
        tdx_path: &str,
        tdx_flag: &str,
        sev_path: &str,
        snp_path: &str,
    ) -> Result<GuestProtection, ProtectionError> {
        let flags =
            retrieve_cpu_flags().map_err(|err| ProtectionError::CheckFailed(err.to_string()))?;

        let metadata = fs::metadata(tdx_path);

        if metadata.is_ok() && metadata.unwrap().is_dir() && flags.contains(tdx_flag) {
            return Ok(GuestProtection::Tdx);
        }

        let check_contents = |file_name: &str| -> Result<bool, ProtectionError> {
            let file_path = Path::new(file_name);
            if !file_path.exists() {
                return Ok(false);
            }

            let contents = fs::read_to_string(file_name).map_err(|err| {
                ProtectionError::CheckFailed(format!("Error reading file {} : {}", file_name, err))
            })?;

            if contents == "Y" {
                return Ok(true);
            }
            Ok(false)
        };

        if check_contents(snp_path)? {
            return Ok(GuestProtection::Snp);
        }

        if check_contents(sev_path)? {
            return Ok(GuestProtection::Sev);
        }

        Ok(GuestProtection::NoProtection)
    }
}

#[cfg(target_arch = "x86_64")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::check;
    use nix::unistd::Uid;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_available_guest_protection_no_privileges() {
        if !Uid::effective().is_root() {
            let res = available_guest_protection();
            assert!(res.is_err());
            assert_eq!(
                "No permission to check guest protection",
                res.unwrap_err().to_string()
            );
        }
    }

    fn test_arch_guest_protection_snp() {
        // Test snp
        let dir = tempdir().unwrap();
        let snp_file_path = dir.path().join("sev_snp");
        let path = snp_file_path.clone();
        let mut snp_file = fs::File::create(snp_file_path).unwrap();
        writeln!(snp_file, "Y").unwrap();

        let actual =
            arch_guest_protection("/xyz/tmp", TDX_CPU_FLAG, "/xyz/tmp", path.to_str().unwrap());
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), check::GuestProtection::Snp);

        writeln!(snp_file, "N").unwrap();
        let actual =
            arch_guest_protection("/xyz/tmp", TDX_CPU_FLAG, "/xyz/tmp", path.to_str().unwrap());
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), check::GuestProtection::NoProtection);
    }

    fn test_arch_guest_protection_sev() {
        // Test sev
        let dir = tempdir().unwrap();
        let sev_file_path = dir.path().join("sev");
        let sev_path = sev_file_path.clone();
        let mut sev_file = fs::File::create(sev_file_path).unwrap();
        writeln!(sev_file, "Y").unwrap();

        let actual = arch_guest_protection(
            "/xyz/tmp",
            TDX_CPU_FLAG,
            sev_path.to_str().unwrap(),
            "/xyz/tmp",
        );
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), check::GuestProtection::Sev);

        writeln!(sev_file, "N").unwrap();
        let actual = arch_guest_protection(
            "/xyz/tmp",
            TDX_CPU_FLAG,
            sev_path.to_str().unwrap(),
            "/xyz/tmp",
        );
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), check::GuestProtection::NoProtection);
    }
}
