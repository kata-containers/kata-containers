// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::errors::*;
// use crate::configs::{FreezerState, Config};
use protocols::agent::CgroupStats;
use protocols::oci::LinuxResources;
use std::collections::HashMap;

pub mod fs;
pub mod systemd;

pub type FreezerState = &'static str;

pub trait Manager {
    fn apply(&self, _pid: i32) -> Result<()> {
        Err(ErrorKind::ErrorCode("not supported!".to_string()).into())
    }

    fn get_pids(&self) -> Result<Vec<i32>> {
        Err(ErrorKind::ErrorCode("not supported!".to_string()).into())
    }

    fn get_all_pids(&self) -> Result<Vec<i32>> {
        Err(ErrorKind::ErrorCode("not supported!".to_string()).into())
    }

    fn get_stats(&self) -> Result<CgroupStats> {
        Err(ErrorKind::ErrorCode("not supported!".to_string()).into())
    }

    fn freeze(&self, _state: FreezerState) -> Result<()> {
        Err(ErrorKind::ErrorCode("not supported!".to_string()).into())
    }

    fn destroy(&mut self) -> Result<()> {
        Err(ErrorKind::ErrorCode("not supported!".to_string()).into())
    }

    fn get_paths(&self) -> Result<HashMap<String, String>> {
        Err(ErrorKind::ErrorCode("not supported!".to_string()).into())
    }

    fn set(&self, _container: &LinuxResources, _update: bool) -> Result<()> {
        Err(ErrorKind::ErrorCode("not supported!".to_string()).into())
    }
}
