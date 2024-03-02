// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate slog;

pub mod cpu;
pub mod device;
pub mod fs;
pub mod hooks;
pub mod k8s;
pub mod mount;
pub mod netns;
pub mod numa;
pub mod protection;
pub mod rand;
pub mod spec;
pub mod validate;

use anyhow::Result;
use std::io::BufRead;
use std::io::BufReader;

// Convenience macro to obtain the scoped logger
#[macro_export]
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[macro_export]
macro_rules! eother {
    () => (std::io::Error::new(std::io::ErrorKind::Other, ""));
    ($fmt:expr, $($arg:tt)*) => ({
        std::io::Error::new(std::io::ErrorKind::Other, format!($fmt, $($arg)*))
    })
}

pub fn check_kernel_cmd_line(
    kernel_cmdline_path: &str,
    search_param: &str,
    search_values: &[&str],
) -> Result<bool> {
    let f = std::fs::File::open(kernel_cmdline_path)?;
    let reader = BufReader::new(f);

    let check_fn = if search_values.is_empty() {
        |param: &str, search_param: &str, _search_values: &[&str]| {
            param.eq_ignore_ascii_case(search_param)
        }
    } else {
        |param: &str, search_param: &str, search_values: &[&str]| {
            let split: Vec<&str> = param.splitn(2, '=').collect();
            if split.len() < 2 || split[0] != search_param {
                return false;
            }

            for value in search_values {
                if value.eq_ignore_ascii_case(split[1]) {
                    return true;
                }
            }
            false
        }
    };

    for line in reader.lines() {
        for field in line?.split_whitespace() {
            if check_fn(field, search_param, search_values) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
