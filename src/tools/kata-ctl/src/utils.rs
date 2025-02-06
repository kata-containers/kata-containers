// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(dead_code)]

use crate::arch::arch_specific;

use anyhow::{anyhow, Context, Result};
use std::{fs, time::Duration};

const NON_PRIV_USER: &str = "nobody";

pub const TIMEOUT: Duration = Duration::from_millis(2000);

pub fn drop_privs() -> Result<()> {
    if nix::unistd::Uid::effective().is_root() {
        privdrop::PrivDrop::default()
            .chroot("/")
            .user(NON_PRIV_USER)
            .apply()
            .map_err(|e| anyhow!("Failed to drop privileges to user {}: {}", NON_PRIV_USER, e))?;
    }

    Ok(())
}

pub const PROC_VERSION_FILE: &str = "/proc/version";

pub fn get_kernel_version(proc_version_file: &str) -> Result<String> {
    let contents = fs::read_to_string(proc_version_file)
        .context(format!("Failed to read file {}", proc_version_file))?;

    let fields: Vec<&str> = contents.split_whitespace().collect();

    if fields.len() < 3 {
        return Err(anyhow!("unexpected contents in file {}", proc_version_file));
    }

    let kernel_version = String::from(fields[2]);
    Ok(kernel_version)
}

pub const OS_RELEASE: &str = "/etc/os-release";

// Clear Linux has a different path (for stateless support)
pub const OS_RELEASE_CLR: &str = "/usr/lib/os-release";

const UNKNOWN: &str = "unknown";

fn get_field_fn(line: &str, delimiter: &str, file_name: &str) -> Result<String> {
    let fields: Vec<&str> = line.split(delimiter).collect();
    if fields.len() < 2 {
        Err(anyhow!("Unexpected file contents for {}", file_name))
    } else {
        let val = fields[1].trim();
        Ok(String::from(val))
    }
}
// Ref: https://www.freedesktop.org/software/systemd/man/os-release.html
pub fn get_distro_details(os_release: &str, os_release_clr: &str) -> Result<(String, String)> {
    let files = [os_release, os_release_clr];
    let mut name = String::new();
    let mut version = String::new();

    for release_file in files.iter() {
        match fs::read_to_string(release_file) {
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    continue;
                } else {
                    return Err(anyhow!(
                        "Error reading file {}: {}",
                        release_file,
                        e.to_string()
                    ));
                }
            }
            Ok(contents) => {
                let lines = contents.lines();

                for line in lines {
                    if line.starts_with("NAME=") && name.is_empty() {
                        name = get_field_fn(line, "=", release_file)?;
                    } else if line.starts_with("VERSION_ID=") && version.is_empty() {
                        version = get_field_fn(line, "=", release_file)?;
                    }
                }
                if !name.is_empty() && !version.is_empty() {
                    return Ok((name, version));
                }
            }
        }
    }

    if name.is_empty() {
        name = String::from(UNKNOWN);
    }

    if version.is_empty() {
        version = String::from(UNKNOWN);
    }

    Ok((name, version))
}

#[cfg(any(
    target_arch = "s390x",
    target_arch = "x86_64",
    target_arch = "aarch64",
    all(target_arch = "powerpc64", target_endian = "little"),
))]
#[allow(clippy::const_is_empty)]
pub fn get_generic_cpu_details(cpu_info_file: &str) -> Result<(String, String)> {
    let cpu_info = kata_sys_util::cpu::get_single_cpu_info(cpu_info_file, "\n\n")?;
    let lines = cpu_info.lines();
    let mut vendor = String::new();
    let mut model = String::new();

    for line in lines {
        if !arch_specific::ARCH_CPU_VENDOR_FIELD.is_empty()
            && line.starts_with(arch_specific::ARCH_CPU_VENDOR_FIELD)
        {
            vendor = get_field_fn(line, ":", cpu_info_file)?;
        }
        if !arch_specific::ARCH_CPU_MODEL_FIELD.is_empty()
            && line.starts_with(arch_specific::ARCH_CPU_MODEL_FIELD)
        {
            model = get_field_fn(line, ":", cpu_info_file)?;
        }
    }

    if vendor.is_empty() && !arch_specific::ARCH_CPU_VENDOR_FIELD.is_empty() {
        return Err(anyhow!(
            "Cannot find cpu vendor field in file : {}",
            cpu_info_file
        ));
    }

    if model.is_empty() && !arch_specific::ARCH_CPU_MODEL_FIELD.is_empty() {
        return Err(anyhow!(
            "Cannot find cpu model field in file : {}",
            cpu_info_file
        ));
    }

    Ok((vendor, model))
}

