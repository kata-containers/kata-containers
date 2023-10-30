// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::fmt::Formatter;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex, RwLock};

use dbs_utils::epoll_manager::EpollManager;
use log::{error, info, warn};
use seccompiler::BpfProgram;
use tracing::instrument;
use vmm_sys_util::eventfd::EventFd;

use crate::api::v1::{InstanceInfo, VmmService};
use crate::error::{EpollError, Result};
use crate::event_manager::{EventContext, EventManager};
use crate::vm::Vm;
use crate::{EXIT_CODE_GENERIC_ERROR, EXIT_CODE_OK};

/// Global coordinator to manage API servers, virtual machines, upgrade etc.
///
/// Originally firecracker assumes an VMM only manages an VM, and doesn't distinguish VMM and VM.
/// Thus caused a mixed and confusion design. Now we have explicit build the object model as:
///  |---Vmm API Server--<-1:1-> HTTP API Server
///  |        |----------<-1:1-> Shimv2/CRI API Server
///  |
/// Vmm <-1:N-> Vm <-1:1-> Address Space Manager <-1:N-> GuestMemory
///  ^           ^---1:1-> Device Manager <-1:N-> Device
///  |           ^---1:1-> Resource Manager
///  |           ^---1:N-> Vcpu
///  |---<-1:N-> Event Manager
pub struct Vmm {
    pub(crate) event_ctx: EventContext,
    epoll_manager: EpollManager,

    // Will change to a HashMap when enabling 1 VMM with multiple VMs.
    vm: Vm,

    vcpu_seccomp_filter: BpfProgram,
    vmm_seccomp_filter: BpfProgram,
}

impl Vmm {
    /// Create a Virtual Machine Monitor instance.
    pub fn new(
        api_shared_info: Arc<RwLock<InstanceInfo>>,
        api_event_fd: EventFd,
        vmm_seccomp_filter: BpfProgram,
        vcpu_seccomp_filter: BpfProgram,
        kvm_fd: Option<RawFd>,
    ) -> Result<Self> {
        let epoll_manager = EpollManager::default();
        Self::new_with_epoll_manager(
            api_shared_info,
            api_event_fd,
            epoll_manager,
            vmm_seccomp_filter,
            vcpu_seccomp_filter,
            kvm_fd,
        )
    }

    /// Create a Virtual Machine Monitor instance with a epoll_manager.
    pub fn new_with_epoll_manager(
        api_shared_info: Arc<RwLock<InstanceInfo>>,
        api_event_fd: EventFd,
        epoll_manager: EpollManager,
        vmm_seccomp_filter: BpfProgram,
        vcpu_seccomp_filter: BpfProgram,
        kvm_fd: Option<RawFd>,
    ) -> Result<Self> {
        let vm = Vm::new(kvm_fd, api_shared_info, epoll_manager.clone())?;
        let event_ctx = EventContext::new(api_event_fd)?;

        Ok(Vmm {
            event_ctx,
            epoll_manager,
            vm,
            vcpu_seccomp_filter,
            vmm_seccomp_filter,
        })
    }

    /// Get a reference to a virtual machine managed by the VMM.
    pub fn get_vm(&self) -> Option<&Vm> {
        Some(&self.vm)
    }

    /// Get a mutable reference to a virtual machine managed by the VMM.
    pub fn get_vm_mut(&mut self) -> Option<&mut Vm> {
        Some(&mut self.vm)
    }

    /// Get the seccomp rules for vCPU threads.
    pub fn vcpu_seccomp_filter(&self) -> BpfProgram {
        self.vcpu_seccomp_filter.clone()
    }

    /// Get the seccomp rules for VMM threads.
    pub fn vmm_seccomp_filter(&self) -> BpfProgram {
        self.vmm_seccomp_filter.clone()
    }

