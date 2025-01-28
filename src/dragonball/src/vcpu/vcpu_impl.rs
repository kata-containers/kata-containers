// Copyright (C) 2019-2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! The implementation for per vcpu

use std::cell::Cell;
use std::result;
use std::sync::atomic::{fence, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, Barrier};
use std::thread;

use dbs_utils::metric::IncMetric;
use dbs_utils::time::TimestampUs;
use kvm_bindings::{KVM_SYSTEM_EVENT_RESET, KVM_SYSTEM_EVENT_SHUTDOWN};
use kvm_ioctls::{VcpuExit, VcpuFd};
use libc::{c_int, c_void, siginfo_t};
use log::{error, info};
use seccompiler::{apply_filter, BpfProgram, Error as SecError};
use vmm_sys_util::eventfd::EventFd;
use vmm_sys_util::signal::{register_signal_handler, Killable};

use super::sm::StateMachine;
use crate::metric::{VcpuMetrics, METRICS};
use crate::signal_handler::sigrtmin;
use crate::IoManagerCached;

#[cfg(target_arch = "x86_64")]
#[path = "x86_64.rs"]
mod x86_64;

#[cfg(target_arch = "aarch64")]
#[path = "aarch64.rs"]
mod aarch64;

#[cfg(target_arch = "x86_64")]
const MAGIC_IOPORT_BASE: u16 = 0xdbdb;
#[cfg(target_arch = "x86_64")]
const MAGIC_IOPORT_DEBUG_INFO: u16 = MAGIC_IOPORT_BASE;

/// Signal number (SIGRTMIN) used to kick Vcpus.
pub const VCPU_RTSIG_OFFSET: i32 = 0;