pub const VHOST_VSOCK_DEVICE: &str = "/dev/vhost-vsock";
pub fn supports_vsocks(vsock_path: &str) -> Result<bool> {
    let metadata = fs::metadata(vsock_path).map_err(|err| {
        anyhow!(
            "Host system does not support vhost-vsock (try running (`sudo modprobe vhost_vsock`) : {}",
            err.to_string()
        )
    })?;
    Ok(metadata.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    #[serial]
    fn test_drop_privs() {
        let res = drop_privs();
        assert!(res.is_ok());
    }

    #[test]
    fn test_kernel_version_empty_input() {
        let res = get_kernel_version("").unwrap_err().to_string();
        let err_msg = format!("Failed to read file {}", "");
        assert_eq!(res, err_msg);
    }

    #[test]
    #[serial]
    fn test_kernel_version_valid_input() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("proc-version");
        let path = file_path.clone();
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(
            file,
            "Linux version 5.15.0-75-generic (buildd@lcy02-amd64-045)"
        )
        .unwrap();
        let kernel = get_kernel_version(path.to_str().unwrap()).unwrap();
        assert_eq!(kernel, "5.15.0-75-generic");
    }

    #[test]
    fn test_kernel_version_system_input() {
        let res = get_kernel_version(PROC_VERSION_FILE);
        assert!(res.is_ok());
    }

    #[test]
    fn test_kernel_version_invalid_input() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("proc-version");
        let path = file_path.clone();
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(file, "Linux-version-5.15.0-75-generic").unwrap();
        let actual = get_kernel_version(path.to_str().unwrap())
            .unwrap_err()
            .to_string();
        let expected = format!("unexpected contents in file {}", path.to_str().unwrap());
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_get_distro_details_empty_files() {
        let res = get_distro_details("xvz.xt", "bar.txt");
        assert!(res.is_ok());
        let (name, version) = res.unwrap();
        assert_eq!(name, UNKNOWN);
        assert_eq!(version, UNKNOWN);
    }

    #[test]
    #[serial]
    fn test_get_distro_details_valid_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("os-version");
        let path = file_path.clone();
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(file, "NAME=Ubuntu\nID_LIKE=debian\nVERSION_ID=20.04.4\n").unwrap();
        let res = get_distro_details("/etc/foo.txt", path.to_str().unwrap());
        let (name, version) = res.unwrap();
        assert_eq!(name, "Ubuntu");
        assert_eq!(version, "20.04.4");
    }

    #[test]
    fn test_get_distro_details_system() {
        let res = get_distro_details(OS_RELEASE, OS_RELEASE_CLR);
        assert!(res.is_ok());
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn get_generic_cpu_details_system() {
        let res = get_generic_cpu_details(crate::check::PROC_CPUINFO);
        assert!(res.is_ok());
    }

    #[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
    #[test]
    fn get_generic_cpu_details_valid_contents() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("cpuinfo");
        let path = file_path.clone();
        let mut file = fs::File::create(file_path).unwrap();
        let expected_vendor_id = "GenuineIntel";
        let expected_model_name = "i7-1065G7 CPU";
        let contents = format!(
            "{} : {} \n{} : {}\n stepping: 5\n\n",
            arch_specific::ARCH_CPU_VENDOR_FIELD,
            expected_vendor_id,
            arch_specific::ARCH_CPU_MODEL_FIELD,
            expected_model_name
        );
        writeln!(file, "{}", contents).unwrap();
        let res = get_generic_cpu_details(path.to_str().unwrap());
        assert_eq!(res.as_ref().unwrap().0, expected_vendor_id);
        assert_eq!(res.as_ref().unwrap().1, expected_model_name);
    }

    #[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
    #[test]
    fn get_generic_cpu_details_invalid_file() {
        let res = get_generic_cpu_details("/tmp/missing.txt");
        assert!(res.is_err());
    }

    #[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
    #[test]
    fn get_generic_cpu_details_invalid_contents() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("cpuinfo");
        let path = file_path.clone();
        let mut file = fs::File::create(file_path).unwrap();
        writeln!(
            file,
            "vendor :GenuineIntel\nmodel_name=i7-1065G7 CPU\nstepping:5\n\n"
        )
        .unwrap();
        let actual = get_generic_cpu_details(path.to_str().unwrap())
            .unwrap_err()
            .to_string();
        let expected = format!(
            r#"Cannot find cpu vendor field in file : {}"#,
            path.to_str().unwrap()
        );
        assert_eq!(actual, expected);
    }

    #[test]
    fn check_supports_vsocks_valid() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("vhost-vsock");
        let path = file_path.clone();
        let _file = fs::File::create(file_path).unwrap();
        let res = supports_vsocks(path.to_str().unwrap()).unwrap();
        assert!(res);
    }

    #[test]
    fn check_supports_vsocks_dir() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("vhost-vsock");
        let path = file_path.clone();
        fs::create_dir(file_path).unwrap();
        let res = supports_vsocks(path.to_str().unwrap()).unwrap();
        assert!(!res);
    }

    #[test]
    fn check_supports_vsocks_missing_file() {
        let res = supports_vsocks("/xyz/vhost-vsock");
        assert!(res.is_err());
    }
}
