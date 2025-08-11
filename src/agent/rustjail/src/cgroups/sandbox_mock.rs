// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use oci_spec::runtime::{LinuxResources, Spec};

#[derive(Debug, Default)]
pub struct SandboxCgroupManager {}

impl SandboxCgroupManager {
    pub fn try_init(&self, _path: &str, _spec: &Spec) -> Result<()> {
        Ok(())
    }

    pub fn enable(&self) -> bool {
        false
    }

    pub fn enable_devcg(&self) -> bool {
        false
    }

    pub fn is_allowed_all_devices(&self) -> bool {
        false
    }

    pub fn allow_all_devices(&self) -> Result<()> {
        Ok(())
    }

    pub fn set(&self, _resources: &LinuxResources) -> Result<()> {
        Ok(())
    }
}
