// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(target_arch = "x86_64")]
use anyhow::anyhow;
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TDXDetails {
    pub major_version: u32,
    pub minor_version: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum GuestProtection {
    #[default]
    NoProtection,
    Tdx(TDXDetails),
    Sev,
    Snp,
    Pef,
    Se,
}

impl fmt::Display for GuestProtection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GuestProtection::Tdx(details) => write!(
                f,
                "tdx (major_version: {}, minor_version: {})",
                details.major_version, details.minor_version
            ),
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

    #[error("Cannot resolve path {0} below {1}: {2}")]
    CannotResolvePath(String, PathBuf, anyhow::Error),

    #[error("Expected file {0} not found: {1}")]
    FileMissing(String, std::io::Error),

    #[error("File {0} contains unexpected content: {1}")]
    FileInvalid(PathBuf, anyhow::Error),
}

#[cfg(target_arch = "x86_64")]
pub const TDX_SYS_FIRMWARE_DIR: &str = "/sys/firmware/tdx/";
#[cfg(target_arch = "x86_64")]
pub const SEV_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_amd/parameters/sev";
#[cfg(target_arch = "x86_64")]
pub const SNP_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_amd/parameters/sev_snp";

// Module directory below TDX_SYS_FIRMWARE_DIR.
#[cfg(target_arch = "x86_64")]
const TDX_FW_MODULE_DIR: &str = "tdx_module";

// File in TDX_FW_MODULE_DIR that specifies TDX major version number.
#[cfg(target_arch = "x86_64")]
const TDX_MAJOR_FILE: &str = "major_version";

// File in TDX_FW_MODULE_DIR that specifies TDX minor version number.
#[cfg(target_arch = "x86_64")]
const TDX_MINOR_FILE: &str = "minor_version";

#[cfg(target_arch = "x86_64")]
pub fn available_guest_protection() -> Result<GuestProtection, ProtectionError> {
    arch_guest_protection(
        TDX_SYS_FIRMWARE_DIR,
        SEV_KVM_PARAMETER_PATH,
        SNP_KVM_PARAMETER_PATH,
    )
}

#[cfg(target_arch = "x86_64")]
pub fn arch_guest_protection(
    tdx_path: &str,
    sev_path: &str,
    snp_path: &str,
) -> Result<GuestProtection, ProtectionError> {
    let metadata = fs::metadata(tdx_path);

    if metadata.is_ok() && metadata.unwrap().is_dir() {
        let module_dir = safe_path::scoped_join(tdx_path, TDX_FW_MODULE_DIR).map_err(|e| {
            ProtectionError::CannotResolvePath(
                TDX_FW_MODULE_DIR.to_string(),
                PathBuf::from(tdx_path),
                anyhow!(e),
            )
        })?;

        let major_file =
            safe_path::scoped_join(module_dir.clone(), TDX_MAJOR_FILE).map_err(|e| {
                ProtectionError::CannotResolvePath(
                    TDX_MAJOR_FILE.to_string(),
                    module_dir.clone(),
                    anyhow!(e),
                )
            })?;

        let minor_file =
            safe_path::scoped_join(module_dir.clone(), TDX_MINOR_FILE).map_err(|e| {
                ProtectionError::CannotResolvePath(
                    TDX_MINOR_FILE.to_string(),
                    module_dir,
                    anyhow!(e),
                )
            })?;

        const HEX_BASE: u32 = 16;
        const HEX_PREFIX: &str = "0x";

        let major_version_str = std::fs::read_to_string(major_file.clone()).map_err(|e| {
            ProtectionError::FileMissing(major_file.clone().to_string_lossy().into(), e)
        })?;

        let major_version_str = major_version_str.trim_start_matches(HEX_PREFIX);

        let major_version = u32::from_str_radix(major_version_str, HEX_BASE)
            .map_err(|e| ProtectionError::FileInvalid(major_file, anyhow!(e)))?;

        let minor_version_str = std::fs::read_to_string(minor_file.clone()).map_err(|e| {
            ProtectionError::FileMissing(minor_file.clone().to_string_lossy().into(), e)
        })?;

        let minor_version_str = minor_version_str.trim_start_matches(HEX_PREFIX);

        let minor_version = u32::from_str_radix(minor_version_str, HEX_BASE)
            .map_err(|e| ProtectionError::FileInvalid(minor_file, anyhow!(e)))?;

        let details = TDXDetails {
            major_version,
            minor_version,
        };

        return Ok(GuestProtection::Tdx(details));
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

        let actual = arch_guest_protection("/xyz/tmp", "/xyz/tmp", path.to_str().unwrap());
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), GuestProtection::Snp);

        writeln!(snp_file, "N").unwrap();
        let actual = arch_guest_protection("/xyz/tmp", "/xyz/tmp", path.to_str().unwrap());
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

        let actual = arch_guest_protection("/xyz/tmp", sev_path.to_str().unwrap(), "/xyz/tmp");
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), GuestProtection::Sev);

        writeln!(sev_file, "N").unwrap();
        let actual = arch_guest_protection("/xyz/tmp", sev_path.to_str().unwrap(), "/xyz/tmp");
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

        let actual = arch_guest_protection(invalid_dir, invalid_dir, invalid_dir);
        assert!(actual.is_ok());
        assert_eq!(actual.unwrap(), GuestProtection::NoProtection);

        let actual = arch_guest_protection(tdx_path.to_str().unwrap(), invalid_dir, invalid_dir);
        assert!(actual.is_err());

        let tdx_module = tdx_path.join(TDX_FW_MODULE_DIR);
        std::fs::create_dir_all(tdx_module.clone()).unwrap();

        let major_file = tdx_module.join(TDX_MAJOR_FILE);
        std::fs::File::create(&major_file).unwrap();

        let minor_file = tdx_module.join(TDX_MINOR_FILE);
        std::fs::File::create(&minor_file).unwrap();

        let result = arch_guest_protection(tdx_path.to_str().unwrap(), invalid_dir, invalid_dir);
        assert!(result.is_err());

        std::fs::write(&major_file, b"invalid").unwrap();
        std::fs::write(&minor_file, b"invalid").unwrap();

        let result = arch_guest_protection(tdx_path.to_str().unwrap(), invalid_dir, invalid_dir);
        assert!(result.is_err());

        // Fake a TDX 1.0 environment
        std::fs::write(&major_file, b"0x00000001").unwrap();
        std::fs::write(&minor_file, b"0x00000000").unwrap();

        let result = arch_guest_protection(tdx_path.to_str().unwrap(), invalid_dir, invalid_dir);
        assert!(result.is_ok());

        let result = result.unwrap();

        let details = match &result {
            GuestProtection::Tdx(details) => details,
            _ => panic!(),
        };

        assert_eq!(details.major_version, 1);
        assert_eq!(details.minor_version, 0);

        let displayed_value = result.to_string();
        assert_eq!(displayed_value, "tdx (major_version: 1, minor_version: 0)");
    }
}
