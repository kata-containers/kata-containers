// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

// use crate::configs::{FreezerState, Config};
use anyhow::{anyhow, Result};
use oci::LinuxResources;
use protocols::agent::CgroupStats;
use std::collections::HashMap;

pub mod fs;
pub mod systemd;

pub type FreezerState = &'static str;

pub trait Manager {
    fn apply(&self, _pid: i32) -> Result<()> {
        Err(anyhow!("not supported!".to_string()))
    }

    fn get_pids(&self) -> Result<Vec<i32>> {
        Err(anyhow!("not supported!"))
    }

    fn get_all_pids(&self) -> Result<Vec<i32>> {
        Err(anyhow!("not supported!"))
    }

    fn get_stats(&self) -> Result<CgroupStats> {
        Err(anyhow!("not supported!"))
    }

    fn freeze(&self, _state: FreezerState) -> Result<()> {
        Err(anyhow!("not supported!"))
    }

    fn destroy(&mut self) -> Result<()> {
        Err(anyhow!("not supported!"))
    }

    fn get_paths(&self) -> Result<HashMap<String, String>> {
        Err(anyhow!("not supported!"))
    }

    fn set(&self, _container: &LinuxResources, _update: bool) -> Result<()> {
        Err(anyhow!("not supported!"))
    }
}
