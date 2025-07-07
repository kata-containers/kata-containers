// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(any(target_arch = "s390x", target_arch = "x86_64", target_arch = "aarch64"))]
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
#[cfg(all(target_arch = "powerpc64", target_endian = "little"))]
use std::fs;
#[cfg(target_arch = "x86_64")]
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

#[cfg(any(
    target_arch = "s390x",
    all(target_arch = "powerpc64", target_endian = "little")
))]
use nix::unistd::Uid;

#[cfg(target_arch = "x86_64")]
use std::fs;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SevSnpDetails {
    pub cbitpos: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum GuestProtection {
    #[default]
    NoProtection,
    Tdx,
    Sev(SevSnpDetails),
    Snp(SevSnpDetails),
    Pef,
    Se,
}

impl fmt::Display for GuestProtection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GuestProtection::Tdx => write!(f, "tdx"),
            GuestProtection::Sev(details) => write!(f, "sev (cbitpos: {}", details.cbitpos),
            GuestProtection::Snp(details) => write!(f, "snp (cbitpos: {}", details.cbitpos),
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

    #[error("Cannot resolve path {0} below {1}: {2}")]
    CannotResolvePath(String, PathBuf, anyhow::Error),

    #[error("Expected file {0} not found: {1}")]
    FileMissing(String, std::io::Error),

    #[error("File {0} contains unexpected content: {1}")]
    FileInvalid(PathBuf, anyhow::Error),
}

#[cfg(target_arch = "x86_64")]
pub const TDX_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_intel/parameters/tdx";
#[cfg(target_arch = "x86_64")]
pub const SEV_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_amd/parameters/sev";
#[cfg(target_arch = "x86_64")]
pub const SNP_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_amd/parameters/sev_snp";

#[cfg(target_arch = "x86_64")]
pub fn available_guest_protection() -> Result<GuestProtection, ProtectionError> {
    arch_guest_protection(SEV_KVM_PARAMETER_PATH, SNP_KVM_PARAMETER_PATH)
}

#[cfg(target_arch = "x86_64")]
pub fn arch_guest_protection(
    sev_path: &str,
    snp_path: &str,
) -> Result<GuestProtection, ProtectionError> {
    // Check if /sys/module/kvm_intel/parameters/tdx is set to 'Y'
    if Path::new(TDX_KVM_PARAMETER_PATH).exists() {
        if let Ok(content) = fs::read(TDX_KVM_PARAMETER_PATH) {
            if !content.is_empty() && content[0] == b'Y' {
                return Ok(GuestProtection::Tdx);
            }
        }
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

    let retrieve_sev_cbitpos = || -> Result<u32, ProtectionError> {
        Err(ProtectionError::CheckFailed(
            "cbitpos retrieval NOT IMPLEMENTED YET".to_owned(),
        ))
    };

    let is_snp_available = check_contents(snp_path)?;
    let is_sev_available = is_snp_available || check_contents(sev_path)?;
    if is_snp_available || is_sev_available {
        let cbitpos = retrieve_sev_cbitpos()?;
        let sev_snp_details = SevSnpDetails { cbitpos };
        return Ok(if is_snp_available {
            GuestProtection::Snp(sev_snp_details)
        } else {
            GuestProtection::Sev(sev_snp_details)
        });
    }

    Ok(GuestProtection::NoProtection)
}

#[cfg(target_arch = "s390x")]
#[allow(dead_code)]
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
const PEF_SYS_FIRMWARE_DIR: &str = "/sys/firmware/ultravisor/";

#[cfg(all(target_arch = "powerpc64", target_endian = "little"))]
pub fn available_guest_protection() -> Result<GuestProtection, ProtectionError> {
    if !Uid::effective().is_root() {
        return Err(ProtectionError::NoPerms);
    }

    let metadata = fs::metadata(PEF_SYS_FIRMWARE_DIR);
    if metadata.is_ok() && metadata.unwrap().is_dir() {
        return Ok(GuestProtection::Pef);
    }

    Ok(GuestProtection::NoProtection)
}

#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
// Guest protection is not supported on ARM64.
pub fn available_guest_protection() -> Result<GuestProtection, ProtectionError> {
    Ok(GuestProtection::NoProtection)
}

#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
// Guest protection is not supported on RISC-V.
pub fn available_guest_protection() -> Result<GuestProtection, ProtectionError> {
    Ok(GuestProtection::NoProtection)
}

#[cfg(target_arch = "x86_64")]
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_arch_guest_protection_snp() {
        // Test snp
        let dir = tempdir().unwrap();
        let snp_file_path = dir.path().join("sev_snp");
        let path = snp_file_path.clone();
        let mut snp_file = fs::File::create(snp_file_path).unwrap();
        writeln!(snp_file, "Y").unwrap();

        let actual = arch_guest_protection("/xyz/tmp", path.to_str().unwrap());
        assert!(actual.is_ok());
        assert!(matches!(actual.unwrap(), GuestProtection::Snp(_)));

        writeln!(snp_file, "N").unwrap();
        let actual = arch_guest_protection("/xyz/tmp", path.to_str().unwrap());
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

        let actual = arch_guest_protection(sev_path.to_str().unwrap(), "/xyz/tmp");
        assert!(actual.is_ok());
        assert!(matches!(actual.unwrap(), GuestProtection::Sev(_)));

        writeln!(sev_file, "N").unwrap();
        let actual = arch_guest_protection(sev_path.to_str().unwrap(), "/xyz/tmp");
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), GuestProtection::NoProtection);
    }

    #[test]
    fn test_arch_guest_protection_tdx() {
        let dir = tempdir().unwrap();

        let invalid_dir = dir.path().join("enoent");
        let invalid_dir = invalid_dir.to_str().unwrap();

        let tdx_file_path = dir.path().join("tdx");
        let tdx_path = tdx_file_path;

        std::fs::create_dir_all(tdx_path.clone()).unwrap();

        let actual = arch_guest_protection(invalid_dir, invalid_dir);
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), GuestProtection::NoProtection);

        let actual = arch_guest_protection(invalid_dir, invalid_dir);
        assert!(actual.is_err());

        let result = arch_guest_protection(invalid_dir, invalid_dir);
        assert!(result.is_ok());

        let result = result.unwrap();

        let displayed_value = result.to_string();
        assert_eq!(displayed_value, "tdx");
    }
}
