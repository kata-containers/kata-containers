// Copyright (C) 2024 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};

fn get_meminfo(opt: &str) -> Result<u64> {
    let file = File::open("/proc/meminfo")?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        if line.starts_with(opt) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb = parts[1].parse::<u64>()?;
                return Ok(kb);
            }
        }
    }

    Err(anyhow!("no {} found", opt))
}

pub fn get_memfree_kb() -> Result<u64> {
    get_meminfo("MemFree:")
}

pub fn get_freeswap_kb() -> Result<u64> {
    get_meminfo("SwapFree:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_memfree_kb() {
        let memfree_kb = get_memfree_kb().unwrap();
        assert!(memfree_kb > 0);
    }
}
