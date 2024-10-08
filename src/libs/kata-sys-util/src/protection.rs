// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "x86_64")]
use anyhow::anyhow;
#[cfg(any(target_arch = "s390x", target_arch = "x86_64", target_arch = "aarch64"))]
use anyhow::Result;
use std::fmt;
#[cfg(target_arch = "x86_64")]
use std::path::Path;
use thiserror::Error;

#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
use nix::unistd::Uid;

#[cfg(target_arch = "x86_64")]
use std::fs;

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
pub enum GuestProtection {
    NoProtection,
    Tdx,
    Sev,
    Snp,
    Pef,
    Se,
}

impl fmt::Display for GuestProtection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GuestProtection::Tdx => write!(f, "tdx"),
            GuestProtection::Sev => write!(f, "sev"),
            GuestProtection::Snp => write!(f, "snp"),
            GuestProtection::Pef => write!(f, "pef"),
            GuestProtection::Se => write!(f, "se"),
            GuestProtection::NoProtection => write!(f, "none"),
        }
    }
}

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum ProtectionError {
    #[error("No permission to check guest protection")]
    NoPerms,

    #[error("Failed to check guest protection: {0}")]
    CheckFailed(String),

    #[error("Invalid guest protection value: {0}")]
    InvalidValue(String),
}

#[cfg(target_arch = "x86_64")]
pub const TDX_SYS_FIRMWARE_DIR: &str = "/sys/firmware/tdx_seam/";
#[cfg(target_arch = "x86_64")]
pub const TDX_CPU_FLAG: &str = "tdx";
#[cfg(target_arch = "x86_64")]
pub const SEV_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_amd/parameters/sev";
#[cfg(target_arch = "x86_64")]
pub const SNP_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_amd/parameters/sev_snp";

#[cfg(target_arch = "x86_64")]
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

#[cfg(target_arch = "x86_64")]
fn retrieve_cpu_flags() -> Result<String> {
    let cpu_info =
        crate::cpu::get_single_cpu_info(crate::cpu::PROC_CPUINFO, crate::cpu::CPUINFO_DELIMITER)?;

    let cpu_flags =
        crate::cpu::get_cpu_flags(&cpu_info, crate::cpu::CPUINFO_FLAGS_TAG).map_err(|e| {
            anyhow!(
                "Error parsing CPU flags, file {:?}, {:?}",
                crate::cpu::PROC_CPUINFO,
                e
            )
        })?;

    Ok(cpu_flags)
}

#[cfg(target_arch = "x86_64")]
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

        if contents.trim() == "Y" {
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

#[cfg(target_arch = "s390x")]
#[allow(dead_code)]
// Guest protection is not supported on ARM64.
pub fn available_guest_protection() -> Result<GuestProtection, ProtectionError> {
    if !Uid::effective().is_root() {
        return Err(ProtectionError::NoPerms)?;
    }

    let facilities = crate::cpu::retrieve_cpu_facilities().map_err(|err| {
        ProtectionError::CheckFailed(format!(
            "Error retrieving cpu facilities file : {}",
            err.to_string()
        ))
    })?;

    // Secure Execution
    // https://www.kernel.org/doc/html/latest/virt/kvm/s390-pv.html
    let se_cpu_facility_bit: i32 = 158;
    if !facilities.contains_key(&se_cpu_facility_bit) {
        return Ok(GuestProtection::NoProtection);
    }

    let cmd_line_values = vec!["1", "on", "y", "yes"];
    let se_cmdline_param = "prot_virt";

    let se_cmdline_present =
        crate::check_kernel_cmd_line("/proc/cmdline", se_cmdline_param, &cmd_line_values)
            .map_err(|err| ProtectionError::CheckFailed(err.to_string()))?;

    if !se_cmdline_present {
        return Err(ProtectionError::InvalidValue(String::from(
            "Protected Virtualization is not enabled on kernel command line!",
        )));
    }

    Ok(GuestProtection::Se)
}

#[cfg(all(target_arch = "powerpc64", target_endian = "little"))]
pub fn available_guest_protection() -> Result<check::GuestProtection, check::ProtectionError> {
    if !Uid::effective().is_root() {
        return Err(check::ProtectionError::NoPerms);
    }

    let metadata = fs::metadata(PEF_SYS_FIRMWARE_DIR);
    if metadata.is_ok() && metadata.unwrap().is_dir() {
        Ok(check::GuestProtection::Pef)
    }

    Ok(check::GuestProtection::NoProtection)
}

#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
// Guest protection is not supported on ARM64.
pub fn available_guest_protection() -> Result<GuestProtection, ProtectionError> {
    Ok(GuestProtection::NoProtection)
}

#[cfg(target_arch = "x86_64")]
#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
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
        assert_eq!(actual.unwrap(), GuestProtection::Snp);

        writeln!(snp_file, "N").unwrap();
        let actual =
            arch_guest_protection("/xyz/tmp", TDX_CPU_FLAG, "/xyz/tmp", path.to_str().unwrap());
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), GuestProtection::NoProtection);
    }

    #[test]
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
        assert_eq!(actual.unwrap(), GuestProtection::Sev);

        writeln!(sev_file, "N").unwrap();
        let actual = arch_guest_protection(
            "/xyz/tmp",
            TDX_CPU_FLAG,
            sev_path.to_str().unwrap(),
            "/xyz/tmp",
        );
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), GuestProtection::NoProtection);
    }
}
