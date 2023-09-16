// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Contains checks that are not architecture-specific

#[cfg(target_arch = "x86_64")]
use crate::types::KernelModule;

use anyhow::{anyhow, Result};
use nix::fcntl::{open, OFlag};
use nix::sys::stat::Mode;
use nix::unistd::close;
use nix::{ioctl_write_int_bad, request_code_none};
use reqwest::header::{CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use slog::{info, o};

#[cfg(target_arch = "x86_64")]
use std::process::{Command, Stdio};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
struct Release {
    tag_name: String,
    prerelease: bool,
    created_at: String,
    tarball_url: String,
}

#[allow(dead_code)]
const MODPROBE_PATH: &str = "/sbin/modprobe";

#[allow(dead_code)]
const MODINFO_PATH: &str = "/sbin/modinfo";

const KATA_GITHUB_RELEASE_URL: &str =
    "https://api.github.com/repos/kata-containers/kata-containers/releases";

const JSON_TYPE: &str = "application/json";

const USER_AGT: &str = "kata";

#[allow(dead_code)]
const ERR_NO_CPUINFO: &str = "cpu_info string is empty";

#[allow(dead_code)]
pub const GENERIC_CPU_VENDOR_FIELD: &str = "vendor_id";

#[allow(dead_code)]
pub const GENERIC_CPU_MODEL_FIELD: &str = "model name";

#[allow(dead_code)]
pub const PROC_CPUINFO: &str = "/proc/cpuinfo";

macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "check"))
    };
}

// get_missing_strings searches for required (strings) in data and returns
// a vector containing the missing strings
#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
fn get_missing_strings(data: &str, required: &'static [&'static str]) -> Result<Vec<String>> {
    let mut missing: Vec<String> = Vec::new();

    for item in required {
        if !data.split_whitespace().any(|x| x == *item) {
            missing.push(item.to_string());
        }
    }

    Ok(missing)
}

#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
pub fn check_cpu_flags(
    retrieved_flags: &str,
    required_flags: &'static [&'static str],
) -> Result<Vec<String>> {
    let missing_flags = get_missing_strings(retrieved_flags, required_flags)?;

    Ok(missing_flags)
}

#[cfg(target_arch = "x86_64")]
pub fn check_cpu_attribs(
    cpu_info: &str,
    required_attribs: &'static [&'static str],
) -> Result<Vec<String>> {
    let mut cpu_info_processed = cpu_info.replace('\t', "");
    cpu_info_processed = cpu_info_processed.replace('\n', " ");

    let missing_attribs = get_missing_strings(&cpu_info_processed, required_attribs)?;
    Ok(missing_attribs)
}

pub fn run_network_checks() -> Result<()> {
    Ok(())
}

// Set of basic checks for kvm. Architectures should implement more specific checks if needed
#[allow(dead_code)]
pub fn check_kvm_is_usable_generic() -> Result<()> {
    // check for root user
    if !nix::unistd::Uid::effective().is_root() {
        return Err(anyhow!("Will not perform kvm checks as non root user"));
    }

    // we do not want to create syscalls to any device besides /dev/kvm
    const KVM_DEVICE: &str = "/dev/kvm";

    // constants specific to kvm ioctls found in kvm.h
    const KVM_IOCTL_ID: u8 = 0xAE;
    const KVM_CREATE_VM: u8 = 0x01;
    const KVM_GET_API_VERSION: u8 = 0x00;
    // per kvm api documentation, this number should always be 12
    // https://www.kernel.org/doc/html/latest/virt/kvm/api.html#kvm-get-api-version
    const API_VERSION: i32 = 12;

    // open kvm device
    // since file is not being created, mode argument is not relevant
    let mode = Mode::empty();
    let flags = OFlag::O_RDWR | OFlag::O_CLOEXEC;
    let fd = open(KVM_DEVICE, flags, mode)?;

    // check kvm api version
    ioctl_write_int_bad!(
        kvm_api_version,
        request_code_none!(KVM_IOCTL_ID, KVM_GET_API_VERSION)
    );
    // 0 is not used but required to produce output
    let v = unsafe { kvm_api_version(fd, 0)? };
    if v != API_VERSION {
        return Err(anyhow!("KVM API version is not correct"));
    }

    // check if you can create vm
    ioctl_write_int_bad!(
        kvm_create_vm,
        request_code_none!(KVM_IOCTL_ID, KVM_CREATE_VM)
    );
    // 0 is default machine type
    let vmfd = unsafe { kvm_create_vm(fd, 0) };
    let _vmfd = match vmfd {
        Ok(vm) => vm,
        Err(ref error) if error.to_string() == "EBUSY: Device or resource busy" => {
            return Err(anyhow!(
                "Another hypervisor is running. KVM_CREATE_VM error: {:?}",
                error
            ))
        }
        Err(error) => return Err(anyhow!("Other KVM_CREATE_VM error: {:?}", error)),
    };

    let _ = close(fd);

    Ok(())
}

