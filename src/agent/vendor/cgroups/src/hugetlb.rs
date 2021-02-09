// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `hugetlb` cgroup subsystem.
//!
//! See the Kernel's documentation for more information about this subsystem, found at:
//!  [Documentation/cgroup-v1/hugetlb.txt](https://www.kernel.org/doc/Documentation/cgroup-v1/hugetlb.txt)
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::ErrorKind::*;
use crate::error::*;
use crate::flat_keyed_to_vec;

use crate::{
    ControllIdentifier, ControllerInternal, Controllers, HugePageResources, Resources, Subsystem,
};

/// A controller that allows controlling the `hugetlb` subsystem of a Cgroup.
///
/// In essence, using this controller it is possible to limit the use of hugepages in the tasks of
/// the control group.
#[derive(Debug, Clone)]
pub struct HugeTlbController {
    base: PathBuf,
    path: PathBuf,
    sizes: Vec<String>,
    v2: bool,
}

impl ControllerInternal for HugeTlbController {
    fn control_type(&self) -> Controllers {
        Controllers::HugeTlb
    }
    fn get_path(&self) -> &PathBuf {
        &self.path
    }
    fn get_path_mut(&mut self) -> &mut PathBuf {
        &mut self.path
    }
    fn get_base(&self) -> &PathBuf {
        &self.base
    }

    fn is_v2(&self) -> bool {
        self.v2
    }

    fn apply(&self, res: &Resources) -> Result<()> {
        // get the resources that apply to this controller
        let res: &HugePageResources = &res.hugepages;

        if res.update_values {
            for i in &res.limits {
                let _ = self.set_limit_in_bytes(&i.size, i.limit);
                if self.limit_in_bytes(&i.size)? != i.limit {
                    return Err(Error::new(Other));
                }
            }
        }
        Ok(())
    }
}

impl ControllIdentifier for HugeTlbController {
    fn controller_type() -> Controllers {
        Controllers::HugeTlb
    }
}

impl<'a> From<&'a Subsystem> for &'a HugeTlbController {
    fn from(sub: &'a Subsystem) -> &'a HugeTlbController {
        unsafe {
            match sub {
                Subsystem::HugeTlb(c) => c,
                _ => {
                    assert_eq!(1, 0);
                    ::std::mem::uninitialized()
                }
            }
        }
    }
}

fn read_u64_from(mut file: File) -> Result<u64> {
    let mut string = String::new();
    match file.read_to_string(&mut string) {
        Ok(_) => string
            .trim()
            .parse()
            .map_err(|e| Error::with_cause(ParseError, e)),
        Err(e) => Err(Error::with_cause(ReadFailed, e)),
    }
}

impl HugeTlbController {
    /// Constructs a new `HugeTlbController` with `oroot` serving as the root of the control group.
    pub fn new(oroot: PathBuf, v2: bool) -> Self {
        let mut root = oroot;
        if !v2 {
            root.push(Self::controller_type().to_string());
        }
        let sizes = get_hugepage_sizes().unwrap();
        Self {
            base: root.clone(),
            path: root,
            sizes: sizes,
            v2: v2,
        }
    }

    /// Whether the system supports `hugetlb_size` hugepages.
    pub fn size_supported(&self, hugetlb_size: &str) -> bool {
        for s in &self.sizes {
            if s == hugetlb_size {
                return true;
            }
        }
        false
    }

    pub fn get_sizes(&self) -> Vec<String> {
        self.sizes.clone()
    }

    fn failcnt_v2(&self, hugetlb_size: &str) -> Result<u64> {
        self.open_path(&format!("hugetlb.{}.events", hugetlb_size), false)
            .and_then(flat_keyed_to_vec)
            .and_then(|x| {
                if x.len() == 0 {
                    return Err(Error::from_string(format!(
                        "get empty from hugetlb.{}.events",
                        hugetlb_size
                    )));
                }
                Ok(x[0].1 as u64)
            })
    }

    /// Check how many times has the limit of `hugetlb_size` hugepages been hit.
    pub fn failcnt(&self, hugetlb_size: &str) -> Result<u64> {
        if self.v2 {
            return self.failcnt_v2(hugetlb_size);
        }
        self.open_path(&format!("hugetlb.{}.failcnt", hugetlb_size), false)
            .and_then(read_u64_from)
    }

    /// Get the limit (in bytes) of how much memory can be backed by hugepages of a certain size
    /// (`hugetlb_size`).
    pub fn limit_in_bytes(&self, hugetlb_size: &str) -> Result<u64> {
        self.open_path(&format!("hugetlb.{}.limit_in_bytes", hugetlb_size), false)
            .and_then(read_u64_from)
    }

    /// Get the current usage of memory that is backed by hugepages of a certain size
    /// (`hugetlb_size`).
    pub fn usage_in_bytes(&self, hugetlb_size: &str) -> Result<u64> {
        let mut file = format!("hugetlb.{}.usage_in_bytes", hugetlb_size);
        if self.v2 {
            file = format!("hugetlb.{}.current", hugetlb_size);
        }
        self.open_path(&file, false).and_then(read_u64_from)
    }

