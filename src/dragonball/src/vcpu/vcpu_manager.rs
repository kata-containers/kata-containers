// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
//
// Copyright © 2019 Intel Corporation
// SPDX-License-Identifier: Apache-2.0

//! vCPU manager to enable bootstrap and CPU hotplug.
use std::io;
use std::os::unix::io::AsRawFd;
use std::sync::mpsc::{channel, Receiver, RecvError, RecvTimeoutError, Sender};
use std::sync::{Arc, Barrier, Mutex, RwLock};
use std::time::Duration;

#[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
use dbs_upcall::{DevMgrService, UpcallClient};
use dbs_utils::epoll_manager::{EpollManager, EventOps, EventSet, Events, MutEventSubscriber};
use dbs_utils::time::TimestampUs;
use kvm_ioctls::{Cap, VcpuFd, VmFd};
use log::{debug, error, info};
use seccompiler::{apply_filter, BpfProgram, Error as SecError};
use vm_memory::GuestAddress;
use vmm_sys_util::eventfd::EventFd;

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::api::v1::InstanceInfo;
use crate::kvm_context::KvmContext;
use crate::vcpu::vcpu_impl::{
    Vcpu, VcpuError, VcpuEvent, VcpuHandle, VcpuResponse, VcpuStateEvent,
};
use crate::vcpu::VcpuConfig;
use crate::vm::VmConfigInfo;
use crate::IoManagerCached;

/// the timeout for communication with vcpu threads
const CPU_RECV_TIMEOUT_MS: u64 = 1000;

