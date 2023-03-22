// Copyright (c) 2022-2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::{DeviceConfig, FsConfig, VmConfig};
use anyhow::{anyhow, Result};
use api_client::simple_api_full_command_and_response;

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

#[allow(dead_code)]
pub async fn cloud_hypervisor_vm_device_add(mut socket: UnixStream) -> Result<Option<String>> {
    let device_config = DeviceConfig::default();

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
