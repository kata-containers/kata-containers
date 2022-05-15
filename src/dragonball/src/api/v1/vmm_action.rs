// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::fs::File;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use log::{debug, error, warn};
use vmm_sys_util::eventfd::EventFd;

use crate::error::Result;
use crate::event_manager::EventManager;
use crate::vm::{KernelConfigInfo, VmConfigInfo};
use crate::vmm::Vmm;

use super::*;

/// Wrapper for all errors associated with VMM actions.
#[derive(Debug, thiserror::Error)]
pub enum VmmActionError {
    /// The action `ConfigureBootSource` failed either because of bad user input or an internal
    /// error.
    #[error("failed to configure boot source for VM: {0}")]
    BootSource(#[source] BootSourceConfigError),
}

/// This enum represents the public interface of the VMM. Each action contains various
/// bits of information (ids, paths, etc.).
#[derive(Clone, Debug, PartialEq)]
pub enum VmmAction {
    /// Configure the boot source of the microVM using as input the `ConfigureBootSource`. This
    /// action can only be called before the microVM has booted.
    ConfigureBootSource(BootSourceConfig),
}

/// The enum represents the response sent by the VMM in case of success. The response is either
/// empty, when no data needs to be sent, or an internal VMM structure.
#[derive(Debug)]
pub enum VmmData {
    /// No data is sent on the channel.
    Empty,
}

/// Request data type used to communicate between the API and the VMM.
pub type VmmRequest = Box<VmmAction>;

/// Data type used to communicate between the API and the VMM.
pub type VmmRequestResult = std::result::Result<VmmData, VmmActionError>;

/// Response data type used to communicate between the API and the VMM.
pub type VmmResponse = Box<VmmRequestResult>;

/// VMM Service to handle requests from the API server.
///
/// There are two levels of API servers as below:
/// API client <--> VMM API Server <--> VMM Core
pub struct VmmService {
    from_api: Receiver<VmmRequest>,
    to_api: Sender<VmmResponse>,
    machine_config: VmConfigInfo,
}

impl VmmService {
    /// Create a new VMM API server instance.
    pub fn new(from_api: Receiver<VmmRequest>, to_api: Sender<VmmResponse>) -> Self {
        VmmService {
            from_api,
            to_api,
            machine_config: VmConfigInfo::default(),
        }
    }

    /// Handle requests from the HTTP API Server and send back replies.
    pub fn run_vmm_action(&mut self, vmm: &mut Vmm, _event_mgr: &mut EventManager) -> Result<()> {
        let request = match self.from_api.try_recv() {
            Ok(t) => *t,
            Err(TryRecvError::Empty) => {
                warn!("Got a spurious notification from api thread");
                return Ok(());
            }
            Err(TryRecvError::Disconnected) => {
                panic!("The channel's sending half was disconnected. Cannot receive data.");
            }
        };
        debug!("receive vmm action: {:?}", request);

        let response = match request {
            VmmAction::ConfigureBootSource(boot_source_body) => {
                self.configure_boot_source(vmm, boot_source_body)
            }
        };

        debug!("send vmm response: {:?}", response);
        self.send_response(response)
    }

    fn send_response(&self, result: VmmRequestResult) -> Result<()> {
        self.to_api
            .send(Box::new(result))
            .map_err(|_| ())
            .expect("vmm: one-shot API result channel has been closed");

        Ok(())
    }

    fn configure_boot_source(
        &self,
        vmm: &mut Vmm,
        boot_source_config: BootSourceConfig,
    ) -> VmmRequestResult {
        use super::BootSourceConfigError::{
            InvalidInitrdPath, InvalidKernelCommandLine, InvalidKernelPath, InvalidVMID,
            UpdateNotAllowedPostBoot,
        };
        use super::VmmActionError::BootSource;

        let vm = vmm.get_vm_by_id_mut("").ok_or(BootSource(InvalidVMID))?;
        if vm.is_vm_initialized() {
            return Err(BootSource(UpdateNotAllowedPostBoot));
        }

        let kernel_file = File::open(&boot_source_config.kernel_path)
            .map_err(|e| BootSource(InvalidKernelPath(e)))?;

        let initrd_file = match boot_source_config.initrd_path {
            None => None,
            Some(ref path) => Some(File::open(path).map_err(|e| BootSource(InvalidInitrdPath(e)))?),
        };

        let mut cmdline = linux_loader::cmdline::Cmdline::new(dbs_boot::layout::CMDLINE_MAX_SIZE);
        let boot_args = boot_source_config
            .boot_args
            .clone()
            .unwrap_or_else(|| String::from(DEFAULT_KERNEL_CMDLINE));
        cmdline
            .insert_str(boot_args)
            .map_err(|e| BootSource(InvalidKernelCommandLine(e)))?;

        let kernel_config = KernelConfigInfo::new(kernel_file, initrd_file, cmdline);
        vm.set_kernel_config(kernel_config);

        Ok(VmmData::Empty)
    }
}
