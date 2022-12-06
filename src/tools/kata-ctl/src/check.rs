// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Contains checks that are not architecture-specific

use anyhow::{anyhow, Context, Result};
use futures_util::TryStreamExt;
use reqwest::header::{CONTENT_TYPE, USER_AGENT};
use scopeguard::defer;
use serde_json::Value;
use std::collections::HashMap;

const KATA_GITHUB_URL: &str =
    "https://api.github.com/repos/kata-containers/kata-containers/releases/latest";

#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
fn get_cpu_info(cpu_info_file: &str) -> Result<String> {
    let contents = std::fs::read_to_string(cpu_info_file)?;
    Ok(contents)
}

// get_single_cpu_info returns the contents of the first cpu from
// the specified cpuinfo file by parsing based on a specified delimiter
#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
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
#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
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

#[cfg(any(target_arch = "x86_64"))]
pub fn check_cpu_attribs(
    cpu_info: &str,
    required_attribs: &'static [&'static str],
) -> Result<Vec<String>> {
    let mut cpu_info_processed = cpu_info.replace('\t', "");
    cpu_info_processed = cpu_info_processed.replace('\n', " ");

    let missing_attribs = get_missing_strings(&cpu_info_processed, required_attribs)?;
    Ok(missing_attribs)
}

#[tokio::main]
pub async fn run_network_checks() -> Result<()> {
    println!("Running network checks...");
    let (connection, handle, _) = rtnetlink::new_connection().context("failed to create netlink connection").unwrap();
    let thread_handler = tokio::spawn(connection);
    defer!({
        thread_handler.abort();
    });

    let host_unique_id = String::from(&kata_sys_util::rand::UUID::new());
    let vm_unique_id = String::from(&kata_sys_util::rand::UUID::new());
    let hostname = format!("kata-ctl-{}", &host_unique_id[0..5]);
    let vmname = format!("kata-ctl-{}", &vm_unique_id[0..5]);

    println!("Creating a virtual ethernet pair between {} and {}...", hostname, vmname);

    handle
        .link()
        .add()
        .veth(hostname.to_string(), vmname.to_string())
        .execute()
        .await?;

    println!("Deleting virtual ethernet pair between {} and {}...", hostname, vmname);

    let mut links = handle.link().get().match_name(hostname.clone()).execute();
    if let Some(link) = links.try_next().await? {
        handle.link().del(link.header.index).execute().await?;
    } else {
        return Err(anyhow!(format!("Link {} not found", hostname)))?;
    }

    Ok(())
}

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

pub fn check_version() -> Result<()> {
    let version = get_kata_version_by_url(KATA_GITHUB_URL).map_err(handle_reqwest_error)?;

    println!("Version: {}", version);

    Ok(())
}

#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
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

    #[test]
    fn check_version_by_empty_url() {
        const TEST_URL: &str = "http:";
        let expected = "builder error: empty host";
        let actual = get_kata_version_by_url(TEST_URL).err().unwrap().to_string();
        assert_eq!(expected, actual);
    }

    #[test]
    fn check_version_by_garbage_url() {
        const TEST_URL: &str = "_localhost_";
        let expected = "builder error: relative URL without a base";
        let actual = get_kata_version_by_url(TEST_URL).err().unwrap().to_string();
        assert_eq!(expected, actual);
    }

    #[test]
    fn check_version_by_invalid_url() {
        const TEST_URL: &str = "http://localhost :80";
        let expected = "builder error: invalid domain character";
        let actual = get_kata_version_by_url(TEST_URL).err().unwrap().to_string();
        assert_eq!(expected, actual);
    }

    #[test]
    fn check_latest_version() {
        let version = get_kata_version_by_url(KATA_GITHUB_URL).unwrap();

        let v = Version::parse(&version).unwrap();
        assert!(!v.major.to_string().is_empty());
        assert!(!v.minor.to_string().is_empty());
        assert!(!v.patch.to_string().is_empty());
    }
}