    /// Get the maximum observed usage of memory that is backed by hugepages of a certain size
    /// (`hugetlb_size`).
    pub fn max_usage_in_bytes(&self, hugetlb_size: &str) -> Result<u64> {
        self.open_path(
            &format!("hugetlb.{}.max_usage_in_bytes", hugetlb_size),
            false,
        )
        .and_then(read_u64_from)
    }

    /// Set the limit (in bytes) of how much memory can be backed by hugepages of a certain size
    /// (`hugetlb_size`).
    pub fn set_limit_in_bytes(&self, hugetlb_size: &str, limit: u64) -> Result<()> {
        let mut file = format!("hugetlb.{}.limit_in_bytes", hugetlb_size);
        if self.v2 {
            file = format!("hugetlb.{}.max", hugetlb_size);
        }
        self.open_path(&file, true).and_then(|mut file| {
            file.write_all(limit.to_string().as_ref())
                .map_err(|e| Error::with_cause(WriteFailed, e))
        })
    }
}

pub const HUGEPAGESIZE_DIR: &'static str = "/sys/kernel/mm/hugepages";
use regex::Regex;
use std::collections::HashMap;
use std::fs;

fn get_hugepage_sizes() -> Result<Vec<String>> {
    let mut m = Vec::new();
    let dirs = fs::read_dir(HUGEPAGESIZE_DIR);
    if dirs.is_err() {
        return Ok(m);
    }

    for e in dirs.unwrap() {
        let entry = e.unwrap();
        let name = entry.file_name().into_string().unwrap();
        let parts: Vec<&str> = name.split('-').collect();
        if parts.len() != 2 {
            continue;
        }
        let bmap = get_binary_size_map();
        let size = parse_size(parts[1], &bmap)?;
        let dabbrs = get_decimal_abbrs();
        m.push(custom_size(size as f64, 1024.0, &dabbrs));
    }

    Ok(m)
}

pub const KB: u128 = 1000;
pub const MB: u128 = 1000 * KB;
pub const GB: u128 = 1000 * MB;
pub const TB: u128 = 1000 * GB;
pub const PB: u128 = 1000 * TB;

pub const KiB: u128 = 1024;
pub const MiB: u128 = 1024 * KiB;
pub const GiB: u128 = 1024 * MiB;
pub const TiB: u128 = 1024 * GiB;
pub const PiB: u128 = 1024 * TiB;

pub fn get_binary_size_map() -> HashMap<String, u128> {
    let mut m = HashMap::new();
    m.insert("k".to_string(), KiB);
    m.insert("m".to_string(), MiB);
    m.insert("g".to_string(), GiB);
    m.insert("t".to_string(), TiB);
    m.insert("p".to_string(), PiB);
    m
}

pub fn get_decimal_size_map() -> HashMap<String, u128> {
    let mut m = HashMap::new();
    m.insert("k".to_string(), KB);
    m.insert("m".to_string(), MB);
    m.insert("g".to_string(), GB);
    m.insert("t".to_string(), TB);
    m.insert("p".to_string(), PB);
    m
}

pub fn get_decimal_abbrs() -> Vec<String> {
    let m = vec![
        "B".to_string(),
        "KB".to_string(),
        "MB".to_string(),
        "GB".to_string(),
        "TB".to_string(),
        "PB".to_string(),
        "EB".to_string(),
        "ZB".to_string(),
        "YB".to_string(),
    ];
    m
}

fn parse_size(s: &str, m: &HashMap<String, u128>) -> Result<u128> {
    let re = Regex::new(r"(?P<num>\d+)(?P<mul>[kKmMgGtTpP]?)[bB]?$");

    if re.is_err() {
        return Err(Error::new(InvalidBytesSize));
    }
    let caps = re.unwrap().captures(s).unwrap();

    let num = caps.name("num");
    let size: u128 = if num.is_some() {
        let n = num.unwrap().as_str().trim().parse::<u128>();
        if n.is_err() {
            return Err(Error::new(InvalidBytesSize));
        }
        n.unwrap()
    } else {
        return Err(Error::new(InvalidBytesSize));
    };

    let q = caps.name("mul");
    let mul: u128 = if q.is_some() {
        let t = m.get(q.unwrap().as_str());
        if t.is_some() {
            *t.unwrap()
        } else {
            return Err(Error::new(InvalidBytesSize));
        }
    } else {
        return Err(Error::new(InvalidBytesSize));
    };

    Ok(size * mul)
}

fn custom_size(mut size: f64, base: f64, m: &Vec<String>) -> String {
    let mut i = 0;
    while size >= base && i < m.len() - 1 {
        size /= base;
        i += 1;
    }

    format!("{}{}", size, m[i].as_str())
}
