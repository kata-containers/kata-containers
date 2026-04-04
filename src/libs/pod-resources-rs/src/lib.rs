// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod pod_resources;

use anyhow::{Result, anyhow};
use cdi::specs::config::DeviceNode;
use cdi::cache::{CdiOption, new_cache, with_auto_refresh};
use cdi::spec_dirs::with_spec_dirs;
use container_device_interface as cdi;
use serde::Deserialize;

use slog::info;
use std::sync::Arc;

/// DEFAULT_DYNAMIC_CDI_SPEC_PATH is the default directory for dynamic CDI Specs,
/// which can be overridden by specifying a different path when creating the cache.
const DEFAULT_DYNAMIC_CDI_SPEC_PATH: &str = "/var/run/cdi";
/// DEFAULT_STATIC_CDI_SPEC_PATH is the default directory for static CDI Specs,
/// which can be overridden by specifying a different path when creating the cache.
const DEFAULT_STATIC_CDI_SPEC_PATH: &str = "/etc/cdi";

/// Typed projection of the upstream `DeviceNode` fields we need. The upstream
/// struct keeps `path` / `host_path` as `pub(crate)`, so we round-trip through
/// serde with a fixed field-name contract rather than relying on JSON key strings.
#[derive(Deserialize)]
struct DeviceNodePaths {
    path: Option<String>,
    #[serde(rename = "hostPath")]
    host_path: Option<String>,
}

/// Returns the host-side path for a CDI `DeviceNode`: `hostPath` if present,
/// otherwise `path`.
pub fn device_node_host_path(dn: &DeviceNode) -> Option<String> {
    let v = serde_json::to_value(dn).ok()?;
    let paths: DeviceNodePaths = serde_json::from_value(v).ok()?;
    paths.host_path.or(paths.path)
}

#[macro_export]
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

pub async fn handle_cdi_devices(devices: &[String]) -> Result<Vec<DeviceNode>> {
    if devices.is_empty() {
        info!(sl!(), "no pod CDI devices requested.");
        return Ok(vec![]);
    }
    // Explicitly set the cache options to disable auto-refresh and
    // to use the default spec dirs for dynamic and static CDI Specs
    let options: Vec<CdiOption> = vec![
        with_auto_refresh(false),
        with_spec_dirs(&[DEFAULT_DYNAMIC_CDI_SPEC_PATH, DEFAULT_STATIC_CDI_SPEC_PATH]),
    ];
    let cache: Arc<std::sync::Mutex<cdi::cache::Cache>> = new_cache(options);

    let target_devices = {
        let mut target_devices = vec![];
        // Lock cache within this scope, std::sync::Mutex has no Send
        // and await will not work with time::sleep
        let mut cache = cache.lock().unwrap();
        match cache.refresh() {
            Ok(_) => {}
            Err(e) => {
                return Err(anyhow!("Refreshing cache failed: {:?}", e));
            }
        }

        for dev in devices.iter() {
            info!(sl!(), "Requested CDI device with FQN: {}", dev);
            match cache.get_device(dev) {
                Some(device) => {
                    info!(sl!(), "Target CDI device: {}", device.get_qualified_name());
                    if let Some(devnodes) = device.edits().container_edits.device_nodes {
                        target_devices.extend(devnodes.iter().cloned());
                    }
                }
                None => {
                    return Err(anyhow!(
                        "Failed to get device node for CDI device: {} in cache",
                        dev
                    ));
                }
            }
        }

        target_devices
    };
    info!(sl!(), "target CDI devices to inject: {:?}", target_devices);

    Ok(target_devices)
}
