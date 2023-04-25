// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// SPDX-License-Identifier: Apache-2.0

use serde_derive::{Deserialize, Serialize};

/// The microvm state.
///
/// When Dragonball starts, the instance state is Uninitialized. Once start_microvm method is
/// called, the state goes from Uninitialized to Starting. The state is changed to Running until
/// the start_microvm method ends. Halting and Halted are currently unsupported.
#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum InstanceState {
    /// Microvm is not initialized.
    Uninitialized,
    /// Microvm is starting.
    Starting,
    /// Microvm is running.
    Running,
    /// Microvm is Paused.
    Paused,
    /// Microvm received a halt instruction.
    Halting,
    /// Microvm is halted.
    Halted,
    /// Microvm exit instead of process exit.
    Exited(i32),
}

/// The state of async actions
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum AsyncState {
    /// Uninitialized
    Uninitialized,
    /// Success
    Success,
    /// Failure
    Failure,
}

/// The strongly typed that contains general information about the microVM.
#[derive(Debug, Deserialize, Serialize)]
pub struct InstanceInfo {
    /// The ID of the microVM.
    pub id: String,
    /// The state of the microVM.
    pub state: InstanceState,
    /// The version of the VMM that runs the microVM.
    pub vmm_version: String,
    /// The pid of the current VMM process.
    pub pid: u32,
    /// The tid of the current VMM master thread.
    pub master_tid: u32,
    /// The state of async actions.
    pub async_state: AsyncState,
    /// List of tids of vcpu threads (vcpu index, tid)
    pub tids: Vec<(u8, u32)>,
    /// Last instance downtime
    pub last_instance_downtime: u64,
}

impl InstanceInfo {
    /// create instance info object with given id, version, and platform type
    pub fn new(id: String, vmm_version: String) -> Self {
        InstanceInfo {
            id,
            state: InstanceState::Uninitialized,
            vmm_version,
            pid: std::process::id(),
            master_tid: 0,
            async_state: AsyncState::Uninitialized,
            tids: Vec::new(),
            last_instance_downtime: 0,
        }
    }
}

impl Default for InstanceInfo {
    fn default() -> Self {
        InstanceInfo {
            id: String::from(""),
            state: InstanceState::Uninitialized,
            vmm_version: env!("CARGO_PKG_VERSION").to_string(),
            pid: std::process::id(),
            master_tid: 0,
            async_state: AsyncState::Uninitialized,
            tids: Vec::new(),
            last_instance_downtime: 0,
        }
    }
}
