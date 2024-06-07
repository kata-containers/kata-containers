// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};

#[cfg(target_arch = "s390x")]
use std::collections::HashMap;
#[cfg(target_arch = "s390x")]
use std::io::BufRead;
#[cfg(target_arch = "s390x")]
use std::io::BufReader;

#[allow(dead_code)]
const ERR_NO_CPUINFO: &str = "cpu_info string is empty";

pub const PROC_CPUINFO: &str = "/proc/cpuinfo";

#[cfg(target_arch = "x86_64")]
pub const CPUINFO_DELIMITER: &str = "\nprocessor";
#[cfg(target_arch = "x86_64")]
pub const CPUINFO_FLAGS_TAG: &str = "flags";

fn read_file_contents(file_path: &str) -> Result<String> {
    let contents = std::fs::read_to_string(file_path)?;
    Ok(contents)
}

// get_single_cpu_info returns the contents of the first cpu from
// the specified cpuinfo file by parsing based on a specified delimiter
pub fn get_single_cpu_info(cpu_info_file: &str, substring: &str) -> Result<String> {
    let contents = read_file_contents(cpu_info_file)?;

    if contents.is_empty() {
        return Err(anyhow!(ERR_NO_CPUINFO));
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
        return Err(anyhow!(ERR_NO_CPUINFO));
    }

    if cpu_flags_tag.is_empty() {
        return Err(anyhow!("cpu flags delimiter string is empty"));
    }

    get_cpu_flags_from_file(cpu_info, cpu_flags_tag)
}

// get a list of cpu flags in cpu_info_flags
//
// cpu_info is the content of cpuinfo file passed in as a string
// returns empty Vec if no flags are found
#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
pub fn get_cpu_flags_vec(cpu_info: &str, cpu_flags_tag: &str) -> Result<Vec<String>> {
    if cpu_info.is_empty() {
        return Err(anyhow!(ERR_NO_CPUINFO));
    }

    if cpu_flags_tag.is_empty() {
        return Err(anyhow!("cpu flags delimiter string is empty"));
    }

    let flags = get_cpu_flags_from_file(cpu_info, cpu_flags_tag)?;

    // take each flag, trim whitespace, convert to String, and add to list
    // skip the first token in the iterator since it is empty
    let flags_vec: Vec<String> = flags
        .split(' ')
        .skip(1)
        .map(|f| f.trim().to_string())
        .collect::<Vec<String>>();

    Ok(flags_vec)
}

// check if the given flag exists in the given flags_vec
//
// flags_vec can be created by calling get_cpu_flags_vec
#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
pub fn contains_cpu_flag(flags_vec: &[String], flag: &str) -> Result<bool> {
    if flag.is_empty() {
        return Err(anyhow!("parameter specifying flag to look for is empty"));
    }

    Ok(flags_vec.iter().any(|f| f == flag))
}

// get a String containing the cpu flags in cpu_info
//
// this function returns the list of flags as a single String
// if no flags are found, returns an empty String
#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
fn get_cpu_flags_from_file(cpu_info: &str, cpu_flags_tag: &str) -> Result<String> {
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

#[cfg(target_arch = "s390x")]
pub fn retrieve_cpu_facilities() -> Result<HashMap<i32, bool>> {
    let f = std::fs::File::open(PROC_CPUINFO)?;
    let mut reader = BufReader::new(f);
    let mut contents = String::new();
    let facilities_field = "facilities";
    let mut facilities = HashMap::new();

    while reader.read_line(&mut contents)? > 0 {
        let fields: Vec<&str> = contents.split_whitespace().collect();
        if fields.len() < 2 {
            contents.clear();
            continue;
        }

        if !fields[0].starts_with(facilities_field) {
            contents.clear();
            continue;
        }

        let mut start = 1;
        if fields[1] == ":" {
            start = 2;
        }

        for field in fields.iter().skip(start) {
            let bit = field.parse::<i32>()?;
            facilities.insert(bit, true);
        }
        return Ok(facilities);
    }

    Ok(facilities)
}

#[cfg(any(target_arch = "s390x", target_arch = "x86_64"))]
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

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
    fn test_get_cpu_flags_vec() {
        let contents = "processor : 0\nvendor_id : VendorExample\nflags : flag_1 flag_2 flag_3 flag_4\nprocessor : 1\n";

        #[derive(Debug)]
        struct TestData<'a> {
            cpu_info_str: &'a str,
            cpu_flags_tag: &'a str,
            result: Result<Vec<String>>,
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
                result: Ok(vec![
                    "flag_1".to_string(),
                    "flag_2".to_string(),
                    "flag_3".to_string(),
                    "flag_4".to_string(),
                ]),
            },
            TestData {
                cpu_info_str: contents,
                cpu_flags_tag: "flags_err",
                result: Ok(Vec::new()),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);
            let result = get_cpu_flags_vec(d.cpu_info_str, d.cpu_flags_tag);
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
    fn test_contains_cpu_flag() {
        let flags_vec = vec![
            "flag_1".to_string(),
            "flag_2".to_string(),
            "flag_3".to_string(),
            "flag_4".to_string(),
        ];

        #[derive(Debug)]
        struct TestData<'a> {
            cpu_flags_vec: &'a Vec<String>,
            cpu_flag: &'a str,
            result: Result<bool>,
        }
        let tests = &[
            // Failure scenarios
            TestData {
                cpu_flags_vec: &flags_vec,
                cpu_flag: "flag_5",
                result: Ok(false),
            },
            TestData {
                cpu_flags_vec: &flags_vec,
                cpu_flag: "",
                result: Err(anyhow!("parameter specifying flag to look for is empty")),
            },
            // Success scenarios
            TestData {
                cpu_flags_vec: &flags_vec,
                cpu_flag: "flag_1",
                result: Ok(true),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);
            let result = contains_cpu_flag(d.cpu_flags_vec, d.cpu_flag);
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
}
