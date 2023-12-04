// Copyright (c) 2022-2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::{DeviceConfig, DiskConfig, FsConfig, NetConfig, VmConfig, VsockConfig};
use anyhow::{anyhow, Result};
use api_client::simple_api_full_command_and_response;

use serde::{Deserialize, Serialize};
use std::os::unix::net::UnixStream;
use tokio::task;

pub async fn cloud_hypervisor_vmm_ping(mut socket: UnixStream) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(&mut socket, "GET", "vmm.ping", None)
            .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vmm_shutdown(mut socket: UnixStream) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response =
            simple_api_full_command_and_response(&mut socket, "PUT", "vmm.shutdown", None)
                .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vm_create(
    mut socket: UnixStream,
    cfg: VmConfig,
) -> Result<Option<String>> {
    let serialised = serde_json::to_string_pretty(&cfg)?;

    task::spawn_blocking(move || -> Result<Option<String>> {
        let data = Some(serialised.as_str());

        let response = simple_api_full_command_and_response(&mut socket, "PUT", "vm.create", data)
            .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vm_start(mut socket: UnixStream) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(&mut socket, "PUT", "vm.boot", None)
            .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

#[allow(dead_code)]
pub async fn cloud_hypervisor_vm_stop(mut socket: UnixStream) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response =
            simple_api_full_command_and_response(&mut socket, "PUT", "vm.shutdown", None)
                .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
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
    mut socket: UnixStream,
    blk_config: DiskConfig,
) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(
            &mut socket,
            "PUT",
            "vm.add-disk",
            Some(&serde_json::to_string(&blk_config)?),
        )
        .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vm_netdev_add(
    mut socket: UnixStream,
    net_config: NetConfig,
) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(
            &mut socket,
            "PUT",
            "vm.add-net",
            Some(&serde_json::to_string(&net_config)?),
        )
        .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vm_device_add(
    mut socket: UnixStream,
    device_config: DeviceConfig,
) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(
            &mut socket,
            "PUT",
            "vm.add-device",
            Some(&serde_json::to_string(&device_config)?),
        )
        .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

#[allow(dead_code)]
pub async fn cloud_hypervisor_vm_device_remove(
    mut socket: UnixStream,
    device_data: VmRemoveDeviceData,
) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(
            &mut socket,
            "PUT",
            "vm.remove-device",
            Some(&serde_json::to_string(&device_data)?),
        )
        .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vm_fs_add(
    mut socket: UnixStream,
    fs_config: FsConfig,
) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(
            &mut socket,
            "PUT",
            "vm.add-fs",
            Some(&serde_json::to_string(&fs_config)?),
        )
        .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vm_vsock_add(
    mut socket: UnixStream,
    vsock_config: VsockConfig,
) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(
            &mut socket,
            "PUT",
            "vm.add-vsock",
            Some(&serde_json::to_string(&vsock_config)?),
        )
        .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}