    /// Run the event loop to service API requests.
    ///
    /// # Arguments
    ///
    /// * `vmm` - An Arc reference to the global Vmm instance.
    /// * `service` - VMM Service provider.
    pub fn run_vmm_event_loop(vmm: Arc<Mutex<Vmm>>, mut service: VmmService) -> i32 {
        let epoll_mgr = vmm.lock().unwrap().epoll_manager.clone();
        let mut event_mgr =
            EventManager::new(&vmm, epoll_mgr).expect("Cannot create epoll manager");

        'poll: loop {
            match event_mgr.handle_events(-1) {
                Ok(_) => {
                    // Check whether there are pending vmm events.
                    if event_mgr.fetch_vmm_event_count() == 0 {
                        continue;
                    }

                    let mut v = vmm.lock().unwrap();
                    if v.event_ctx.api_event_triggered {
                        // The run_vmm_action() needs to access event_mgr, so it could
                        // not be handled in EpollHandler::handle_events(). It has been
                        // delayed to the main loop.
                        v.event_ctx.api_event_triggered = false;
                        service
                            .run_vmm_action(&mut v, &mut event_mgr)
                            .unwrap_or_else(|_| {
                                warn!("got spurious notification from api thread");
                            });
                    }
                    if v.event_ctx.exit_evt_triggered {
                        info!("Gracefully terminated VMM control loop");
                        let ret = v.stop(EXIT_CODE_OK as i32);
                        let tracer = service.tracer();
                        let mut tracer_guard = tracer.lock().unwrap();
                        tracer_guard.end_tracing().expect("End tracing err");
                        return ret;
                    }
                }
                Err(e) => {
                    error!("Abruptly exited VMM control loop: {:?}", e);
                    if let EpollError::EpollMgr(dbs_utils::epoll_manager::Error::Epoll(e)) = e {
                        if e.errno() == libc::EAGAIN || e.errno() == libc::EINTR {
                            continue 'poll;
                        }
                    }
                    return vmm.lock().unwrap().stop(EXIT_CODE_GENERIC_ERROR as i32);
                }
            }
        }
    }

    /// Waits for all vCPUs to exit and terminates the Dragonball process.
    #[instrument(name = "stop vmm")]
    fn stop(&mut self, exit_code: i32) -> i32 {
        info!("Vmm is stopping.");
        if let Some(vm) = self.get_vm_mut() {
            if vm.is_vm_initialized() {
                if let Err(e) = vm.remove_devices() {
                    warn!("failed to remove devices: {:?}", e);
                }

                #[cfg(feature = "dbs-upcall")]
                if let Err(e) = vm.remove_upcall() {
                    warn!("failed to remove upcall: {:?}", e);
                }

                if let Err(e) = vm.reset_console() {
                    warn!("Cannot set canonical mode for the terminal. {:?}", e);
                }

                // Now, we use exit_code instead of invoking _exit to
                // terminate process, so all of vcpu threads should be stopped
                // prior to vmm event loop.
                match vm.vcpu_manager() {
                    Ok(mut mgr) => {
                        if let Err(e) = mgr.exit_all_vcpus() {
                            warn!("Failed to exit vcpu thread. {:?}", e);
                        }
                        #[cfg(feature = "dbs-upcall")]
                        mgr.set_upcall_channel(None);
                    }
                    Err(e) => warn!("Failed to get vcpu manager {:?}", e),
                }

                // save exit state to VM, instead of exit process.
                vm.vm_exit(exit_code);
            }
        }

        exit_code
    }
}

impl std::fmt::Debug for Vmm {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vmm")
            .field("event_ctx", &self.event_ctx)
            .field("vm", &self.vm.shared_info())
            .field("vcpu_seccomp_filter", &self.vcpu_seccomp_filter)
            .field("vmm_seccomp_filter", &self.vmm_seccomp_filter)
            .finish()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use test_utils::skip_if_not_root;

    use super::*;

    pub fn create_vmm_instance(epoll_manager: EpollManager) -> Vmm {
        let info = Arc::new(RwLock::new(InstanceInfo::default()));
        let event_fd = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let seccomp_filter: BpfProgram = Vec::new();

        Vmm::new_with_epoll_manager(
            info,
            event_fd,
            epoll_manager,
            seccomp_filter.clone(),
            seccomp_filter,
            None,
        )
        .unwrap()
    }

    #[test]
    fn test_create_vmm_instance() {
        skip_if_not_root!();

        create_vmm_instance(EpollManager::default());
    }
}
