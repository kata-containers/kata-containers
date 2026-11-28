// Copyright (c) 2022-2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::{
    DeviceConfig, DiskConfig, FsConfig, NetConfig, VmConfig, VmInfo, VmResize, VsockConfig,
};
use anyhow::{anyhow, Context, Result};
use api_client::simple_api_full_command_with_fds_and_response;

use serde::{Deserialize, Serialize};
use std::os::{fd::RawFd, unix::net::UnixStream};
use tokio::sync::Mutex;
use tokio::task;

/// Type alias for the serialized API socket shared across callers.
///
/// All Cloud Hypervisor HTTP API calls share a single `UnixStream`. Because
/// the CH API uses HTTP/1.1 over a Unix domain socket without pipelining,
/// concurrent requests on the same stream corrupt the response framing.
/// Wrapping the socket in a `Mutex` ensures only one request-response cycle
/// is in flight at a time.
pub type ApiSocket = Mutex<Option<UnixStream>>;

/// Execute a CH API command while holding the API socket lock.
///
/// Acquires the mutex, clones the socket, and runs the blocking HTTP
/// request-response in `spawn_blocking`. The mutex guard is held until
/// the blocking task completes, ensuring no concurrent API calls.
async fn api_command(
    api_socket: &ApiSocket,
    method: &'static str,
    endpoint: &'static str,
    body: Option<String>,
    fds: Option<Vec<RawFd>>,
) -> Result<Option<String>> {
    let _guard = api_socket.lock().await;
    let mut socket = _guard
        .as_ref()
        .ok_or_else(|| anyhow!("missing api socket"))?
        .try_clone()
        .context("clone api socket")?;

    let result = task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_with_fds_and_response(
            &mut socket,
            method,
            endpoint,
            body.as_deref(),
            fds.as_deref().unwrap_or_default(),
        )
        .map_err(|e| anyhow!(e))?;
        Ok(response)
    })
    .await?;

    result
}

pub async fn cloud_hypervisor_vmm_ping(api_socket: &ApiSocket) -> Result<Option<String>> {
    api_command(api_socket, "GET", "vmm.ping", None, None).await
}

pub async fn cloud_hypervisor_vmm_shutdown(api_socket: &ApiSocket) -> Result<Option<String>> {
    api_command(api_socket, "PUT", "vmm.shutdown", None, None).await
}

pub async fn cloud_hypervisor_vm_create(
    api_socket: &ApiSocket,
    cfg: VmConfig,
) -> Result<Option<String>> {
    let body = serde_json::to_string_pretty(&cfg)?;
    api_command(api_socket, "PUT", "vm.create", Some(body), None).await
}

pub async fn cloud_hypervisor_vm_start(api_socket: &ApiSocket) -> Result<Option<String>> {
    api_command(api_socket, "PUT", "vm.boot", None, None).await
}

pub async fn cloud_hypervisor_vm_pause(api_socket: &ApiSocket) -> Result<Option<String>> {
    api_command(api_socket, "PUT", "vm.pause", None, None).await
}

pub async fn cloud_hypervisor_vm_resume(api_socket: &ApiSocket) -> Result<Option<String>> {
    api_command(api_socket, "PUT", "vm.resume", None, None).await
}

#[derive(Clone, Deserialize, Serialize, Default, Debug)]
pub struct VmSnapshotConfig {
    pub destination_url: String,
}

/// Memory restore mode supported by Cloud Hypervisor's `vm.restore` API.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MemoryRestoreMode {
    /// Copy the snapshot memory into guest memory (Cloud Hypervisor default).
    #[serde(rename = "Copy")]
    Copy,
    /// Populate guest memory on demand from the snapshot using userfaultfd.
    #[serde(rename = "OnDemand")]
    OnDemand,
    /// Map the snapshot memory copy-on-write instead of copying it.
    #[serde(rename = "CopyOnWrite")]
    CopyOnWrite,
}

impl Default for MemoryRestoreMode {
    fn default() -> Self {
        Self::Copy
    }
}

