// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod pod_resources;

use anyhow::{anyhow, Result};
use cdi::cache::{new_cache, with_auto_refresh, Cache, CdiOption};
use cdi::spec_dirs::with_spec_dirs;
use cdi::specs::config::DeviceNode;
use container_device_interface as cdi;
use serde::Deserialize;

use slog::info;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// DEFAULT_DYNAMIC_CDI_SPEC_PATH is the default directory for dynamic CDI Specs,
/// which can be overridden by specifying a different path when creating the cache.
const DEFAULT_DYNAMIC_CDI_SPEC_PATH: &str = "/var/run/cdi";
/// DEFAULT_STATIC_CDI_SPEC_PATH is the default directory for static CDI Specs,
/// which can be overridden by specifying a different path when creating the cache.
const DEFAULT_STATIC_CDI_SPEC_PATH: &str = "/etc/cdi";

/// Default CDI spec directories consulted when resolving devices for cold plug.
pub const DEFAULT_CDI_SPEC_DIRS: [&str; 2] =
    [DEFAULT_DYNAMIC_CDI_SPEC_PATH, DEFAULT_STATIC_CDI_SPEC_PATH];

/// Device source token for the legacy device-plugin API (container.devices);
/// must match kata-types' `pod_resource_device_sources` tokens.
pub const POD_RESOURCE_DEVICE_SOURCE_DEVICE_PLUGIN: &str = "device-plugin";

/// Device source token for Dynamic Resource Allocation (KEP-3695); must match
/// kata-types' `pod_resource_device_sources` tokens.
pub const POD_RESOURCE_DEVICE_SOURCE_DRA: &str = "dra";

/// Build a CDI cache with auto-refresh disabled: callers refresh explicitly,
/// and a racing background refresh makes device lookups flaky.
fn new_cdi_cache(spec_dirs: &[&str]) -> Arc<Mutex<Cache>> {
    let options: Vec<CdiOption> = vec![with_auto_refresh(false), with_spec_dirs(spec_dirs)];
    new_cache(options)
}

/// Resolve CDI device names to their host device-node paths; names that do
/// not resolve in the cache are omitted from the map. A cache build/refresh
/// failure is a hard error so the fail-closed overlap guard can propagate it.
pub fn cdi_device_node_host_paths(
    spec_dirs: &[&str],
    names: &[String],
) -> Result<HashMap<String, Vec<String>>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    if names.is_empty() {
        return Ok(out);
    }

    let cache = new_cdi_cache(spec_dirs);
    let mut cache = cache.lock().unwrap();
    cache
        .refresh()
        .map_err(|e| anyhow!("Refreshing cache failed: {:?}", e))?;

    for name in names {
        if out.contains_key(name) {
            continue;
        }
        if let Some(device) = cache.get_device(name) {
            let mut paths = Vec::new();
            if let Some(devnodes) = device.edits().container_edits.device_nodes {
                for node in devnodes.iter() {
                    if let Some(p) = device_node_host_path(node) {
                        if !p.is_empty() {
                            paths.push(p);
                        }
                    }
                }
            }
            out.insert(name.clone(), paths);
        }
    }

    Ok(out)
}

/// Filter `devs` to the names that resolve to at least one device node in the
/// CDI cache (order-preserving); only node-bearing devices are cold-plugged, so
/// a device with only env/mount edits must not count (it would fail-close an
/// unlisted source that injects nothing). Mirrors Go's `cdiResolvableDevices`:
/// a cache failure means "none resolvable", never an error.
pub fn resolvable_cdi_devices(spec_dirs: &[&str], devs: &[String]) -> Vec<String> {
    if devs.is_empty() {
        return Vec::new();
    }
    let map = match cdi_device_node_host_paths(spec_dirs, devs) {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };
    let mut seen = std::collections::HashSet::new();
    devs.iter()
        .filter(|d| map.get(*d).is_some_and(|v| !v.is_empty()) && seen.insert((*d).clone()))
        .cloned()
        .collect()
}

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
    let cache: Arc<Mutex<Cache>> = new_cdi_cache(&DEFAULT_CDI_SPEC_DIRS);

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
