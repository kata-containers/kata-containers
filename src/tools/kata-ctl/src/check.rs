// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Contains checks that are not architecture-specific

use anyhow::{anyhow, Result};
use std::fs;

fn get_cpu_info(cpu_info_file: &str) -> Result<String> {
    let contents = fs::read_to_string(cpu_info_file)?;
    Ok(contents)
}

// get_single_cpu_info returns the contents of the first cpu from
// the specified cpuinfo file by parsing based on a specified delimiter
pub fn get_single_cpu_info(cpu_info_file: &str, substring: &str) -> Result<String> {
    let contents = get_cpu_info(cpu_info_file)?; 

    if contents.is_empty() {
        return Err(anyhow!("cpu_info string is empty"))?;
    }

    let subcontents: Vec<&str> = contents.split(substring).collect();
    let result = subcontents
        .first()
        .ok_or("error splitting contents of cpuinfo")
        .map_err(|e| anyhow!(e))?
        .to_string();

    Ok(result)
}

// get_cpu_flags returns a string of cpu flags from cpuinfo, passed in
// as a string
pub fn get_cpu_flags(cpu_info: &str, cpu_flags_tag: &str) -> Result<String> {
    if cpu_info.is_empty() {
        return Err(anyhow!("cpu_info string is empty"))?;
    }

    let subcontents: Vec<&str> = cpu_info.split("\n").collect();
    for line in subcontents {
        if line.starts_with(cpu_flags_tag) {
            let line_data: Vec<&str> = line.split(":").collect();
            let flags = line_data
                .last()
                .ok_or("error splitting flags in cpuinfo")
                .map_err(|e| anyhow!(e))?
                .to_string();
            return Ok(flags);
        }
    }

    Ok("".to_string())
}

// get_missing_strings searches for required (strings) in data and returns
// a vector containing the missing strings
fn get_missing_strings(data: &str, required: &'static [&'static str]) -> Result<Vec<String>> {
    let data_vec: Vec <&str> = data.split_whitespace().collect();

    let mut missing: Vec <String>  = Vec::new();

    for item in required {
        if !data_vec.contains(&item) {
            missing.push(item.to_string());
        }
    }

    Ok(missing)
}

pub fn check_cpu_flags(retrieved_flags: &str, required_flags: &'static [&'static str]) -> Result<Vec<String>> {
    let missing_flags = get_missing_strings(retrieved_flags, required_flags)?;

    Ok(missing_flags)
}

pub fn check_cpu_attribs(cpu_info: &str, required_attribs: &'static [&'static str]) -> Result<Vec<String>> {
    let mut cpu_info_processed = cpu_info.replace("\t", "");
    cpu_info_processed = cpu_info_processed.replace("\n", " ");

    let missing_attribs = get_missing_strings(&cpu_info_processed, required_attribs)?;
    Ok(missing_attribs)
}

pub fn run_network_checks() -> Result<()> {
    Ok(())
}

pub fn check_version() -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cpu_info_empty_input() {
        let expected = "No such file or directory (os error 2)";
        let actual = get_cpu_info("").err().unwrap().to_string();
        assert_eq!(expected, actual);

        let actual = get_single_cpu_info("", "\nprocessor").err().unwrap().to_string();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_get_cpu_flags_empty_input() {
        let expected = "cpu_info string is empty";
        let actual = get_cpu_flags("", "").err().unwrap().to_string();
        assert_eq!(expected, actual);
    }
}
