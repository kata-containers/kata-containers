// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use protobuf::MessageField;

use crate::cgroups::Manager as CgroupManager;
use crate::protocols::agent::{BlkioStats, CgroupStats, CpuStats, MemoryStats, PidsStats};
use anyhow::Result;
use cgroups::freezer::FreezerState;
use libc::{self, pid_t};
use oci::{LinuxResources, Spec};
use oci_spec::runtime as oci;
use std::any::Any;
use std::collections::HashMap;
use std::string::String;
use std::sync::{Arc, RwLock};

use super::DevicesCgroupInfo;

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
            cpu_stats: MessageField::some(CpuStats::default()),
            memory_stats: MessageField::some(MemoryStats::new()),
            pids_stats: MessageField::some(PidsStats::new()),
            blkio_stats: MessageField::some(BlkioStats::new()),
            hugetlb_stats: HashMap::new(),
            ..Default::default()
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

    fn update_cpuset_path(&self, _: &str, _: &str) -> Result<()> {
        Ok(())
    }

    fn get_cgroup_path(&self, _: &str) -> Result<String> {
        Ok("".to_string())
    }

    fn as_any(&self) -> Result<&dyn Any> {
        Ok(self)
    }

    fn name(&self) -> &str {
        "mock"
    }
}

impl Manager {
    pub fn new(
        cpath: &str,
        _spec: &Spec,
        _devcg_info: Option<Arc<RwLock<DevicesCgroupInfo>>>,
    ) -> Result<Self> {
        Ok(Self {
            paths: HashMap::new(),
            mounts: HashMap::new(),
            cpath: cpath.to_string(),
        })
    }
}