impl MemoryRestoreMode {
    pub fn parse(mode: &str) -> Result<Self> {
        let normalized = mode.trim().to_ascii_lowercase();

        match normalized.as_str() {
            "copy" => Ok(Self::Copy),
            "ondemand" => Ok(Self::OnDemand),
            "copyonwrite" => Ok(Self::CopyOnWrite),
            _ => Err(anyhow!(
                "invalid memory restore mode {mode:?}: valid values are \"copy\", \"ondemand\" and \"copyonwrite\""
            )),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, Default, Debug)]
pub struct RestoreConfig {
    pub source_url: String,
    /// Memory restore mode. When unset, Cloud Hypervisor's default (`Copy`) is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_restore_mode: Option<MemoryRestoreMode>,
}

pub async fn cloud_hypervisor_vm_snapshot(
    api_socket: &ApiSocket,
    cfg: VmSnapshotConfig,
) -> Result<Option<String>> {
    let body = serde_json::to_string(&cfg)?;
    api_command(api_socket, "PUT", "vm.snapshot", Some(body), None).await
}

pub async fn cloud_hypervisor_vm_restore(
    api_socket: &ApiSocket,
    cfg: RestoreConfig,
) -> Result<Option<String>> {
    let body = serde_json::to_string(&cfg)?;
    api_command(api_socket, "PUT", "vm.restore", Some(body), None).await
}

#[allow(dead_code)]
pub async fn cloud_hypervisor_vm_stop(api_socket: &ApiSocket) -> Result<Option<String>> {
    api_command(api_socket, "PUT", "vm.shutdown", None, None).await
}

#[derive(Deserialize, Debug)]
pub struct PciDeviceInfo {
    pub id: String,
    pub bdf: String,
}

#[derive(Clone, Deserialize, Serialize, Default, Debug)]
pub struct VmRemoveDeviceData {
    #[serde(default)]
    pub id: String,
}

pub async fn cloud_hypervisor_vm_blockdev_add(
    api_socket: &ApiSocket,
    blk_config: DiskConfig,
) -> Result<Option<String>> {
    let body = serde_json::to_string(&blk_config)?;
    api_command(api_socket, "PUT", "vm.add-disk", Some(body), None).await
}

pub async fn cloud_hypervisor_vm_netdev_add(
    api_socket: &ApiSocket,
    net_config: NetConfig,
) -> Result<Option<String>> {
    let body = serde_json::to_string(&net_config)?;
    api_command(api_socket, "PUT", "vm.add-net", Some(body), None).await
}

pub async fn cloud_hypervisor_vm_netdev_add_with_fds(
    api_socket: &ApiSocket,
    net_config: NetConfig,
    request_fds: Vec<RawFd>,
) -> Result<Option<String>> {
    let body = serde_json::to_string(&net_config)?;
    api_command(
        api_socket,
        "PUT",
        "vm.add-net",
        Some(body),
        Some(request_fds),
    )
    .await
}

pub async fn cloud_hypervisor_vm_device_add(
    api_socket: &ApiSocket,
    device_config: DeviceConfig,
) -> Result<Option<String>> {
    let body = serde_json::to_string(&device_config)?;
    api_command(api_socket, "PUT", "vm.add-device", Some(body), None).await
}

#[allow(dead_code)]
pub async fn cloud_hypervisor_vm_device_remove(
    api_socket: &ApiSocket,
    device_data: VmRemoveDeviceData,
) -> Result<Option<String>> {
    let body = serde_json::to_string(&device_data)?;
    api_command(api_socket, "PUT", "vm.remove-device", Some(body), None).await
}

pub async fn cloud_hypervisor_vm_fs_add(
    api_socket: &ApiSocket,
    fs_config: FsConfig,
) -> Result<Option<String>> {
    let body = serde_json::to_string(&fs_config)?;
    api_command(api_socket, "PUT", "vm.add-fs", Some(body), None).await
}

pub async fn cloud_hypervisor_vm_vsock_add(
    api_socket: &ApiSocket,
    vsock_config: VsockConfig,
) -> Result<Option<String>> {
    let body = serde_json::to_string(&vsock_config)?;
    api_command(api_socket, "PUT", "vm.add-vsock", Some(body), None).await
}

pub async fn cloud_hypervisor_vm_info(api_socket: &ApiSocket) -> Result<VmInfo> {
    let response = api_command(api_socket, "GET", "vm.info", None, None).await?;
    let vm_info = response.ok_or(anyhow!("failed to get vminfo"))?;
    serde_json::from_str(&vm_info).with_context(|| format!("failed to serde {vm_info}"))
}

pub async fn cloud_hypervisor_vm_resize(
    api_socket: &ApiSocket,
    vmresize: VmResize,
) -> Result<Option<String>> {
    let body = serde_json::to_string(&vmresize)?;
    api_command(api_socket, "PUT", "vm.resize", Some(body), None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_restore_mode_parse() {
        for (input, expected) in [
            ("Copy", MemoryRestoreMode::Copy),
            ("copy", MemoryRestoreMode::Copy),
            ("COPY", MemoryRestoreMode::Copy),
            ("OnDemand", MemoryRestoreMode::OnDemand),
            ("ondemand", MemoryRestoreMode::OnDemand),
            ("ONDEMAND", MemoryRestoreMode::OnDemand),
            ("CopyOnWrite", MemoryRestoreMode::CopyOnWrite),
            ("copyonwrite", MemoryRestoreMode::CopyOnWrite),
            ("COPYONWRITE", MemoryRestoreMode::CopyOnWrite),
        ] {
            assert_eq!(
                MemoryRestoreMode::parse(input).unwrap(),
                expected,
                "failed to parse {input:?}"
            );
        }

        for input in [
            "",
            "PreFault",
            "userfaultfd",
            "on_demand",
            "on-demand",
            "copy_on_write",
            "copy-on-write",
        ] {
            assert!(
                MemoryRestoreMode::parse(input).is_err(),
                "{:?} should be rejected",
                input
            );
        }
    }

    #[test]
    fn test_restore_config_serialization() {
        let cfg = RestoreConfig {
            source_url: "file:///run/vc/vm/template".to_string(),
            memory_restore_mode: Some(MemoryRestoreMode::OnDemand),
        };
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["source_url"], "file:///run/vc/vm/template");
        assert_eq!(json["memory_restore_mode"], "OnDemand");

        // When no mode is configured the field must be omitted so that Cloud
        // Hypervisor applies its own default ("Copy").
        let cfg = RestoreConfig {
            source_url: "file:///run/vc/vm/template".to_string(),
            memory_restore_mode: None,
        };
        let json = serde_json::to_value(&cfg).unwrap();
        assert!(json.get("memory_restore_mode").is_none());
    }
}