fn get_kata_all_releases_by_url(url: &str) -> std::result::Result<Vec<Release>, reqwest::Error> {
    let releases: Vec<Release> = reqwest::blocking::Client::new()
        .get(url)
        .header(CONTENT_TYPE, JSON_TYPE)
        .header(USER_AGENT, USER_AGT)
        .send()?
        .error_for_status()?
        .json()?;
    Ok(releases)
}

fn handle_reqwest_error(e: reqwest::Error) -> anyhow::Error {
    if e.is_connect() {
        return anyhow!(e).context("http connection failure: connection refused");
    }

    if e.is_timeout() {
        return anyhow!(e).context("http connection failure: connection timeout");
    }

    if e.is_builder() {
        return anyhow!(e).context("http connection failure: url malformed");
    }

    if e.is_decode() {
        return anyhow!(e).context("http connection failure: unable to decode response body");
    }

    anyhow!(e).context("unknown http connection failure: {:?}")
}

pub fn check_all_releases() -> Result<()> {
    let releases: Vec<Release> =
        get_kata_all_releases_by_url(KATA_GITHUB_RELEASE_URL).map_err(handle_reqwest_error)?;

    for release in releases {
        if !release.prerelease {
            info!(
                sl!(),
                "Official  : Release {:15}; created {} ; {}",
                release.tag_name,
                release.created_at,
                release.tarball_url
            );
        } else {
            info!(
                sl!(),
                "PreRelease: Release {:15}; created {} ; {}",
                release.tag_name,
                release.created_at,
                release.tarball_url
            );
        }
    }
    Ok(())
}

pub fn check_official_releases() -> Result<()> {
    let releases: Vec<Release> =
        get_kata_all_releases_by_url(KATA_GITHUB_RELEASE_URL).map_err(handle_reqwest_error)?;

    info!(sl!(), "Official Releases...");
    for release in releases {
        if !release.prerelease {
            info!(
                sl!(),
                "Release {:15}; created {} ; {}",
                release.tag_name,
                release.created_at,
                release.tarball_url
            );
        }
    }

    Ok(())
}

#[cfg(target_arch = "x86_64")]
pub fn check_kernel_module_loaded(kernel_module: &KernelModule) -> Result<(), String> {
    const MODPROBE_PARAMETERS_DRY_RUN: &str = "--dry-run";
    const MODPROBE_PARAMETERS_FIRST_TIME: &str = "--first-time";

    let status_modinfo_success;

    // Partial check w/ modinfo
    // verifies that the module exists
    match Command::new(MODINFO_PATH)
        .arg(kernel_module.name)
        .stdout(Stdio::piped())
        .output()
    {
        Ok(v) => {
            status_modinfo_success = v.status.success();

            // The module is already not loaded.
            if !status_modinfo_success {
                let msg = String::from_utf8_lossy(&v.stderr).replace('\n', "");
                return Err(msg);
            }
        }
        Err(_e) => {
            let msg = format!(
                "Command {:} not found, verify that `kmod` package is already installed.",
                MODINFO_PATH,
            );
            return Err(msg);
        }
    }

    // Partial check w/ modprobe
    // check that the module is already loaded
    match Command::new(MODPROBE_PATH)
        .arg(MODPROBE_PARAMETERS_DRY_RUN)
        .arg(MODPROBE_PARAMETERS_FIRST_TIME)
        .arg(kernel_module.name)
        .stdout(Stdio::piped())
        .output()
    {
        Ok(v) => {
            // a successful simulated modprobe insert, means the module is not already loaded
            let status_modprobe_success = v.status.success();

            if status_modprobe_success && status_modinfo_success {
                // This condition is true in the case that the module exist, but is not already loaded
                let msg = format!("The kernel module `{:}` exist but is not already loaded. Try reloading it using 'modprobe {:}'",
                kernel_module.name, kernel_module.name
                    );
                return Err(msg);
            }
        }

        Err(_e) => {
            let msg = format!(
                "Command {:} not found, verify that `kmod` package is already installed.",
                MODPROBE_PATH,
            );
            return Err(msg);
        }
    }
    Ok(())
}

