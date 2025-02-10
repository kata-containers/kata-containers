// Copyright (c) 2025 IBM Corp.
//
// SPDX-License-Identifier: Apache-2.0
//

use super::cmdline_generator::{Machine, TeeType};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[cfg(test)]
mod test_items {
    use crate::qemu::cmdline_generator::TeeType;
    use anyhow::Error;
    use lazy_static::lazy_static;
    use std::sync::Mutex;

    lazy_static! {
        pub static ref MOCK_PROTECTION: Mutex<Option<Result<TeeType, Error>>> = Mutex::new(None);
    }
}

#[cfg(test)]
use test_items::MOCK_PROTECTION;

const SE_CPU_FACILITY_BIT: i32 = 158;
const SE_CMDLINE_PARAM: &str = "prot_virt";
const SE_CMDLINE_VALUES: &[&str] = &["1"];
const PROC_CPU_INFO: &str = "/proc/cpuinfo";
const PROC_KERNEL_CMDLINE: &str = "/proc/cmdline";
pub const SEC_EXEC_ID: &str = "pv0";

pub fn enable_s390x_protection(machine: &mut Machine) -> Result<()> {
    let protection = available_guest_protection()?;

    if protection != TeeType::Se {
        return Err(anyhow!(
            "Got unexpected protection {:?}, only SE (Secure Execution) is supported",
            protection
        ));
    }

    let options = if machine.get_options().is_empty() {
        format!("confidential-guest-support={}", SEC_EXEC_ID)
    } else {
        format!(
            "{},confidential-guest-support={}",
            machine.get_options(),
            SEC_EXEC_ID
        )
    };
    machine.set_options(options.as_str());

    info!(
        sl!(),
        "Enabling s390x Secure Execution guest protection; machine options: {}",
        machine.get_options()
    );

    Ok(())
}

#[cfg(test)]
pub(crate) fn set_mock_protection(mock_result: Option<Result<TeeType>>) {
    *MOCK_PROTECTION.lock().unwrap() = mock_result;
}

fn available_guest_protection() -> Result<TeeType> {
    #[cfg(test)]
    {
        if let Some(mock_result) = &*MOCK_PROTECTION.lock().unwrap() {
            return match mock_result {
                Ok(tee_type) => Ok(*tee_type),
                Err(e) => Err(anyhow!(e.to_string())),
            };
        }
    }

    let facilities = cpu_facilities(PROC_CPU_INFO).context("Failed to get CPU facilities")?;

    if !facilities[&SE_CPU_FACILITY_BIT] {
        return Err(anyhow!("This CPU does not support Secure Execution"));
    }

    let se_cmdline_present =
        check_cmdline(PROC_KERNEL_CMDLINE, SE_CMDLINE_PARAM, SE_CMDLINE_VALUES)
            .context("Failed to check kernel cmdline")?;

    if !se_cmdline_present {
        return Err(anyhow!(
            "Protected Virtualization is not enabled on kernel command line! Need {}={}{} to enable Secure Execution",
            SE_CMDLINE_PARAM,
            SE_CMDLINE_VALUES[0],
            if SE_CMDLINE_VALUES.len() > 1 {
                format!(" (or {})", SE_CMDLINE_VALUES[1..].join(", "))
            } else {
                String::new()
            }
        ));
    }

    Ok(TeeType::Se)
}

fn cpu_facilities(cpu_info_path: &str) -> Result<HashMap<i32, bool>> {
    const FACILITIES_FIELD: &str = "facilities";

    let file =
        File::open(cpu_info_path).with_context(|| format!("Failed to open {}", cpu_info_path))?;
    let reader = BufReader::new(file);
    let mut facilities = HashMap::new();

    for line in reader.lines() {
        let line = line?;
        let fields: Vec<&str> = line.split_whitespace().collect();

        if fields.len() < 2 {
            continue;
        }

        if !fields[0].starts_with(FACILITIES_FIELD) {
            continue;
        }

        let start = if fields[1] == ":" { 2 } else { 1 };

        for field in fields[start..].iter() {
            let bit = field
                .parse::<i32>()
                .with_context(|| format!("Failed to parse facility bit: {}", field))?;
            facilities.insert(bit, true);
        }

        return Ok(facilities);
    }

    Err(anyhow!(
        "Couldn't find '{}' from '{}' output",
        FACILITIES_FIELD,
        cpu_info_path
    ))
}

