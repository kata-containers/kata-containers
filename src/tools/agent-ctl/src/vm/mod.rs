// Copyright (c) 2024 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// Description: Boot UVM for testing container storages/volumes.

use anyhow::{anyhow, Context, Result};
use hypervisor::Hypervisor;
use kata_types::config::{hypervisor::HYPERVISOR_NAME_CH, hypervisor::HYPERVISOR_NAME_QEMU};
use share_fs_utils::SharedFs;
use slog::info;
use std::sync::Arc;

mod share_fs_utils;
mod vm_ops;
pub mod vm_utils;

lazy_static! {
    pub(crate) static ref SUPPORTED_VMMS: Vec<&'static str> =
        vec![HYPERVISOR_NAME_CH, HYPERVISOR_NAME_QEMU];
}

#[derive(Clone)]
pub struct TestVm {
    pub hypervisor_name: String,
    pub hypervisor_instance: Arc<dyn Hypervisor>,
    pub socket_addr: String,
    pub hybrid_vsock: bool,
    pub share_fs: SharedFs,
}

// Helper method to boot a test pod VM
pub fn setup_vm(hypervisor_name: &str) -> Result<TestVm> {
    info!(
        sl!(),
        "booting a pod vm using hypervisor:{:?}", hypervisor_name
    );

    if !SUPPORTED_VMMS.contains(&hypervisor_name) {
        return Err(anyhow!("Unsupported hypervisor:{}", hypervisor_name));
    }

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(vm_ops::boot_vm(hypervisor_name))
        .context("booting the test vm")
}

// Helper method to stop a test pod VM
pub fn remove_vm(instance: TestVm) -> Result<()> {
    info!(sl!(), "Stopping booted pod vm");

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(vm_ops::stop_vm(instance))
        .context("stopping the test vm")
}
