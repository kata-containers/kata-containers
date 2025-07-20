// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod cgroup_persist;
mod resource;
pub use resource::CgroupsResource;
mod resource_inner;
mod utils;

use anyhow::{anyhow, Result};
use cgroups_rs::manager::is_systemd_cgroup;
use hypervisor::HYPERVISOR_DRAGONBALL;
use kata_sys_util::spec::load_oci_spec;
use kata_types::config::TomlConfig;

use crate::cgroups::cgroup_persist::CgroupState;

const SANDBOXED_CGROUP_PATH: &str = "kata_sandboxed_pod";

pub struct CgroupArgs {
    pub sid: String,
    pub config: TomlConfig,
}

pub struct CgroupConfig {
    pub path: String,
    pub overhead_path: String,
    pub sandbox_cgroup_only: bool,
}

impl CgroupConfig {
    fn new(sid: &str, toml_config: &TomlConfig) -> Result<Self> {
        let path = if let Ok(spec) = load_oci_spec() {
            spec.linux()
                .clone()
                .and_then(|linux| linux.cgroups_path().clone())
                .map(|path| {
                    // The trim of '/' is important, because cgroup_path is a relative path.
                    path.display()
                        .to_string()
                        .trim_start_matches('/')
                        .to_string()
                })
                .unwrap_or_default()
        } else {
            format!("{}/{}", SANDBOXED_CGROUP_PATH, sid)
        };

        let overhead_path = utils::gen_overhead_path(is_systemd_cgroup(&path), sid);

        // Dragonball and runtime are the same process, so that the
        // sandbox_cgroup_only is overwriten to true.
        let sandbox_cgroup_only = if toml_config.runtime.hypervisor_name == HYPERVISOR_DRAGONBALL {
            true
        } else {
            toml_config.runtime.sandbox_cgroup_only
        };

        Ok(Self {
            path,
            overhead_path,
            sandbox_cgroup_only,
        })
    }

    fn restore(state: &CgroupState) -> Result<Self> {
        let path = state
            .path
            .as_ref()
            .ok_or_else(|| anyhow!("cgroup path is missing in state"))?;
        let overhead_path = state
            .overhead_path
            .as_ref()
            .ok_or_else(|| anyhow!("overhead path is missing in state"))?;

        Ok(Self {
            path: path.clone(),
            overhead_path: overhead_path.clone(),
            sandbox_cgroup_only: state.sandbox_cgroup_only,
        })
    }
}
