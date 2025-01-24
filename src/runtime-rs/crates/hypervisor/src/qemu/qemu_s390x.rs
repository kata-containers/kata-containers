// Copyright (c) 2025 IBM Corp.
//
// SPDX-License-Identifier: Apache-2.0
//

use super::cmdline_generator::{Machine, TeeType};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

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

fn available_guest_protection() -> Result<TeeType> {
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