#[cfg(target_arch = "x86_64")]
/// Errors associated with the wrappers over KVM ioctls.
#[derive(Debug, thiserror::Error)]
pub enum VcpuError {
    /// Failed to signal Vcpu.
    #[error("cannot signal the vCPU thread")]
    SignalVcpu(#[source] vmm_sys_util::errno::Error),

    /// Cannot open the vCPU file descriptor.
    #[error("cannot open the vCPU file descriptor")]
    VcpuFd(#[source] kvm_ioctls::Error),

    /// Cannot spawn a new vCPU thread.
    #[error("cannot spawn vCPU thread")]
    VcpuSpawn(#[source] std::io::Error),

    /// Cannot cleanly initialize vCPU TLS.
    #[error("cannot cleanly initialize TLS fro vCPU")]
    VcpuTlsInit,

    /// Vcpu not present in TLS.
    #[error("vCPU not present in the TLS")]
    VcpuTlsNotPresent,

    /// Unexpected KVM_RUN exit reason
    #[error("Unexpected KVM_RUN exit reason")]
    VcpuUnhandledKvmExit,

    /// Pause vcpu failed
    #[error("failed to pause vcpus")]
    PauseFailed,

    /// Kvm Ioctl Error
    #[error("failure in issuing KVM ioctl command")]
    Kvm(#[source] kvm_ioctls::Error),

    /// Msr error
    #[error("failure to deal with MSRs")]
    Msr(vmm_sys_util::fam::Error),

    /// A call to cpuid instruction failed on x86_64.
    #[error("failure while configuring CPUID for virtual CPU on x86_64")]
    CpuId(dbs_arch::cpuid::Error),

    /// Error configuring the floating point related registers on x86_64.
    #[error("failure while configuring the floating point related registers on x86_64")]
    FPUConfiguration(dbs_arch::regs::Error),

    /// Cannot set the local interruption due to bad configuration on x86_64.
    #[error("cannot set the local interruption due to bad configuration on x86_64")]
    LocalIntConfiguration(dbs_arch::interrupts::Error),

    /// Error configuring the MSR registers on x86_64.
    #[error("failure while configuring the MSR registers on x86_64")]
    MSRSConfiguration(dbs_arch::regs::Error),

    /// Error configuring the general purpose registers on x86_64.
    #[error("failure while configuring the general purpose registers on x86_64")]
    REGSConfiguration(dbs_arch::regs::Error),

    /// Error configuring the special registers on x86_64.
    #[error("failure while configuring the special registers on x86_64")]
    SREGSConfiguration(dbs_arch::regs::Error),

    /// Error configuring the page table on x86_64.
    #[error("failure while configuring the page table on x86_64")]
    PageTable(dbs_boot::Error),

    /// The call to KVM_SET_CPUID2 failed on x86_64.
    #[error("failure while calling KVM_SET_CPUID2 on x86_64")]
    SetSupportedCpusFailed(#[source] kvm_ioctls::Error),
}

#[cfg(target_arch = "aarch64")]
/// Errors associated with the wrappers over KVM ioctls.
#[derive(Debug, thiserror::Error)]
pub enum VcpuError {
    /// Failed to signal Vcpu.
    #[error("cannot signal the vCPU thread")]
    SignalVcpu(#[source] vmm_sys_util::errno::Error),

    /// Cannot open the vCPU file descriptor.
    #[error("cannot open the vCPU file descriptor")]
    VcpuFd(#[source] kvm_ioctls::Error),

    /// Cannot spawn a new vCPU thread.
    #[error("cannot spawn vCPU thread")]
    VcpuSpawn(#[source] std::io::Error),

    /// Cannot cleanly initialize vCPU TLS.
    #[error("cannot cleanly initialize TLS fro vCPU")]
    VcpuTlsInit,

    /// Vcpu not present in TLS.
    #[error("vCPU not present in the TLS")]
    VcpuTlsNotPresent,

    /// Unexpected KVM_RUN exit reason
    #[error("Unexpected KVM_RUN exit reason")]
    VcpuUnhandledKvmExit,

    /// Pause vcpu failed
    #[error("failed to pause vcpus")]
    PauseFailed,

    /// Kvm Ioctl Error
    #[error("failure in issuing KVM ioctl command")]
    Kvm(#[source] kvm_ioctls::Error),

    /// Msr error
    #[error("failure to deal with MSRs")]
    Msr(vmm_sys_util::fam::Error),

    #[cfg(target_arch = "aarch64")]
    /// Error configuring the general purpose aarch64 registers on aarch64.
    #[error("failure while configuring the general purpose registers on aarch64")]
    REGSConfiguration(dbs_arch::regs::Error),

    #[cfg(target_arch = "aarch64")]
    /// Error setting up the global interrupt controller on aarch64.
    #[error("failure while setting up the global interrupt controller on aarch64")]
    SetupGIC(dbs_arch::gic::Error),

    #[cfg(target_arch = "aarch64")]
    /// Error getting the Vcpu preferred target on aarch64.
    #[error("failure while getting the vCPU preferred target on aarch64")]
    VcpuArmPreferredTarget(kvm_ioctls::Error),

    #[cfg(target_arch = "aarch64")]
    /// Error doing vCPU Init on aarch64.
    #[error("failure while doing vCPU init on aarch64")]
    VcpuArmInit(kvm_ioctls::Error),
}

/// Result for Vcpu related operations.
pub type Result<T> = result::Result<T, VcpuError>;

/// List of events that the Vcpu can receive.
#[derive(Debug)]
pub enum VcpuEvent {
    /// Kill the Vcpu.
    Exit,
    /// Pause the Vcpu.
    Pause,
    /// Event that should resume the Vcpu.
    Resume,
    /// Get vcpu thread tid
    Gettid,

    /// Event to revalidate vcpu IoManager cache
    RevalidateCache,
}

/// List of responses that the Vcpu reports.
pub enum VcpuResponse {
    /// Vcpu is paused.
    Paused,
    /// Vcpu is resumed.
    Resumed,
    /// Vcpu index and thread tid.
    Tid(u8, u32),
    /// Requested Vcpu operation is not allowed.
    NotAllowed,
    /// Requestion action encountered an error
    Error(VcpuError),
    /// Vcpu IoManager cache is revalidated
    CacheRevalidated,
}

#[derive(Debug, PartialEq)]
/// Vcpu Hotplug Result returned from the guest
pub enum VcpuResizeResult {
    /// All vCPU hotplug / hot-unplug operations are successful
    Success = 0,
    /// vCPU hotplug / hot-unplug failed
    Failed = 1,
}

/// List of events that the vcpu_state_sender can send.
pub enum VcpuStateEvent {
    /// (result, response) for hotplug / hot-unplugged.
    /// response records how many cpu has successfully being hotplugged / hot-unplugged.
    Hotplug((VcpuResizeResult, u32)),
}

/// Wrapper over vCPU that hides the underlying interactions with the vCPU thread.
pub struct VcpuHandle {
    event_sender: Sender<VcpuEvent>,
    response_receiver: Receiver<VcpuResponse>,
    vcpu_thread: thread::JoinHandle<()>,
}

impl VcpuHandle {
    /// Send event to vCPU thread
    pub fn send_event(&self, event: VcpuEvent) -> Result<()> {
        // Use expect() to crash if the other thread closed this channel.
        self.event_sender
            .send(event)
            .expect("event sender channel closed on vcpu end.");
        // Kick the vCPU so it picks up the message.
        self.vcpu_thread
            .kill(sigrtmin() + VCPU_RTSIG_OFFSET)
            .map_err(VcpuError::SignalVcpu)?;
        Ok(())
    }

    /// Receive response from vcpu thread
    pub fn response_receiver(&self) -> &Receiver<VcpuResponse> {
        &self.response_receiver
    }

    #[allow(dead_code)]
    /// Join the vcpu thread
    pub fn join_vcpu_thread(self) -> thread::Result<()> {
        self.vcpu_thread.join()
    }
}

#[derive(PartialEq)]
enum VcpuEmulation {
    Handled,
    Interrupted,
    Stopped,
}

/// A wrapper around creating and using a kvm-based VCPU.
pub struct Vcpu {
    // vCPU fd used by the vCPU
    fd: Arc<VcpuFd>,
    // vCPU id info
    id: u8,
    // Io manager Cached for facilitating IO operations
    io_mgr: IoManagerCached,
    // Records vCPU create time stamp
    create_ts: TimestampUs,

    // The receiving end of events channel owned by the vcpu side.
    event_receiver: Receiver<VcpuEvent>,
    // The transmitting end of the events channel which will be given to the handler.
    event_sender: Option<Sender<VcpuEvent>>,
    // The receiving end of the responses channel which will be given to the handler.
    response_receiver: Option<Receiver<VcpuResponse>>,
    // The transmitting end of the responses channel owned by the vcpu side.
    response_sender: Sender<VcpuResponse>,
    // Event notifier for CPU hotplug.
    // After arm adapts to hotplug vcpu, the dead code macro needs to be removed
    #[cfg_attr(target_arch = "aarch64", allow(dead_code))]
    vcpu_state_event: EventFd,
    // CPU hotplug events.
    // After arm adapts to hotplug vcpu, the dead code macro needs to be removed
    #[cfg_attr(target_arch = "aarch64", allow(dead_code))]
    vcpu_state_sender: Sender<VcpuStateEvent>,

    // An `EventFd` that will be written into when this vcpu exits.
    exit_evt: EventFd,
    // Whether kvm used supports immediate_exit flag.
    support_immediate_exit: bool,

    // metrics for a vCPU.
    metrics: Arc<VcpuMetrics>,

    // CPUID information for the x86_64 CPU
    #[cfg(target_arch = "x86_64")]
    cpuid: kvm_bindings::CpuId,

    /// Multiprocessor affinity register recorded for aarch64
    #[cfg(target_arch = "aarch64")]
    pub(crate) mpidr: u64,
}

// Using this for easier explicit type-casting to help IDEs interpret the code.
type VcpuCell = Cell<Option<*const Vcpu>>;

impl Vcpu {
    thread_local!(static TLS_VCPU_PTR: VcpuCell = const { Cell::new(None) });

    /// Associates `self` with the current thread.
    ///
    /// It is a prerequisite to successfully run `init_thread_local_data()` before using
    /// `run_on_thread_local()` on the current thread.
    /// This function will return an error if there already is a `Vcpu` present in the TLS.
    fn init_thread_local_data(&mut self) -> Result<()> {
        Self::TLS_VCPU_PTR.with(|cell: &VcpuCell| {
            if cell.get().is_some() {
                return Err(VcpuError::VcpuTlsInit);
            }
            cell.set(Some(self as *const Vcpu));
            Ok(())
        })
    }

    /// Deassociates `self` from the current thread.
    ///
    /// Should be called if the current `self` had called `init_thread_local_data()` and
    /// now needs to move to a different thread.
    ///
    /// Fails if `self` was not previously associated with the current thread.
    fn reset_thread_local_data(&mut self) -> Result<()> {
        // Best-effort to clean up TLS. If the `Vcpu` was moved to another thread
        // _before_ running this, then there is nothing we can do.
        Self::TLS_VCPU_PTR.with(|cell: &VcpuCell| {
            if let Some(vcpu_ptr) = cell.get() {
                if vcpu_ptr == self as *const Vcpu {
                    Self::TLS_VCPU_PTR.with(|cell: &VcpuCell| cell.take());
                    return Ok(());
                }
            }
            Err(VcpuError::VcpuTlsNotPresent)
        })
    }

    /// Runs `func` for the `Vcpu` associated with the current thread.
    ///
    /// It requires that `init_thread_local_data()` was run on this thread.
    ///
    /// Fails if there is no `Vcpu` associated with the current thread.
    ///
    /// # Safety
    ///
    /// This is marked unsafe as it allows temporary aliasing through
    /// dereferencing from pointer an already borrowed `Vcpu`.
    unsafe fn run_on_thread_local<F>(func: F) -> Result<()>
    where
        F: FnOnce(&Vcpu),
    {
        Self::TLS_VCPU_PTR.with(|cell: &VcpuCell| {
            if let Some(vcpu_ptr) = cell.get() {
                // Dereferencing here is safe since `TLS_VCPU_PTR` is populated/non-empty,
                // and it is being cleared on `Vcpu::drop` so there is no dangling pointer.
                let vcpu_ref: &Vcpu = &*vcpu_ptr;
                func(vcpu_ref);
                Ok(())
            } else {
                Err(VcpuError::VcpuTlsNotPresent)
            }
        })
    }

    /// Registers a signal handler which makes use of TLS and kvm immediate exit to
    /// kick the vcpu running on the current thread, if there is one.
    pub fn register_kick_signal_handler() {
        extern "C" fn handle_signal(_: c_int, _: *mut siginfo_t, _: *mut c_void) {
            // This is safe because it's temporarily aliasing the `Vcpu` object, but we are
            // only reading `vcpu.fd` which does not change for the lifetime of the `Vcpu`.
            unsafe {
                let _ = Vcpu::run_on_thread_local(|vcpu| {
                    vcpu.fd.set_kvm_immediate_exit(1);
                    fence(Ordering::Release);
                });
            }
        }

        register_signal_handler(sigrtmin() + VCPU_RTSIG_OFFSET, handle_signal)
            .expect("Failed to register vcpu signal handler");
    }

    /// Returns the cpu index as seen by the guest OS.
    pub fn cpu_index(&self) -> u8 {
        self.id
    }

    /// Moves the vcpu to its own thread and constructs a VcpuHandle.
    /// The handle can be used to control the remote vcpu.
    pub fn start_threaded(
        mut self,
        seccomp_filter: BpfProgram,
        barrier: Arc<Barrier>,
    ) -> Result<VcpuHandle> {
        let event_sender = self.event_sender.take().unwrap();
        let response_receiver = self.response_receiver.take().unwrap();

        let vcpu_thread = thread::Builder::new()
            .name(format!("db_vcpu{}", self.cpu_index()))
            .spawn(move || {
                self.init_thread_local_data()
                    .expect("Cannot cleanly initialize vcpu TLS.");
                barrier.wait();
                self.run(seccomp_filter);
            })
            .map_err(VcpuError::VcpuSpawn)?;

        Ok(VcpuHandle {
            event_sender,
            response_receiver,
            vcpu_thread,
        })
    }

    /// Extract the vcpu running logic for test mocking.
    #[cfg(not(test))]
    pub fn emulate(fd: &VcpuFd) -> std::result::Result<VcpuExit<'_>, kvm_ioctls::Error> {
        fd.run()
    }

    /// Runs the vCPU in KVM context and handles the kvm exit reason.
    ///
    /// Returns error or enum specifying whether emulation was handled or interrupted.
    fn run_emulation(&mut self) -> Result<VcpuEmulation> {
        match Vcpu::emulate(&self.fd) {
            Ok(run) => {
                match run {
                    #[cfg(target_arch = "x86_64")]
                    VcpuExit::IoIn(addr, data) => {
                        let _ = self.io_mgr.pio_read(addr, data);
                        self.metrics.exit_io_in.inc();
                        Ok(VcpuEmulation::Handled)
                    }
                    #[cfg(target_arch = "x86_64")]
                    VcpuExit::IoOut(addr, data) => {
                        if !self.check_io_port_info(addr, data)? {
                            let _ = self.io_mgr.pio_write(addr, data);
                        }
                        self.metrics.exit_io_out.inc();
                        Ok(VcpuEmulation::Handled)
                    }
                    VcpuExit::MmioRead(addr, data) => {
                        let _ = self.io_mgr.mmio_read(addr, data);
                        self.metrics.exit_mmio_read.inc();
                        Ok(VcpuEmulation::Handled)
                    }
                    VcpuExit::MmioWrite(addr, data) => {
                        let _ = self.io_mgr.mmio_write(addr, data);
                        self.metrics.exit_mmio_write.inc();
                        Ok(VcpuEmulation::Handled)
                    }
                    VcpuExit::Hlt => {
                        info!("Received KVM_EXIT_HLT signal");
                        Err(VcpuError::VcpuUnhandledKvmExit)
                    }
                    VcpuExit::Shutdown => {
                        info!("Received KVM_EXIT_SHUTDOWN signal");
                        Err(VcpuError::VcpuUnhandledKvmExit)
                    }
                    // Documentation specifies that below kvm exits are considered errors.
                    VcpuExit::FailEntry(reason, cpu) => {
                        self.metrics.failures.inc();
                        error!("Received KVM_EXIT_FAIL_ENTRY signal, reason {reason}, cpu number {cpu}");
                        Err(VcpuError::VcpuUnhandledKvmExit)
                    }
                    VcpuExit::InternalError => {
                        self.metrics.failures.inc();
                        error!("Received KVM_EXIT_INTERNAL_ERROR signal");
                        Err(VcpuError::VcpuUnhandledKvmExit)
                    }
                    VcpuExit::SystemEvent(event_type, event_flags) => match event_type {
                        KVM_SYSTEM_EVENT_RESET | KVM_SYSTEM_EVENT_SHUTDOWN => {
                            info!(
                                "Received KVM_SYSTEM_EVENT: type: {}, event: {}",
                                event_type, event_flags
                            );
                            Ok(VcpuEmulation::Stopped)
                        }
                        _ => {
                            self.metrics.failures.inc();
                            error!(
                                "Received KVM_SYSTEM_EVENT signal type: {}, flag: {}",
                                event_type, event_flags
                            );
                            Err(VcpuError::VcpuUnhandledKvmExit)
                        }
                    },
                    r => {
                        self.metrics.failures.inc();
                        // TODO: Are we sure we want to finish running a vcpu upon
                        // receiving a vm exit that is not necessarily an error?
                        error!("Unexpected exit reason on vcpu run: {:?}", r);
                        Err(VcpuError::VcpuUnhandledKvmExit)
                    }
                }
            }
            // The unwrap on raw_os_error can only fail if we have a logic
            // error in our code in which case it is better to panic.
            Err(ref e) => {
                match e.errno() {
                    libc::EAGAIN => Ok(VcpuEmulation::Handled),
                    libc::EINTR => {
                        self.fd.set_kvm_immediate_exit(0);
                        // Notify that this KVM_RUN was interrupted.
                        Ok(VcpuEmulation::Interrupted)
                    }
                    _ => {
                        self.metrics.failures.inc();
                        error!("Failure during vcpu run: {}", e);
                        #[cfg(target_arch = "x86_64")]
                        {
                            error!(
                                "dump regs: {:?}, dump sregs: {:?}",
                                self.fd.get_regs(),
                                self.fd.get_sregs()
                            );
                        }
                        Err(VcpuError::VcpuUnhandledKvmExit)
                    }
                }
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    // checkout the io port that dragonball used only
    fn check_io_port_info(&self, addr: u16, data: &[u8]) -> Result<bool> {
        let mut checked = false;

        // debug info signal
        if addr == MAGIC_IOPORT_DEBUG_INFO && data.len() == 4 {
            let data = unsafe { std::ptr::read(data.as_ptr() as *const u32) };
            log::warn!("KDBG: guest kernel debug info: 0x{:x}", data);
            checked = true;
        };

        Ok(checked)
    }

    fn gettid() -> u32 {
        nix::unistd::gettid().as_raw() as u32
    }

    fn revalidate_cache(&mut self) -> Result<()> {
        self.io_mgr.revalidate_cache();

        Ok(())
    }

    /// Main loop of the vCPU thread.
    ///
    /// Runs the vCPU in KVM context in a loop. Handles KVM_EXITs then goes back in.
    /// Note that the state of the VCPU and associated VM must be setup first for this to do
    /// anything useful.
    pub fn run(&mut self, seccomp_filter: BpfProgram) {
        // Load seccomp filters for this vCPU thread.
        // Execution panics if filters cannot be loaded, use --seccomp-level=0 if skipping filters
        // altogether is the desired behaviour.
        if let Err(e) = apply_filter(&seccomp_filter) {
            if matches!(e, SecError::EmptyFilter) {
                info!("vCPU thread {} use empty seccomp filters.", self.id);
            } else {
                panic!(
                    "Failed to set the requested seccomp filters on vCPU {}: Error: {}",
                    self.id, e
                );
            }
        }

        info!("vcpu {} is running", self.cpu_index());

        // Start running the machine state in the `Paused` state.
        StateMachine::run(self, Self::paused);
    }

    // This is the main loop of the `Running` state.
    fn running(&mut self) -> StateMachine<Self> {
        // This loop is here just for optimizing the emulation path.
        // No point in ticking the state machine if there are no external events.
        loop {
            match self.run_emulation() {
                // Emulation ran successfully, continue.
                Ok(VcpuEmulation::Handled) => {
                    // We need to break here if kvm doesn't support
                    // immediate_exit flag. Because the signal sent from vmm
                    // thread may occurs when handling the vcpu exit events, and
                    // in this case the external vcpu events may not be handled
                    // correctly, so we need to check the event_receiver channel
                    // after handle vcpu exit events to decrease the window that
                    // doesn't handle the vcpu external events.
                    if !self.support_immediate_exit {
                        break;
                    }
                }
                // Emulation was interrupted, check external events.
                Ok(VcpuEmulation::Interrupted) => break,
                // Emulation was stopped due to reset or shutdown.
                Ok(VcpuEmulation::Stopped) => return StateMachine::next(Self::waiting_exit),
                // Emulation errors lead to vCPU exit.
                Err(e) => {
                    error!("vcpu: {}, run_emulation failed: {:?}", self.id, e);
                    return StateMachine::next(Self::waiting_exit);
                }
            }
        }

        // By default don't change state.
        let mut state = StateMachine::next(Self::running);

        // Break this emulation loop on any transition request/external event.
        match self.event_receiver.try_recv() {
            // Running ---- Exit ----> Exited
            Ok(VcpuEvent::Exit) => {
                // Move to 'exited' state.
                state = StateMachine::next(Self::exited);
            }
            // Running ---- Pause ----> Paused
            Ok(VcpuEvent::Pause) => {
                // Nothing special to do.
                self.response_sender
                    .send(VcpuResponse::Paused)
                    .expect("failed to send pause status");

                // TODO: we should call `KVM_KVMCLOCK_CTRL` here to make sure
                // TODO continued: the guest soft lockup watchdog does not panic on Resume.
                //let _ = self.fd.kvmclock_ctrl();

                // Move to 'paused' state.
                state = StateMachine::next(Self::paused);
            }
            Ok(VcpuEvent::Resume) => {
                self.response_sender
                    .send(VcpuResponse::Resumed)
                    .expect("failed to send resume status");
            }
            Ok(VcpuEvent::Gettid) => {
                self.response_sender
                    .send(VcpuResponse::Tid(self.cpu_index(), Vcpu::gettid()))
                    .expect("failed to send vcpu thread tid");
            }
            Ok(VcpuEvent::RevalidateCache) => {
                self.revalidate_cache()
                    .map(|()| {
                        self.response_sender
                            .send(VcpuResponse::CacheRevalidated)
                            .expect("failed to revalidate vcpu IoManager cache");
                    })
                    .map_err(|e| self.response_sender.send(VcpuResponse::Error(e)))
                    .expect("failed to revalidate vcpu IoManager cache");
            }
            // Unhandled exit of the other end.
            Err(TryRecvError::Disconnected) => {
                // Move to 'exited' state.
                state = StateMachine::next(Self::exited);
            }
            // All other events or lack thereof have no effect on current 'running' state.
            Err(TryRecvError::Empty) => (),
        }

        state
    }

    // This is the main loop of the `Paused` state.
    fn paused(&mut self) -> StateMachine<Self> {
        match self.event_receiver.recv() {
            // Paused ---- Exit ----> Exited
            Ok(VcpuEvent::Exit) => {
                // Move to 'exited' state.
                StateMachine::next(Self::exited)
            }
            // Paused ---- Resume ----> Running
            Ok(VcpuEvent::Resume) => {
                self.response_sender
                    .send(VcpuResponse::Resumed)
                    .expect("failed to send resume status");
                // Move to 'running' state.
                StateMachine::next(Self::running)
            }
            Ok(VcpuEvent::Pause) => {
                self.response_sender
                    .send(VcpuResponse::Paused)
                    .expect("failed to send pause status");
                // continue 'pause' state.
                StateMachine::next(Self::paused)
            }
            Ok(VcpuEvent::Gettid) => {
                self.response_sender
                    .send(VcpuResponse::Tid(self.cpu_index(), Vcpu::gettid()))
                    .expect("failed to send vcpu thread tid");
                StateMachine::next(Self::paused)
            }
            Ok(VcpuEvent::RevalidateCache) => {
                self.revalidate_cache()
                    .map(|()| {
                        self.response_sender
                            .send(VcpuResponse::CacheRevalidated)
                            .expect("failed to revalidate vcpu IoManager cache");
                    })
                    .map_err(|e| self.response_sender.send(VcpuResponse::Error(e)))
                    .expect("failed to revalidate vcpu IoManager cache");

                StateMachine::next(Self::paused)
            }
            // Unhandled exit of the other end.
            Err(_) => {
                // Move to 'exited' state.
                StateMachine::next(Self::exited)
            }
        }
    }

    // This is the main loop of the `WaitingExit` state.
    fn waiting_exit(&mut self) -> StateMachine<Self> {
        // trigger vmm to stop machine
        if let Err(e) = self.exit_evt.write(1) {
            self.metrics.failures.inc();
            error!("Failed signaling vcpu exit event: {}", e);
        }

        let mut state = StateMachine::next(Self::waiting_exit);

        match self.event_receiver.recv() {
            Ok(VcpuEvent::Exit) => state = StateMachine::next(Self::exited),
            Ok(_) => error!(
                "wrong state received in waiting exit state on vcpu {}",
                self.id
            ),
            Err(_) => {
                error!(
                    "vcpu channel closed in waiting exit state on vcpu {}",
                    self.id
                );
                state = StateMachine::next(Self::exited);
            }
        }

        state
    }

    // This is the main loop of the `Exited` state.
    fn exited(&mut self) -> StateMachine<Self> {
        // State machine reached its end.
        StateMachine::finish(Self::exited)
    }

    /// Get vcpu file descriptor.
    pub fn vcpu_fd(&self) -> &VcpuFd {
        self.fd.as_ref()
    }

    pub fn metrics(&self) -> Arc<VcpuMetrics> {
        self.metrics.clone()
    }
}

impl Drop for Vcpu {
    fn drop(&mut self) {
        let _ = self.reset_thread_local_data();
        let id: u32 = self.id as u32;
        METRICS.write().unwrap().vcpu.remove(&id);
    }
}

#[cfg(test)]
pub mod tests {
    use std::sync::mpsc::{channel, Receiver};
    use std::sync::Mutex;

    use arc_swap::ArcSwap;
    use dbs_device::device_manager::IoManager;
    use lazy_static::lazy_static;
    use test_utils::skip_if_not_root;

    use super::*;
    use crate::kvm_context::KvmContext;

    pub enum EmulationCase {
        IoIn,
        IoOut,
        MmioRead,
        MmioWrite,
        Hlt,
        Shutdown,
        FailEntry(u64, u32),
        InternalError,
        Unknown,
        SystemEvent(u32, u64),
        Error(i32),
    }

    lazy_static! {
        pub static ref EMULATE_RES: Mutex<EmulationCase> = Mutex::new(EmulationCase::Unknown);
    }

    impl Vcpu {
        pub fn emulate(_fd: &VcpuFd) -> std::result::Result<VcpuExit<'_>, kvm_ioctls::Error> {
            let res = &*EMULATE_RES.lock().unwrap();
            match res {
                EmulationCase::IoIn => Ok(VcpuExit::IoIn(0, &mut [])),
                EmulationCase::IoOut => Ok(VcpuExit::IoOut(0, &[])),
                EmulationCase::MmioRead => Ok(VcpuExit::MmioRead(0, &mut [])),
                EmulationCase::MmioWrite => Ok(VcpuExit::MmioWrite(0, &[])),
                EmulationCase::Hlt => Ok(VcpuExit::Hlt),
                EmulationCase::Shutdown => Ok(VcpuExit::Shutdown),
                EmulationCase::FailEntry(error_type, cpu_num) => {
                    Ok(VcpuExit::FailEntry(*error_type, *cpu_num))
                }
                EmulationCase::InternalError => Ok(VcpuExit::InternalError),
                EmulationCase::Unknown => Ok(VcpuExit::Unknown),
                EmulationCase::SystemEvent(event_type, event_flags) => {
                    Ok(VcpuExit::SystemEvent(*event_type, *event_flags))
                }
                EmulationCase::Error(e) => Err(kvm_ioctls::Error::new(*e)),
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn create_vcpu() -> (Vcpu, Receiver<VcpuStateEvent>) {
        let kvm_context = KvmContext::new(None).unwrap();
        let vm = kvm_context.kvm().create_vm().unwrap();
        let vcpu_fd = Arc::new(vm.create_vcpu(0).unwrap());
        let io_manager = IoManagerCached::new(Arc::new(ArcSwap::new(Arc::new(IoManager::new()))));
        let supported_cpuid = kvm_context
            .supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)
            .unwrap();
        let reset_event_fd = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let vcpu_state_event = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let (tx, rx) = channel();
        let time_stamp = TimestampUs::default();

        let vcpu = Vcpu::new_x86_64(
            0,
            vcpu_fd,
            io_manager,
            supported_cpuid,
            reset_event_fd,
            vcpu_state_event,
            tx,
            time_stamp,
            false,
        )
        .unwrap();

        (vcpu, rx)
    }

    #[cfg(target_arch = "aarch64")]
    fn create_vcpu() -> (Vcpu, Receiver<VcpuStateEvent>) {
        use kvm_ioctls::Kvm;
        use std::os::fd::AsRawFd;
        // Call for kvm too frequently would cause error in some host kernel.
        std::thread::sleep(std::time::Duration::from_millis(5));

        let kvm = Kvm::new().unwrap();
        let vm = Arc::new(kvm.create_vm().unwrap());
        let _kvm_context = KvmContext::new(Some(kvm.as_raw_fd())).unwrap();
        let vcpu_fd = Arc::new(vm.create_vcpu(0).unwrap());
        let io_manager = IoManagerCached::new(Arc::new(ArcSwap::new(Arc::new(IoManager::new()))));
        let reset_event_fd = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let vcpu_state_event = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let (tx, rx) = channel();
        let time_stamp = TimestampUs::default();

        let vcpu = Vcpu::new_aarch64(
            0,
            vcpu_fd,
            io_manager,
            reset_event_fd,
            vcpu_state_event,
            tx,
            time_stamp,
            false,
        )
        .unwrap();

        (vcpu, rx)
    }

    #[test]
    fn test_vcpu_run_emulation() {
        skip_if_not_root!();

        let (mut vcpu, _) = create_vcpu();

        #[cfg(target_arch = "x86_64")]
        {
            // Io in
            *(EMULATE_RES.lock().unwrap()) = EmulationCase::IoIn;
            let res = vcpu.run_emulation();
            assert!(matches!(res, Ok(VcpuEmulation::Handled)));

            // Io out
            *(EMULATE_RES.lock().unwrap()) = EmulationCase::IoOut;
            let res = vcpu.run_emulation();
            assert!(matches!(res, Ok(VcpuEmulation::Handled)));
        }

        // Mmio read
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::MmioRead;
        let res = vcpu.run_emulation();
        assert!(matches!(res, Ok(VcpuEmulation::Handled)));

        // Mmio write
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::MmioWrite;
        let res = vcpu.run_emulation();
        assert!(matches!(res, Ok(VcpuEmulation::Handled)));

        // KVM_EXIT_HLT signal
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::Hlt;
        let res = vcpu.run_emulation();
        assert!(matches!(res, Err(VcpuError::VcpuUnhandledKvmExit)));

        // KVM_EXIT_SHUTDOWN signal
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::Shutdown;
        let res = vcpu.run_emulation();
        assert!(matches!(res, Err(VcpuError::VcpuUnhandledKvmExit)));

        // KVM_EXIT_FAIL_ENTRY signal
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::FailEntry(0, 0);
        let res = vcpu.run_emulation();
        assert!(matches!(res, Err(VcpuError::VcpuUnhandledKvmExit)));

        // KVM_EXIT_INTERNAL_ERROR signal
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::InternalError;
        let res = vcpu.run_emulation();
        assert!(matches!(res, Err(VcpuError::VcpuUnhandledKvmExit)));

        // KVM_SYSTEM_EVENT_RESET
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::SystemEvent(KVM_SYSTEM_EVENT_RESET, 0);
        let res = vcpu.run_emulation();
        assert!(matches!(res, Ok(VcpuEmulation::Stopped)));

        // KVM_SYSTEM_EVENT_SHUTDOWN
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::SystemEvent(KVM_SYSTEM_EVENT_SHUTDOWN, 0);
        let res = vcpu.run_emulation();
        assert!(matches!(res, Ok(VcpuEmulation::Stopped)));

        // Other system event
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::SystemEvent(0, 0);
        let res = vcpu.run_emulation();
        assert!(matches!(res, Err(VcpuError::VcpuUnhandledKvmExit)));

        // Unknown exit reason
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::Unknown;
        let res = vcpu.run_emulation();
        assert!(matches!(res, Err(VcpuError::VcpuUnhandledKvmExit)));

        // Error: EAGAIN
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::Error(libc::EAGAIN);
        let res = vcpu.run_emulation();
        assert!(matches!(res, Ok(VcpuEmulation::Handled)));

        // Error: EINTR
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::Error(libc::EINTR);
        let res = vcpu.run_emulation();
        assert!(matches!(res, Ok(VcpuEmulation::Interrupted)));

        // other error
        *(EMULATE_RES.lock().unwrap()) = EmulationCase::Error(libc::EINVAL);
        let res = vcpu.run_emulation();
        assert!(matches!(res, Err(VcpuError::VcpuUnhandledKvmExit)));
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_vcpu_check_io_port_info() {
        skip_if_not_root!();

        let (vcpu, _receiver) = create_vcpu();

        // debug info signal
        let res = vcpu
            .check_io_port_info(MAGIC_IOPORT_DEBUG_INFO, &[0, 0, 0, 0])
            .unwrap();
        assert!(res);
    }
}
