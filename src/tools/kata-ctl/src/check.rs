// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Contains checks that are not architecture-specific

use anyhow::{anyhow, Result};
// See: https://github.com/kata-containers/kata-containers/issues/5438
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "powerpc64le",
    target_arch = "x86_64"
))]
use reqwest::header::{CONTENT_TYPE, USER_AGENT};
use serde_json::Value;
#[cfg(any(
    target_arch = "aarch64",
    target_arch = "powerpc64le",
    target_arch = "x86_64"
))]
use std::collections::HashMap;
use std::fs;

const KATA_GITHUB_URL: &str =
    "https://api.github.com/repos/kata-containers/kata-containers/releases/latest";

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

    let subcontents: Vec<&str> = cpu_info.split('\n').collect();
    for line in subcontents {
        if line.starts_with(cpu_flags_tag) {
            let line_data: Vec<&str> = line.split(':').collect();
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
    let mut missing: Vec<String> = Vec::new();

    for item in required {
        if !data.split_whitespace().any(|x| x == *item) {
            missing.push(item.to_string());
        }
    }

    Ok(missing)
}

pub fn check_cpu_flags(
    retrieved_flags: &str,
    required_flags: &'static [&'static str],
) -> Result<Vec<String>> {
    let missing_flags = get_missing_strings(retrieved_flags, required_flags)?;

    Ok(missing_flags)
}

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

#[cfg(any(
    target_arch = "aarch64",
    target_arch = "powerpc64le",
    target_arch = "x86_64"
))]
fn get_kata_version_by_url(url: &str) -> std::result::Result<String, reqwest::Error> {
    let content = reqwest::blocking::Client::new()
        .get(url)
        .header(CONTENT_TYPE, "application/json")
        .header(USER_AGENT, "kata")
        .send()?
        .json::<HashMap<String, Value>>()?;

    let version = content["tag_name"].as_str().unwrap();
    Ok(version.to_string())
}

#[cfg(any(
    target_arch = "aarch64",
    target_arch = "powerpc64le",
    target_arch = "x86_64"
))]
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

#[cfg(any(
    target_arch = "aarch64",
    target_arch = "powerpc64le",
    target_arch = "x86_64"
))]
pub fn check_version() -> Result<()> {
    let version = get_kata_version_by_url(KATA_GITHUB_URL).map_err(handle_reqwest_error)?;

    println!("Version: {}", version);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;

    #[test]
    fn test_get_cpu_info_empty_input() {
        let expected = "No such file or directory (os error 2)";
        let actual = get_cpu_info("").err().unwrap().to_string();
        assert_eq!(expected, actual);

        let actual = get_single_cpu_info("", "\nprocessor")
            .err()
            .unwrap()
            .to_string();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_get_cpu_flags_empty_input() {
        let expected = "cpu_info string is empty";
        let actual = get_cpu_flags("", "").err().unwrap().to_string();
        assert_eq!(expected, actual);
    }

    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "powerpc64le",
        target_arch = "x86_64"
    ))]
    #[test]
    fn check_version_by_empty_url() {
        const TEST_URL: &str = "http:";
        let expected = "builder error: empty host";
        let actual = get_kata_version_by_url(TEST_URL).err().unwrap().to_string();
        assert_eq!(expected, actual);
    }

    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "powerpc64le",
        target_arch = "x86_64"
    ))]
    #[test]
    fn check_version_by_garbage_url() {
        const TEST_URL: &str = "_localhost_";
        let expected = "builder error: relative URL without a base";
        let actual = get_kata_version_by_url(TEST_URL).err().unwrap().to_string();
        assert_eq!(expected, actual);
    }

    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "powerpc64le",
        target_arch = "x86_64"
    ))]
    #[test]
    fn check_version_by_invalid_url() {
        const TEST_URL: &str = "http://localhost :80";
        let expected = "builder error: invalid domain character";
        let actual = get_kata_version_by_url(TEST_URL).err().unwrap().to_string();
        assert_eq!(expected, actual);
    }

    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "powerpc64le",
        target_arch = "x86_64"
    ))]
    #[test]
    fn check_latest_version() {
        let version = get_kata_version_by_url(KATA_GITHUB_URL).unwrap();

        let v = Version::parse(&version).unwrap();
        assert!(!v.major.to_string().is_empty());
        assert!(!v.minor.to_string().is_empty());
        assert!(!v.patch.to_string().is_empty());
    }
}
