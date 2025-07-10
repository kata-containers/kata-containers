// Copyright (c) 2024 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// Description: Boot UVM for testing container storages/volumes.

use anyhow::{anyhow, Context, Result};
use hypervisor::Hypervisor;
use kata_types::config::{hypervisor::HYPERVISOR_NAME_CH, hypervisor::HYPERVISOR_NAME_QEMU};
use slog::info;
use std::sync::Arc;

mod vm_ops;
mod vm_utils;

#[derive(Clone)]
pub struct TestVm {
    pub hypervisor_name: String,
    pub hypervisor_instance: Arc<dyn Hypervisor>,
    pub socket_addr: String,
    pub hybrid_vsock: bool,
}

// Helper method to boot a test pod VM
pub fn setup_test_vm(hypervisor_name: &str) -> Result<TestVm> {
    info!(
        sl!(),
        "booting a pod vm using hypervisor:{:?}", hypervisor_name
    );

    // only supports qemu & cloud hypervisor for now
    if !hypervisor_name.contains(HYPERVISOR_NAME_QEMU)
        && !hypervisor_name.contains(HYPERVISOR_NAME_CH)
    {
        return Err(anyhow!("Unsupported hypervisor:{}", hypervisor_name));
    }

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(vm_ops::boot_test_vm(hypervisor_name))
        .context("booting the test vm")
}

// Helper method to stop a test pod VM
pub fn remove_test_vm(instance: TestVm) -> Result<()> {
    info!(sl!(), "Stopping booted pod vm");

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(vm_ops::stop_test_vm(instance.hypervisor_instance))
        .context("stopping the test vm")
}
