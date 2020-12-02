// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use protobuf::{CachedSize, SingularPtrField, UnknownFields};

use crate::cgroups::Manager as CgroupManager;
use crate::protocols::agent::{BlkioStats, CgroupStats, CpuStats, MemoryStats, PidsStats};
use anyhow::Result;
use cgroups::freezer::FreezerState;
use libc::{self, pid_t};
use oci::LinuxResources;
use std::collections::HashMap;
use std::string::String;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Manager {
    pub paths: HashMap<String, String>,
    pub mounts: HashMap<String, String>,
    pub cpath: String,
}

impl CgroupManager for Manager {
    fn apply(&self, _: pid_t) -> Result<()> {
        Ok(())
    }

    fn set(&self, _: &LinuxResources, _: bool) -> Result<()> {
        Ok(())
    }

    fn get_stats(&self) -> Result<CgroupStats> {
        Ok(CgroupStats {
            cpu_stats: SingularPtrField::some(CpuStats::default()),
            memory_stats: SingularPtrField::some(MemoryStats::new()),
            pids_stats: SingularPtrField::some(PidsStats::new()),
            blkio_stats: SingularPtrField::some(BlkioStats::new()),
            hugetlb_stats: HashMap::new(),
            unknown_fields: UnknownFields::default(),
            cached_size: CachedSize::default(),
        })
    }

    fn freeze(&self, _: FreezerState) -> Result<()> {
        Ok(())
    }

    fn destroy(&mut self) -> Result<()> {
        Ok(())
    }

    fn get_pids(&self) -> Result<Vec<pid_t>> {
        Ok(Vec::new())
    }
}

impl Manager {
    pub fn new(cpath: &str) -> Result<Self> {
        Ok(Self {
            paths: HashMap::new(),
            mounts: HashMap::new(),
            cpath: cpath.to_string(),
        })
    }

    pub fn update_cpuset_path(&self, _: &str, _: &str) -> Result<()> {
        Ok(())
    }

    pub fn get_cg_path(&self, _: &str) -> Option<String> {
        Some("".to_string())
    }
}
