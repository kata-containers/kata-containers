// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TomlConfig {
    pub hypervisor: HashMap<String, Hypervisor>,
    pub runtime: Runtime,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Hypervisor {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub kernel: String,
    #[serde(default)]
    pub initrd: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub default_vcpus: u32,
    #[serde(default)]
    pub default_maxvcpus: u32,
    #[serde(default)]
    pub default_memory: u32,
    #[serde(default)]
    pub kernel_params: String,
    #[serde(default)]
    pub block_device_driver: String,
    #[serde(default)]
    pub block_device_cache_direct: bool,
    #[serde(default)]
    pub shared_fs: Option<String>,
    #[serde(default)]
    pub virtio_fs_daemon: String,
    #[serde(default)]
    pub virtio_fs_cache: String,
    #[serde(default)]
    pub virtio_fs_cache_size: u32,
    #[serde(default)]
    pub virtio_fs_extra_args: Vec<String>,
    #[serde(default, rename = "enable_debug")]
    pub debug: bool,
    #[serde(default, rename = "enable_hugepages")]
    pub huge_pages: bool,
    #[serde(default, rename = "enable_mem_prealloc")]
    pub mem_prealloc: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Runtime {
    #[serde(default, rename = "enable_debug")]
    pub debug: bool,
    #[serde(default)]
    pub log_level: String,
    #[serde(default)]
    pub log_format: String,
    #[serde(default)]
    pub disable_new_netns: bool,
    #[serde(default)]
    pub sandbox_cgroup_only: bool,
    #[serde(default)]
    pub internetworking_model: String,
    #[serde(default)]
    pub enable_tracing: bool,
    // If specified, sandbox_bind_mounts identifieds host paths to be mounted into the sandboxes
    // shared path. This is only valid if filesystem sharing is utilized. The provided path(s) will
    // be bindmounted into the shared fs directory. If defaults are utilized, these mounts should
    // be available in the guest at
    // `/run/kata-containers/shared/containers/passthrough/sandbox-mounts` These will not be
    // exposed to the container workloads, and are only provided for potential guest services.
    #[serde(default)]
    pub sandbox_bind_mounts: Vec<String>,
}