#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_arch = "x86_64")]
    use crate::types::{KernelModule, KernelParam, KernelParamType};
    use kata_sys_util::cpu::{get_cpu_flags, get_single_cpu_info};
    use semver::Version;
    use slog::warn;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;
    use test_utils::skip_if_root;

    #[test]
    fn test_get_single_cpu_info() {
        // Valid cpuinfo example
        let dir = tempdir().unwrap();
        let file_path_full = dir.path().join("cpuinfo_full");
        let path_full = file_path_full.clone();
        let mut file_full = fs::File::create(file_path_full).unwrap();
        let contents = "processor : 0\nvendor_id : VendorExample\nflags : flag_1 flag_2 flag_3 flag_4\nprocessor : 1\n".to_string();
        writeln!(file_full, "{}", contents).unwrap();

        // Empty cpuinfo example
        let file_path_empty = dir.path().join("cpuinfo_empty");
        let path_empty = file_path_empty.clone();
        let mut _file_empty = fs::File::create(file_path_empty).unwrap();

        #[derive(Debug)]
        struct TestData<'a> {
            cpuinfo_path: &'a str,
            processor_delimiter_str: &'a str,
            result: Result<String>,
        }
        let tests = &[
            // Failure scenarios
            TestData {
                cpuinfo_path: "",
                processor_delimiter_str: "",
                result: Err(anyhow!("No such file or directory (os error 2)")),
            },
            TestData {
                cpuinfo_path: &path_empty.as_path().display().to_string(),
                processor_delimiter_str: "\nprocessor",
                result: Err(anyhow!(ERR_NO_CPUINFO)),
            },
            // Success scenarios
            TestData {
                cpuinfo_path: &path_full.as_path().display().to_string(),
                processor_delimiter_str: "\nprocessor",
                result: Ok(
                    "processor : 0\nvendor_id : VendorExample\nflags : flag_1 flag_2 flag_3 flag_4"
                        .to_string(),
                ),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);
            let result = get_single_cpu_info(d.cpuinfo_path, d.processor_delimiter_str);
            let msg = format!("{}, result: {:?}", msg, result);

            if d.result.is_ok() {
                assert_eq!(
                    result.as_ref().unwrap(),
                    d.result.as_ref().unwrap(),
                    "{}",
                    msg
                );
                continue;
            }

            let expected_error = format!("{}", d.result.as_ref().unwrap_err());
            let actual_error = format!("{}", result.unwrap_err());
            assert!(actual_error == expected_error, "{}", msg);
        }
    }

    #[test]
    fn test_get_cpu_flags() {
        let contents = "processor : 0\nvendor_id : VendorExample\nflags : flag_1 flag_2 flag_3 flag_4\nprocessor : 1\n";

        #[derive(Debug)]
        struct TestData<'a> {
            cpu_info_str: &'a str,
            cpu_flags_tag: &'a str,
            result: Result<String>,
        }
        let tests = &[
            // Failure scenarios
            TestData {
                cpu_info_str: "",
                cpu_flags_tag: "",
                result: Err(anyhow!(ERR_NO_CPUINFO)),
            },
            TestData {
                cpu_info_str: "",
                cpu_flags_tag: "flags",
                result: Err(anyhow!(ERR_NO_CPUINFO)),
            },
            TestData {
                cpu_info_str: contents,
                cpu_flags_tag: "",
                result: Err(anyhow!("cpu flags delimiter string is empty")),
            },
            // Success scenarios
            TestData {
                cpu_info_str: contents,
                cpu_flags_tag: "flags",
                result: Ok(" flag_1 flag_2 flag_3 flag_4".to_string()),
            },
            TestData {
                cpu_info_str: contents,
                cpu_flags_tag: "flags_err",
                result: Ok("".to_string()),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);
            let result = get_cpu_flags(d.cpu_info_str, d.cpu_flags_tag);
            let msg = format!("{}, result: {:?}", msg, result);

            if d.result.is_ok() {
                assert_eq!(
                    result.as_ref().unwrap(),
                    d.result.as_ref().unwrap(),
                    "{}",
                    msg
                );
                continue;
            }

            let expected_error = format!("{}", d.result.as_ref().unwrap_err());
            let actual_error = format!("{}", result.unwrap_err());
            assert!(actual_error == expected_error, "{}", msg);
        }
    }

    #[test]
    fn test_check_kvm_is_usable_generic() {
        skip_if_root!();
        #[allow(dead_code)]
        let result = check_kvm_is_usable_generic();
        assert!(
            result.err().unwrap().to_string() == "Will not perform kvm checks as non root user"
        );
    }

    #[test]
    fn test_get_kata_all_releases_by_url() {
        #[derive(Debug)]
        struct TestData<'a> {
            test_url: &'a str,
            expected: &'a str,
        }
        let tests = &[
            // Failure scenarios
            TestData {
                test_url: "http:",
                expected: "builder error: empty host",
            },
            TestData {
                test_url: "_localhost_",
                expected: "builder error: relative URL without a base",
            },
            TestData {
                test_url: "http://localhost :80",
                expected: "builder error: invalid domain character",
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);
            let actual = get_kata_all_releases_by_url(d.test_url)
                .err()
                .unwrap()
                .to_string();
            let msg = format!("{}, result: {:?}", msg, actual);
            assert_eq!(d.expected, actual, "{}", msg);
        }
    }

    #[test]
    fn check_latest_version() {
        let releases = get_kata_all_releases_by_url(KATA_GITHUB_RELEASE_URL);
        // sometime in GitHub action accessing to github.com API may fail
        // we can skip this test to prevent the whole test fail.
        if releases.is_err() {
            warn!(
                sl!(),
                "get kata version failed({:?}), this maybe a temporary error, just skip the test.",
                releases.unwrap_err()
            );
            return;
        }
        let releases = releases.unwrap();

        assert!(!releases.is_empty());
        let release = &releases[0];

        let v = Version::parse(&release.tag_name).unwrap();
        assert!(!v.major.to_string().is_empty());
        assert!(!v.minor.to_string().is_empty());
        assert!(!v.patch.to_string().is_empty());
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn check_module_loaded() {
        #[allow(dead_code)]

        struct TestData<'a> {
            module_name: &'a str,
            param_name: &'a str,
            kernel_module: &'a KernelModule<'a>,
            param_value: &'a str,
            result: Result<()>,
        }

        let tests = &[
            // Failure scenarios
            TestData {
                module_name: "",
                param_name: "",
                kernel_module: &KernelModule {
                    name: "",
                    params: &[KernelParam {
                        name: "",
                        value: KernelParamType::Simple("Y"),
                    }],
                },
                param_value: "",
                result: Err(anyhow!("modinfo: ERROR: Module {} not found.", "")),
            },
            // Success scenarios
            TestData {
                module_name: "loop",
                param_name: "",
                kernel_module: &KernelModule {
                    name: "loop",
                    params: &[KernelParam {
                        name: "nonexistantparam",
                        value: KernelParamType::Simple("Y"),
                    }],
                },
                param_value: "",
                result: Ok(()),
            },
            TestData {
                module_name: "loop",
                param_name: "hw_queue_depth",
                kernel_module: &KernelModule {
                    name: "loop",
                    params: &[KernelParam {
                        name: "hw_queue_depth",
                        value: KernelParamType::Simple("128"),
                    }],
                },
                param_value: "128",
                result: Ok(()),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]", i);
            let result = check_kernel_module_loaded(d.kernel_module);
            let msg = format!("{}, result: {:?}", msg, result);

            if d.result.is_ok() {
                assert_eq!(result, Ok(()));
                continue;
            }

            let expected_error = format!("{}", &d.result.as_ref().unwrap_err());
            let actual_error = result.unwrap_err().to_string();
            println!("testing for {}", d.module_name);
            assert!(actual_error == expected_error, "{}", msg);
        }
    }
}