/// vCPU manager error
#[derive(Debug, thiserror::Error)]
pub enum VcpuManagerError {
    /// IO errors in vCPU manager
    #[error("IO errors in vCPU manager {0}")]
    VcpuIO(#[source] io::Error),

    /// vCPU manager is not initialized
    #[error("vcpu manager is not initialized")]
    VcpuManagerNotInitialized,

    /// Expected vcpu exceed max count
    #[error("expected vcpu exceed max count")]
    ExpectedVcpuExceedMax,

    /// vCPU not found
    #[error("vcpu not found {0}")]
    VcpuNotFound(u8),

    /// Cannot recv vCPU thread tid
    #[error("cannot get vCPU thread id")]
    VcpuGettid,

    /// vCPU pause failed.
    #[error("failure while pausing vCPU thread")]
    VcpuPause,

    /// vCPU resume failed.
    #[error("failure while resuming vCPU thread")]
    VcpuResume,

    /// vCPU save failed.
    #[error("failure while save vCPU state")]
    VcpuSave,

    /// Vcpu is in unexpected state.
    #[error("Vcpu is in unexpected state")]
    UnexpectedVcpuResponse,

    /// Vcpu not create
    #[error("Vcpu is not create")]
    VcpuNotCreate,

    /// The number of max_vcpu reached kvm's limitation
    #[error("specified vcpu count {0} is greater than max allowed count {1} by kvm")]
    MaxVcpuLimitation(u8, usize),

    /// Revalidate vcpu IoManager cache failed.
    #[error("failure while revalidating vcpu IoManager cache")]
    VcpuRevalidateCache,

    /// Event fd is already set so there could be some problem in the VMM if we try to reset it.
    #[error("Event fd is already set for the vcpu")]
    EventAlreadyExist,

    /// Response channel error
    #[error("Response channel error: {0}")]
    VcpuResponseChannel(RecvError),

    /// Vcpu response timeout
    #[error("Vcpu response timeout: {0}")]
    VcpuResponseTimeout(RecvTimeoutError),

    /// Cannot build seccomp filters.
    #[error("failure while configuring seccomp filters: {0}")]
    SeccompFilters(#[source] seccompiler::Error),

    /// Cannot send event to vCPU.
    #[error("failure while sending message to vCPU thread: {0}")]
    VcpuEvent(#[source] VcpuError),

    /// vCPU Error
    #[error("vcpu internal error: {0}")]
    Vcpu(#[source] VcpuError),

    #[cfg(feature = "hotplug")]
    /// vCPU resize error
    #[error("resize vcpu error: {0}")]
    VcpuResize(#[source] VcpuResizeError),

    /// Kvm Ioctl Error
    #[error("failure in issuing KVM ioctl command: {0}")]
    Kvm(#[source] kvm_ioctls::Error),
}

#[cfg(feature = "hotplug")]
/// Errror associated with resize instance
#[derive(Debug, thiserror::Error)]
pub enum VcpuResizeError {
    /// vcpu is in hotplug process
    #[error("vcpu is in hotplug process")]
    VcpuIsHotplugging,

    /// Cannot update the configuration of the microvm pre boot.
    #[error("resize vcpu operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// Expected vcpu exceed max count
    #[error("expected vcpu exceed max count")]
    ExpectedVcpuExceedMax,

    /// vcpu 0 can't be removed
    #[error("vcpu 0 can't be removed")]
    Vcpu0CanNotBeRemoved,

    /// Lack removable vcpu
    #[error("Removable vcpu not enough, removable vcpu num: {0}, number to remove: {1}, present vcpu count {2}")]
    LackRemovableVcpus(u16, u16, u16),

    #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
    /// Cannot update the configuration by upcall channel.
    #[error("cannot update the configuration by upcall channel: {0}")]
    Upcall(#[source] dbs_upcall::UpcallClientError),
}

/// Result for vCPU manager operations
pub type Result<T> = std::result::Result<T, VcpuManagerError>;

#[derive(Debug, PartialEq, Copy, Clone)]
enum VcpuAction {
    None,
    Hotplug,
    Hotunplug,
}

/// Infos related to per vcpu
#[derive(Default)]
pub(crate) struct VcpuInfo {
    pub(crate) vcpu: Option<Vcpu>,
    vcpu_fd: Option<Arc<VcpuFd>>,
    handle: Option<VcpuHandle>,
    tid: u32,
}

impl std::fmt::Debug for VcpuInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VcpuInfo")
            .field("vcpu", &self.vcpu.is_some())
            .field("vcpu_fd", &self.vcpu_fd.is_some())
            .field("handle", &self.handle.is_some())
            .field("tid", &self.tid)
            .finish()
    }
}

/// Manage all vcpu related actions
pub struct VcpuManager {
    pub(crate) vcpu_infos: Vec<VcpuInfo>,
    vcpu_config: VcpuConfig,
    vcpu_seccomp_filter: BpfProgram,
    vcpu_state_event: EventFd,
    vcpu_state_sender: Sender<VcpuStateEvent>,
    support_immediate_exit: bool,

    // The purpose of putting a reference of IoManager here is to simplify the
    // design of the API when creating vcpus, and the IoManager has numerous OS
    // resources that need to be released when vmm exits. However, since
    // VcpuManager is referenced by VcpuEpollHandler and VcpuEpollHandler will
    // not be released when vmm is closed, we need to release io manager
    // manually when we exit all vcpus.
    io_manager: Option<IoManagerCached>,
    shared_info: Arc<RwLock<InstanceInfo>>,
    vm_as: GuestAddressSpaceImpl,
    pub(crate) vm_fd: Arc<VmFd>,

    action_sycn_tx: Option<Sender<bool>>,
    vcpus_in_action: (VcpuAction, Vec<u8>),
    pub(crate) reset_event_fd: Option<EventFd>,

    #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
    upcall_channel: Option<Arc<UpcallClient<DevMgrService>>>,

    // X86 specific fields.
    #[cfg(target_arch = "x86_64")]
    pub(crate) supported_cpuid: kvm_bindings::CpuId,
}

#[allow(clippy::too_many_arguments)]
impl VcpuManager {
    /// Get a new VcpuManager instance
    pub fn new(
        vm_fd: Arc<VmFd>,
        kvm_context: &KvmContext,
        vm_config_info: &VmConfigInfo,
        vm_as: GuestAddressSpaceImpl,
        vcpu_seccomp_filter: BpfProgram,
        shared_info: Arc<RwLock<InstanceInfo>>,
        io_manager: IoManagerCached,
        epoll_manager: EpollManager,
    ) -> Result<Arc<Mutex<Self>>> {
        let support_immediate_exit = kvm_context.kvm().check_extension(Cap::ImmediateExit);
        let max_vcpu_count = vm_config_info.max_vcpu_count;
        let kvm_max_vcpu_count = kvm_context.get_max_vcpus();

        // check the max vcpu count in kvm. max_vcpu_count is u8 and kvm_context.get_max_vcpus()
        // returns usize, so convert max_vcpu_count to usize instead of converting kvm max vcpu to
        // u8, to avoid wraping usize. Otherwise if kvm_max_vcpu_count is greater than 255, it'll
        // be casted into a smaller number.
        if max_vcpu_count as usize > kvm_max_vcpu_count {
            error!(
                "vcpu_manager: specified vcpu count {} is greater than max allowed count {} by kvm",
                max_vcpu_count, kvm_max_vcpu_count
            );
            return Err(VcpuManagerError::MaxVcpuLimitation(
                max_vcpu_count,
                kvm_max_vcpu_count,
            ));
        }

        let mut vcpu_infos = Vec::with_capacity(max_vcpu_count.into());
        vcpu_infos.resize_with(max_vcpu_count.into(), Default::default);

        let (tx, rx) = channel();
        let vcpu_state_event =
            EventFd::new(libc::EFD_NONBLOCK).map_err(VcpuManagerError::VcpuIO)?;
        let vcpu_state_event2 = vcpu_state_event
            .try_clone()
            .map_err(VcpuManagerError::VcpuIO)?;

        #[cfg(target_arch = "x86_64")]
        let supported_cpuid = kvm_context
            .supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)
            .map_err(VcpuManagerError::Kvm)?;
        #[cfg(target_arch = "x86_64")]
        let vpmu_feature_level = match vm_config_info.vpmu_feature {
            1 => dbs_arch::cpuid::VpmuFeatureLevel::LimitedlyEnabled,
            2 => dbs_arch::cpuid::VpmuFeatureLevel::FullyEnabled,
            _ => dbs_arch::cpuid::VpmuFeatureLevel::Disabled,
        };

        let vcpu_manager = Arc::new(Mutex::new(VcpuManager {
            vcpu_infos,
            vcpu_config: VcpuConfig {
                boot_vcpu_count: vm_config_info.vcpu_count,
                max_vcpu_count,
                threads_per_core: vm_config_info.cpu_topology.threads_per_core,
                cores_per_die: vm_config_info.cpu_topology.cores_per_die,
                dies_per_socket: vm_config_info.cpu_topology.dies_per_socket,
                sockets: vm_config_info.cpu_topology.sockets,
                #[cfg(target_arch = "x86_64")]
                vpmu_feature: vpmu_feature_level,
            },
            vcpu_seccomp_filter,
            vcpu_state_event,
            vcpu_state_sender: tx,
            support_immediate_exit,
            io_manager: Some(io_manager),
            shared_info,
            vm_as,
            vm_fd,
            action_sycn_tx: None,
            vcpus_in_action: (VcpuAction::None, Vec::new()),
            reset_event_fd: None,
            #[cfg(all(feature = "hotplug", feature = "dbs-upcall"))]
            upcall_channel: None,
            #[cfg(target_arch = "x86_64")]
            supported_cpuid,
        }));

        let handler = Box::new(VcpuEpollHandler {
            vcpu_manager: vcpu_manager.clone(),
            eventfd: vcpu_state_event2,
            rx,
        });
        epoll_manager.add_subscriber(handler);

        Ok(vcpu_manager)
    }

    /// get vcpu instances in vcpu manager
    pub fn vcpus(&self) -> Vec<&Vcpu> {
        let mut vcpus = Vec::new();
        for vcpu_info in &self.vcpu_infos {
            if let Some(vcpu) = &vcpu_info.vcpu {
                vcpus.push(vcpu);
            }
        }
        vcpus
    }

    /// get vcpu instances in vcpu manager as mut
    pub fn vcpus_mut(&mut self) -> Vec<&mut Vcpu> {
        let mut vcpus = Vec::new();
        for vcpu_info in &mut self.vcpu_infos {
            if let Some(vcpu) = &mut vcpu_info.vcpu {
                vcpus.push(vcpu);
            }
        }
        vcpus
    }

    /// add reset event fd for each vcpu, if the reset_event_fd is already set, error will be returned.
    pub fn set_reset_event_fd(&mut self, reset_event_fd: EventFd) -> Result<()> {
        if self.reset_event_fd.is_some() {
            return Err(VcpuManagerError::EventAlreadyExist);
        }
        self.reset_event_fd = Some(reset_event_fd);
        Ok(())
    }

    /// create default num of vcpus for bootup
    pub fn create_boot_vcpus(
        &mut self,
        request_ts: TimestampUs,
        entry_addr: GuestAddress,
    ) -> Result<()> {
        info!("create boot vcpus");
        self.create_vcpus(
            self.vcpu_config.boot_vcpu_count,
            Some(request_ts),
            Some(entry_addr),
        )?;

        Ok(())
    }

    /// start the boot vcpus
    pub fn start_boot_vcpus(&mut self, vmm_seccomp_filter: BpfProgram) -> Result<()> {
        info!("start boot vcpus");
        self.start_vcpus(self.vcpu_config.boot_vcpu_count, vmm_seccomp_filter, true)?;

        Ok(())
    }

    /// create a specified num of vcpu
    /// note: we can't create vcpus again until the previously created vcpus are
    /// started
    pub fn create_vcpus(
        &mut self,
        vcpu_count: u8,
        request_ts: Option<TimestampUs>,
        entry_addr: Option<GuestAddress>,
    ) -> Result<Vec<u8>> {
        info!("create vcpus");
        if vcpu_count > self.vcpu_config.max_vcpu_count {
            return Err(VcpuManagerError::ExpectedVcpuExceedMax);
        }

        let request_ts = request_ts.unwrap_or_default();
        let mut created_cpus = Vec::new();
        for cpu_id in self.calculate_available_vcpus(vcpu_count) {
            self.create_vcpu(cpu_id, request_ts.clone(), entry_addr)?;
            created_cpus.push(cpu_id);
        }

        Ok(created_cpus)
    }

    /// start a specified num of vcpu
    pub fn start_vcpus(
        &mut self,
        vcpu_count: u8,
        vmm_seccomp_filter: BpfProgram,
        need_resume: bool,
    ) -> Result<()> {
        info!("start vcpus");
        Vcpu::register_kick_signal_handler();
        self.activate_vcpus(vcpu_count, need_resume)?;

        // Load seccomp filters for the VMM thread.
        // Execution panics if filters cannot be loaded, use --seccomp-level=0 if skipping filters
        // altogether is the desired behaviour.
        if let Err(e) = apply_filter(&vmm_seccomp_filter) {
            if !matches!(e, SecError::EmptyFilter) {
                return Err(VcpuManagerError::SeccompFilters(e));
            }
        }

        Ok(())
    }

    /// pause all vcpus
    pub fn pause_all_vcpus(&mut self) -> Result<()> {
        self.pause_vcpus(&self.present_vcpus())
    }

    /// resume all vcpus
    pub fn resume_all_vcpus(&mut self) -> Result<()> {
        self.resume_vcpus(&self.present_vcpus())
    }

    /// exit all vcpus, and never restart again
    pub fn exit_all_vcpus(&mut self) -> Result<()> {
        self.exit_vcpus(&self.present_vcpus())?;
        // clear all vcpu infos
        self.vcpu_infos.clear();
        // release io manager's reference manually
        self.io_manager.take();

        Ok(())
    }

    /// revalidate IoManager cache of all vcpus
    pub fn revalidate_all_vcpus_cache(&mut self) -> Result<()> {
        self.revalidate_vcpus_cache(&self.present_vcpus())
    }

    /// return all present vcpus
    pub fn present_vcpus(&self) -> Vec<u8> {
        self.vcpu_infos
            .iter()
            .enumerate()
            .filter(|(_i, info)| info.handle.is_some())
            .map(|(i, _info)| i as u8)
            .collect()
    }

    /// Get available vcpus to create with target vcpu_count
    /// Argument:
    /// * vcpu_count: target vcpu_count online in VcpuManager.
    /// Return:
    /// * return available vcpu ids to create vcpu .
    fn calculate_available_vcpus(&self, vcpu_count: u8) -> Vec<u8> {
        let present_vcpus_count = self.present_vcpus_count();
        let mut available_vcpus = Vec::new();

        if present_vcpus_count < vcpu_count {
            let mut size = vcpu_count - present_vcpus_count;
            for cpu_id in 0..self.vcpu_config.max_vcpu_count {
                let info = &self.vcpu_infos[cpu_id as usize];
                if info.handle.is_none() {
                    available_vcpus.push(cpu_id);
                    size -= 1;
                    if size == 0 {
                        break;
                    }
                }
            }
        }

        available_vcpus
    }

    /// Present vcpus count
    fn present_vcpus_count(&self) -> u8 {
        self.vcpu_infos
            .iter()
            .fold(0, |sum, info| sum + info.handle.is_some() as u8)
    }

    /// Configure single vcpu
    fn configure_single_vcpu(
        &mut self,
        entry_addr: Option<GuestAddress>,
        vcpu: &mut Vcpu,
    ) -> std::result::Result<(), VcpuError> {
        vcpu.configure(
            &self.vcpu_config,
            &self.vm_fd,
            &self.vm_as,
            entry_addr,
            None,
        )
    }

    fn create_vcpu(
        &mut self,
        cpu_index: u8,
        request_ts: TimestampUs,
        entry_addr: Option<GuestAddress>,
    ) -> Result<()> {
        info!("creating vcpu {}", cpu_index);
        if self.vcpu_infos.get(cpu_index as usize).is_none() {
            return Err(VcpuManagerError::VcpuNotFound(cpu_index));
        }
        // We will reuse the kvm's vcpufd after first creation, for we can't
        // create vcpufd with same id in one kvm instance.
        let kvm_vcpu = match &self.vcpu_infos[cpu_index as usize].vcpu_fd {
            Some(vcpu_fd) => vcpu_fd.clone(),
            None => {
                let vcpu_fd = Arc::new(
                    self.vm_fd
                        .create_vcpu(cpu_index as u64)
                        .map_err(VcpuError::VcpuFd)
                        .map_err(VcpuManagerError::Vcpu)?,
                );
                self.vcpu_infos[cpu_index as usize].vcpu_fd = Some(vcpu_fd.clone());
                vcpu_fd
            }
        };

        let mut vcpu = self.create_vcpu_arch(cpu_index, kvm_vcpu, request_ts)?;
        self.configure_single_vcpu(entry_addr, &mut vcpu)
            .map_err(VcpuManagerError::Vcpu)?;
        self.vcpu_infos[cpu_index as usize].vcpu = Some(vcpu);

        Ok(())
    }

    fn start_vcpu(&mut self, cpu_index: u8, barrier: Arc<Barrier>) -> Result<()> {
        info!("starting vcpu {}", cpu_index);
        if self.vcpu_infos.get(cpu_index as usize).is_none() {
            return Err(VcpuManagerError::VcpuNotFound(cpu_index));
        }
        if let Some(vcpu) = self.vcpu_infos[cpu_index as usize].vcpu.take() {
            let handle = vcpu
                .start_threaded(self.vcpu_seccomp_filter.clone(), barrier)
                .map_err(VcpuManagerError::Vcpu)?;
            self.vcpu_infos[cpu_index as usize].handle = Some(handle);
            Ok(())
        } else {
            Err(VcpuManagerError::VcpuNotCreate)
        }
    }

    fn get_vcpus_tid(&mut self, cpu_indexes: &[u8]) -> Result<()> {
        for cpu_id in cpu_indexes {
            if self.vcpu_infos.get(*cpu_id as usize).is_none() {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
            if let Some(handle) = &self.vcpu_infos[*cpu_id as usize].handle {
                handle
                    .send_event(VcpuEvent::Gettid)
                    .map_err(VcpuManagerError::VcpuEvent)?;
            } else {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
        }

        for cpu_id in cpu_indexes {
            if let Some(handle) = &self.vcpu_infos[*cpu_id as usize].handle {
                match handle
                    .response_receiver()
                    .recv_timeout(Duration::from_millis(CPU_RECV_TIMEOUT_MS))
                {
                    Ok(VcpuResponse::Tid(_, id)) => self.vcpu_infos[*cpu_id as usize].tid = id,
                    Err(e) => {
                        error!("vCPU get tid error! {:?}", e);
                        return Err(VcpuManagerError::VcpuGettid);
                    }
                    _ => {
                        error!("vCPU get tid error!");
                        return Err(VcpuManagerError::VcpuGettid);
                    }
                }
            } else {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
        }

        // Save all vCPU thread ID to self.shared_info
        let tids: Vec<(u8, u32)> = cpu_indexes
            .iter()
            .map(|cpu_id| (*cpu_id, self.vcpu_infos[*cpu_id as usize].tid))
            .collect();

        // Append the new started vcpu thread IDs into self.shared_info
        self.shared_info
            .write()
            .unwrap()
            .tids
            .extend_from_slice(&tids[..]);

        Ok(())
    }

    fn revalidate_vcpus_cache(&mut self, cpu_indexes: &[u8]) -> Result<()> {
        for cpu_id in cpu_indexes {
            if self.vcpu_infos.get(*cpu_id as usize).is_none() {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
            if let Some(handle) = &self.vcpu_infos[*cpu_id as usize].handle {
                handle
                    .send_event(VcpuEvent::RevalidateCache)
                    .map_err(VcpuManagerError::VcpuEvent)?;
            } else {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
        }

        Ok(())
    }

    fn pause_vcpus(&mut self, cpu_indexes: &[u8]) -> Result<()> {
        for cpu_id in cpu_indexes {
            if self.vcpu_infos.get(*cpu_id as usize).is_none() {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
            if let Some(handle) = &self.vcpu_infos[*cpu_id as usize].handle {
                handle
                    .send_event(VcpuEvent::Pause)
                    .map_err(VcpuManagerError::VcpuEvent)?;
            } else {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
        }

        Ok(())
    }

    fn resume_vcpus(&mut self, cpu_indexes: &[u8]) -> Result<()> {
        for cpu_id in cpu_indexes {
            if self.vcpu_infos.get(*cpu_id as usize).is_none() {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
            if let Some(handle) = &self.vcpu_infos[*cpu_id as usize].handle {
                handle
                    .send_event(VcpuEvent::Resume)
                    .map_err(VcpuManagerError::VcpuEvent)?;
            } else {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
        }

        Ok(())
    }

    // exit vcpus and notify the vmm exit event
    fn exit_vcpus(&mut self, cpu_indexes: &[u8]) -> Result<()> {
        info!("exiting vcpus {:?}", cpu_indexes);
        for cpu_id in cpu_indexes {
            if self.vcpu_infos.get(*cpu_id as usize).is_none() {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
            if let Some(handle) = &self.vcpu_infos[*cpu_id as usize].handle {
                handle
                    .send_event(VcpuEvent::Exit)
                    .map_err(VcpuManagerError::VcpuEvent)?;
            } else {
                return Err(VcpuManagerError::VcpuNotFound(*cpu_id));
            }
        }

        for cpu_id in cpu_indexes {
            let handle = self.vcpu_infos[*cpu_id as usize].handle.take().unwrap();
            handle
                .join_vcpu_thread()
                .map_err(|e| error!("vcpu exit error! {:?}", e))
                .ok();
        }

        let tids: &mut Vec<(u8, u32)> = &mut self
            .shared_info
            .write()
            .expect(
                "Failed to stop vcpus because shared info couldn't be written due to poisoned lock",
            )
            .tids;

        // Here's a trick: since we always stop the vcpus started latest,
        // thus it's ok here to remove the stopped vcpus from end to head.
        tids.truncate(tids.len() - cpu_indexes.len());

        Ok(())
    }

    fn stop_vcpus_in_action(&mut self) -> Result<()> {
        let vcpus_in_action = self.vcpus_in_action.1.clone();
        self.exit_vcpus(&vcpus_in_action)
    }

    fn activate_vcpus(&mut self, vcpu_count: u8, need_resume: bool) -> Result<Vec<u8>> {
        let present_vcpus_count = self.present_vcpus_count();
        if vcpu_count > self.vcpu_config.max_vcpu_count {
            return Err(VcpuManagerError::ExpectedVcpuExceedMax);
        } else if vcpu_count < present_vcpus_count {
            return Ok(Vec::new());
        }

        let available_vcpus = self.calculate_available_vcpus(vcpu_count);
        let barrier = Arc::new(Barrier::new(available_vcpus.len() + 1_usize));
        for cpu_id in available_vcpus.iter() {
            self.start_vcpu(*cpu_id, barrier.clone())?;
        }
        barrier.wait();

        self.get_vcpus_tid(&available_vcpus)?;
        if need_resume {
            self.resume_vcpus(&available_vcpus)?;
        }

        Ok(available_vcpus)
    }

    fn sync_action_finish(&mut self, got_error: bool) {
        if let Some(tx) = self.action_sycn_tx.take() {
            if let Err(e) = tx.send(got_error) {
                debug!("cpu sync action send to closed channel {}", e);
            }
        }
    }

    fn set_vcpus_action(&mut self, action: VcpuAction, vcpus: Vec<u8>) {
        self.vcpus_in_action = (action, vcpus);
    }

    fn get_vcpus_action(&self) -> VcpuAction {
        self.vcpus_in_action.0
    }
}

#[cfg(target_arch = "x86_64")]
impl VcpuManager {
    fn create_vcpu_arch(
        &self,
        cpu_index: u8,
        vcpu_fd: Arc<VcpuFd>,
        request_ts: TimestampUs,
    ) -> Result<Vcpu> {
        // It's safe to unwrap because guest_kernel always exist until vcpu manager done
        Vcpu::new_x86_64(
            cpu_index,
            vcpu_fd,
            // safe to unwrap
            self.io_manager.as_ref().unwrap().clone(),
            self.supported_cpuid.clone(),
            self.reset_event_fd.as_ref().unwrap().try_clone().unwrap(),
            self.vcpu_state_event.try_clone().unwrap(),
            self.vcpu_state_sender.clone(),
            request_ts,
            self.support_immediate_exit,
        )
        .map_err(VcpuManagerError::Vcpu)
    }
}

#[cfg(target_arch = "aarch64")]
impl VcpuManager {
    // On aarch64, the vCPUs need to be created (i.e call KVM_CREATE_VCPU) and configured before
    // setting up the IRQ chip because the `KVM_CREATE_VCPU` ioctl will return error if the IRQCHIP
    // was already initialized.
    // Search for `kvm_arch_vcpu_create` in arch/arm/kvm/arm.c.
    fn create_vcpu_arch(
        &self,
        cpu_index: u8,
        vcpu_fd: Arc<VcpuFd>,
        request_ts: TimestampUs,
    ) -> Result<Vcpu> {
        Vcpu::new_aarch64(
            cpu_index,
            vcpu_fd,
            // safe to unwrap
            self.io_manager.as_ref().unwrap().clone(),
            self.reset_event_fd.as_ref().unwrap().try_clone().unwrap(),
            self.vcpu_state_event.try_clone().unwrap(),
            self.vcpu_state_sender.clone(),
            request_ts.clone(),
            self.support_immediate_exit,
        )
        .map_err(VcpuManagerError::Vcpu)
    }
}

#[cfg(feature = "hotplug")]
mod hotplug {
    #[cfg(feature = "dbs-upcall")]
    use super::*;
    #[cfg(feature = "dbs-upcall")]
    use dbs_upcall::{CpuDevRequest, DevMgrRequest};
    #[cfg(feature = "dbs-upcall")]
    use std::cmp::Ordering;

    #[cfg(all(target_arch = "x86_64", feature = "dbs-upcall"))]
    use dbs_boot::mptable::APIC_VERSION;
    #[cfg(all(target_arch = "aarch64"))]
    const APIC_VERSION: u8 = 0;

    #[cfg(feature = "dbs-upcall")]
    impl VcpuManager {
        /// add upcall channel for vcpu manager
        pub fn set_upcall_channel(
            &mut self,
            upcall_channel: Option<Arc<UpcallClient<DevMgrService>>>,
        ) {
            self.upcall_channel = upcall_channel;
        }

        /// resize the count of vcpu in runtime
        pub fn resize_vcpu(
            &mut self,
            vcpu_count: u8,
            sync_tx: Option<Sender<bool>>,
        ) -> std::result::Result<(), VcpuManagerError> {
            if self.get_vcpus_action() != VcpuAction::None {
                return Err(VcpuManagerError::VcpuResize(
                    VcpuResizeError::VcpuIsHotplugging,
                ));
            }
            self.action_sycn_tx = sync_tx;

            if let Some(upcall) = self.upcall_channel.clone() {
                let now_vcpu = self.present_vcpus_count();
                info!("resize vcpu: now: {}, desire: {}", now_vcpu, vcpu_count);
                match vcpu_count.cmp(&now_vcpu) {
                    Ordering::Equal => {
                        info!("resize vcpu: no need to resize");
                        self.sync_action_finish(false);
                        Ok(())
                    }
                    Ordering::Greater => self.do_add_vcpu(vcpu_count, upcall),
                    Ordering::Less => self.do_del_vcpu(vcpu_count, upcall),
                }
            } else {
                Err(VcpuManagerError::VcpuResize(
                    VcpuResizeError::UpdateNotAllowedPostBoot,
                ))
            }
        }

        fn do_add_vcpu(
            &mut self,
            vcpu_count: u8,
            upcall_client: Arc<UpcallClient<DevMgrService>>,
        ) -> std::result::Result<(), VcpuManagerError> {
            info!("resize vcpu: add");
            if vcpu_count > self.vcpu_config.max_vcpu_count {
                return Err(VcpuManagerError::VcpuResize(
                    VcpuResizeError::ExpectedVcpuExceedMax,
                ));
            }

            let created_vcpus = self.create_vcpus(vcpu_count, None, None)?;
            let cpu_ids = self.activate_vcpus(vcpu_count, true).map_err(|e| {
                // we need to rollback when activate vcpu error
                error!("activate vcpu error, rollback! {:?}", e);
                let activated_vcpus: Vec<u8> = created_vcpus
                    .iter()
                    .filter(|&cpu_id| self.vcpu_infos[*cpu_id as usize].handle.is_some())
                    .copied()
                    .collect();
                if let Err(e) = self.exit_vcpus(&activated_vcpus) {
                    error!("try to rollback error, stop_vcpu: {:?}", e);
                }
                e
            })?;

            let mut cpu_ids_array = [0u8; (u8::MAX as usize) + 1];
            cpu_ids_array[..cpu_ids.len()].copy_from_slice(&cpu_ids[..cpu_ids.len()]);
            let req = DevMgrRequest::AddVcpu(CpuDevRequest {
                count: cpu_ids.len() as u8,
                apic_ids: cpu_ids_array,
                apic_ver: APIC_VERSION,
            });
            self.send_upcall_action(upcall_client, req)?;

            self.set_vcpus_action(VcpuAction::Hotplug, cpu_ids);

            Ok(())
        }

        fn do_del_vcpu(
            &mut self,
            vcpu_count: u8,
            upcall_client: Arc<UpcallClient<DevMgrService>>,
        ) -> std::result::Result<(), VcpuManagerError> {
            info!("resize vcpu: delete");
            if vcpu_count == 0 {
                return Err(VcpuManagerError::VcpuResize(
                    VcpuResizeError::Vcpu0CanNotBeRemoved,
                ));
            }

            let mut cpu_ids = self.calculate_removable_vcpus();
            let cpu_num_to_be_del = (self.present_vcpus_count() - vcpu_count) as usize;
            if cpu_num_to_be_del >= cpu_ids.len() {
                return Err(VcpuManagerError::VcpuResize(
                    VcpuResizeError::LackRemovableVcpus(
                        cpu_ids.len() as u16,
                        cpu_num_to_be_del as u16,
                        self.present_vcpus_count() as u16,
                    ),
                ));
            }

            cpu_ids.reverse();
            cpu_ids.truncate(cpu_num_to_be_del);

            let mut cpu_ids_array = [0u8; 256];
            cpu_ids_array[..cpu_ids.len()].copy_from_slice(&cpu_ids[..cpu_ids.len()]);
            let req = DevMgrRequest::DelVcpu(CpuDevRequest {
                count: cpu_num_to_be_del as u8,
                apic_ids: cpu_ids_array,
                apic_ver: APIC_VERSION,
            });
            self.send_upcall_action(upcall_client, req)?;

            self.set_vcpus_action(VcpuAction::Hotunplug, cpu_ids);

            Ok(())
        }

        #[cfg(test)]
        fn send_upcall_action(
            &self,
            _upcall_client: Arc<UpcallClient<DevMgrService>>,
            _request: DevMgrRequest,
        ) -> std::result::Result<(), VcpuManagerError> {
            Ok(())
        }

        #[cfg(not(test))]
        fn send_upcall_action(
            &self,
            upcall_client: Arc<UpcallClient<DevMgrService>>,
            request: DevMgrRequest,
        ) -> std::result::Result<(), VcpuManagerError> {
            // This is used to fix clippy warnings.
            use dbs_upcall::{DevMgrResponse, UpcallClientRequest, UpcallClientResponse};

            let vcpu_state_event = self.vcpu_state_event.try_clone().unwrap();
            let vcpu_state_sender = self.vcpu_state_sender.clone();

            upcall_client
                .send_request(
                    UpcallClientRequest::DevMgr(request),
                    Box::new(move |result| match result {
                        UpcallClientResponse::DevMgr(response) => {
                            if let DevMgrResponse::CpuDev(resp) = response {
                                vcpu_state_sender
                                    .send(VcpuStateEvent::Hotplug((
                                        resp.result,
                                        resp.info.apic_id_index,
                                    )))
                                    .unwrap();
                                vcpu_state_event.write(1).unwrap();
                            }
                        }
                        UpcallClientResponse::UpcallReset => {
                            vcpu_state_sender
                                .send(VcpuStateEvent::Hotplug((0, 0)))
                                .unwrap();
                            vcpu_state_event.write(1).unwrap();
                        }
                        #[cfg(test)]
                        UpcallClientResponse::FakeResponse => {
                            panic!("shouldn't happen");
                        }
                    }),
                )
                .map_err(VcpuResizeError::Upcall)
                .map_err(VcpuManagerError::VcpuResize)
        }

        /// Get removable vcpus.
        /// Return:
        /// * return removable vcpu_id with cascade order.
        fn calculate_removable_vcpus(&self) -> Vec<u8> {
            self.present_vcpus()
        }
    }
}

struct VcpuEpollHandler {
    vcpu_manager: Arc<Mutex<VcpuManager>>,
    eventfd: EventFd,
    rx: Receiver<VcpuStateEvent>,
}

impl VcpuEpollHandler {
    fn process_cpu_state_event(&mut self, _ops: &mut EventOps) {
        // It's level triggered, so it's safe to ignore the result.
        let _ = self.eventfd.read();
        while let Ok(event) = self.rx.try_recv() {
            match event {
                VcpuStateEvent::Hotplug((success, cpu_count)) => {
                    info!("get vcpu event, cpu_index {}", cpu_count);
                    self.process_cpu_action(success != 0, cpu_count);
                }
            }
        }
    }

    fn process_cpu_action(&self, success: bool, _cpu_index: u32) {
        let mut vcpu_manager = self.vcpu_manager.lock().unwrap();
        if success {
            match vcpu_manager.get_vcpus_action() {
                VcpuAction::Hotplug => {
                    // Notify hotplug success
                    vcpu_manager.sync_action_finish(false);
                }
                VcpuAction::Hotunplug => {
                    if let Err(e) = vcpu_manager.stop_vcpus_in_action() {
                        error!("stop vcpus in action error: {:?}", e);
                    }
                    // notify hotunplug success
                    vcpu_manager.sync_action_finish(false);
                }
                VcpuAction::None => {
                    error!("cannot be here");
                }
            };
            vcpu_manager.set_vcpus_action(VcpuAction::None, Vec::new());

            vcpu_manager.sync_action_finish(true);
            // TODO(sicun): rollback
        }
    }
}

impl MutEventSubscriber for VcpuEpollHandler {
    fn process(&mut self, events: Events, ops: &mut EventOps) {
        let vcpu_state_eventfd = self.eventfd.as_raw_fd();

        match events.fd() {
            fd if fd == vcpu_state_eventfd => self.process_cpu_state_event(ops),
            _ => error!("vcpu manager epoll handler: unknown event"),
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        ops.add(Events::new(&self.eventfd, EventSet::IN)).unwrap();
    }
}