fn check_cmdline(
    kernel_cmdline_path: &str,
    search_param: &str,
    search_values: &[&str],
) -> Result<bool> {
    let file = File::open(kernel_cmdline_path)
        .with_context(|| format!("Failed to open {}", kernel_cmdline_path))?;
    let reader = BufReader::new(file);

    let check = if search_values.is_empty() {
        |option: &str, search_param: &str, _: &[&str]| option.eq_ignore_ascii_case(search_param)
    } else {
        |param: &str, search_param: &str, search_values: &[&str]| {
            let split: Vec<&str> = param.splitn(2, '=').collect();
            if split.len() < 2 || split[0] != search_param {
                return false;
            }
            search_values
                .iter()
                .any(|value| value.eq_ignore_ascii_case(split[1]))
        }
    };

    for line in reader.lines() {
        let line = line?;
        for field in line.split_whitespace() {
            if check(field, search_param, search_values) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HypervisorConfig;
    use kata_types::config::hypervisor::MachineInfo;
    use serial_test::serial;
    use std::fs::write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_cpu_facilities() -> Result<()> {
        // Test case 1: Valid facilities line with colon
        let content = "facilities : 1 2 3 158\n";
        let temp_file = NamedTempFile::new()?;
        write(&temp_file, content)?;

        let facilities = cpu_facilities(temp_file.path().to_str().unwrap())?;
        assert!(facilities.contains_key(&1));
        assert!(facilities.contains_key(&2));
        assert!(facilities.contains_key(&3));
        assert!(facilities.contains_key(&158));
        assert!(!facilities.contains_key(&4));

        // Test case 2: Valid facilities line without colon
        let content = "facilities 1 2 3\n";
        let temp_file = NamedTempFile::new()?;
        write(&temp_file, content)?;

        let facilities = cpu_facilities(temp_file.path().to_str().unwrap())?;
        assert!(facilities.contains_key(&1));
        assert!(facilities.contains_key(&2));
        assert!(facilities.contains_key(&3));
        assert!(!facilities.contains_key(&4));

        // Test case 3: No facilities line
        let content = "some other content\nwithout facilities\n";
        let temp_file = NamedTempFile::new()?;
        write(&temp_file, content)?;

        let result = cpu_facilities(temp_file.path().to_str().unwrap());
        assert!(result.is_err());

        // Test case 4: Invalid facility number
        let content = "facilities : 1 2 invalid 3\n";
        let temp_file = NamedTempFile::new()?;
        write(&temp_file, content)?;

        let result = cpu_facilities(temp_file.path().to_str().unwrap());
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_check_cmdline() -> Result<()> {
        // Test case 1: Parameter with value match
        let content = "param1=value1 prot_virt=1 param2=value2\n";
        let temp_file = NamedTempFile::new()?;
        write(&temp_file, content)?;

        let result = check_cmdline(temp_file.path().to_str().unwrap(), "prot_virt", &["1"])?;
        assert!(result);

        // Test case 2: Parameter with value no match
        let content = "param1=value1 prot_virt=2 param2=value2\n";
        let temp_file = NamedTempFile::new()?;
        write(&temp_file, content)?;

        let result = check_cmdline(temp_file.path().to_str().unwrap(), "prot_virt", &["1"])?;
        assert!(!result);

        // Test case 3: Case insensitive value no match
        let content = "param1=value1 PROT_VIRT=1 param2=value2\n";
        let temp_file = NamedTempFile::new()?;
        write(&temp_file, content)?;

        let result = check_cmdline(temp_file.path().to_str().unwrap(), "prot_virt", &["1"])?;
        assert!(!result);

        // Test case 4: Empty search values (flag-only parameter)
        let content = "param1=value1 flag_param param2=value2\n";
        let temp_file = NamedTempFile::new()?;
        write(&temp_file, content)?;

        let result = check_cmdline(temp_file.path().to_str().unwrap(), "flag_param", &[])?;
        assert!(result);

        // Test case 5: Multiple possible values
        let content = "param1=value1 prot_virt=2 param2=value2\n";
        let temp_file = NamedTempFile::new()?;
        write(&temp_file, content)?;

        let result = check_cmdline(
            temp_file.path().to_str().unwrap(),
            "prot_virt",
            &["1", "2", "3"],
        )?;
        assert!(result);

        // Test case 6: Parameter not present
        let content = "param1=value1 param2=value2\n";
        let temp_file = NamedTempFile::new()?;
        write(&temp_file, content)?;

        let result = check_cmdline(temp_file.path().to_str().unwrap(), "prot_virt", &["1"])?;
        assert!(!result);

        Ok(())
    }

    #[test]
    #[serial]
    fn test_enable_s390x_protection() -> Result<()> {
        // Test case 1: Success with SE protection
        set_mock_protection(Some(Ok(TeeType::Se)));
        let config = HypervisorConfig {
            machine_info: MachineInfo {
                machine_type: String::from("s390-ccw-virtio"),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut machine = Machine::new(&config);
        enable_s390x_protection(&mut machine)?;
        assert_eq!(
            machine.get_options(),
            format!("confidential-guest-support={}", SEC_EXEC_ID)
        );

        // Clean up mock after test
        set_mock_protection(None);
        Ok(())
    }

    #[test]
    #[serial]
    fn test_enable_s390x_protection_errors() {
        let config = HypervisorConfig {
            machine_info: MachineInfo {
                machine_type: String::from("s390-ccw-virtio"),
                ..Default::default()
            },
            ..Default::default()
        };

        // Test case 1: CPU doesn't support Secure Execution
        set_mock_protection(Some(Err(anyhow!(
            "This CPU does not support Secure Execution"
        ))));
        let mut machine = Machine::new(&config);
        let result = enable_s390x_protection(&mut machine);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("This CPU does not support Secure Execution"));

        // Test case 2: Protected Virtualization not enabled
        set_mock_protection(Some(Err(anyhow!(
            "Protected Virtualization is not enabled on kernel command line!"
        ))));
        let mut machine = Machine::new(&config);
        let result = enable_s390x_protection(&mut machine);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Protected Virtualization is not enabled on kernel command line!"));

        // Clean up mock after tests
        set_mock_protection(None);
    }
}
