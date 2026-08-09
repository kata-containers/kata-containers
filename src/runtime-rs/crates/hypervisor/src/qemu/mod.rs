// Copyright (c) 2022 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

mod cmdline_generator;
mod inner;
mod qmp;

use crate::device::pci_path::PciPath;
use crate::device::DeviceType;
use crate::hypervisor_persist::HypervisorState;
use crate::{Hypervisor, MemoryConfig};
use crate::{HypervisorConfig, VcpuThreadIds};
use inner::QemuInner;
use kata_types::capabilities::{Capabilities, CapabilityBits};
use persist::sandbox_persist::Persist;

use anyhow::{Context, Result};
use async_trait::async_trait;

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::process::Child;
use tokio::sync::RwLock;
use tokio::sync::{mpsc, watch, Mutex};

#[derive(Debug)]
pub struct Qemu {
    inner: Arc<RwLock<QemuInner>>,
    exit_waiter: Mutex<(mpsc::Receiver<()>, i32)>,
    /// Exit code published by the sole OS-thread reaper (fix10).
    /// `None` until QEMU has been reaped.
    reaped_exit: watch::Sender<Option<i32>>,
}

impl Default for Qemu {
    fn default() -> Self {
        Self::new()
    }
}

impl Qemu {
    pub fn new() -> Self {
        let (exit_notify, exit_waiter) = mpsc::channel(1);
        let (reaped_exit, _) = watch::channel(None);

        Self {
            inner: Arc::new(RwLock::new(QemuInner::new(exit_notify))),
            exit_waiter: Mutex::new((exit_waiter, 0)),
            reaped_exit,
        }
    }

    pub async fn set_hypervisor_config(&self, config: HypervisorConfig) {
        let mut inner = self.inner.write().await;
        inner.set_hypervisor_config(config)
    }

    /// Reap QEMU on a dedicated OS thread using sync `try_wait()`.
    ///
    /// fix10 (kata-containers#13564): a tokio task awaiting `Child::wait()` can
    /// be cancelled when containerd SIGKILLs / abandons the shim after a
    /// non-blocking Shutdown — Dropping the Child without a successful wait
    /// leaves QEMU as a zombie under the (still-alive) orphaned shim. An OS
    /// thread is not cancelled with the tokio task; `try_wait` reaps on Unix.
    fn spawn_os_reaper(child: Child, reaped_exit: watch::Sender<Option<i32>>) {
        let pid = child.id();
        let _ = thread::Builder::new()
            .name("qemu-reap".into())
            .spawn(move || {
                let mut child = child;
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            let code = status.code().unwrap_or(0);
                            info!(
                                sl!(),
                                "fix10 OS reaper: qemu pid={:?} exited code={}", pid, code
                            );
                            let _ = reaped_exit.send(Some(code));
                            return;
                        }
                        Ok(None) => thread::sleep(Duration::from_millis(200)),
                        Err(e) => {
                            // ECHILD if a racing reaper already collected the status.
                            warn!(
                                sl!(),
                                "fix10 OS reaper: try_wait pid={:?} err={:?}", pid, e
                            );
                            let _ = reaped_exit.send(Some(0));
                            return;
                        }
                    }
                }
            });
    }

    async fn wait_for_reaped_exit(&self) -> i32 {
        let mut rx = self.reaped_exit.subscribe();
        loop {
            if let Some(code) = *rx.borrow_and_update() {
                return code;
            }
            if rx.changed().await.is_err() {
                return 0;
            }
        }
    }
}

#[async_trait]
impl Hypervisor for Qemu {
    async fn prepare_vm(
        &self,
        id: &str,
        netns: Option<String>,
        _annotations: &HashMap<String, String>,
        selinux_label: Option<String>,
    ) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.prepare_vm(id, netns, selinux_label).await
    }

    async fn start_vm(&self, timeout: i32) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.start_vm(timeout).await
    }

    async fn stop_vm(&self) -> Result<()> {
        // fix8: signal under write lock only (start_kill), never hold locks
        // across the multi-minute TDX reclaim wait.
        // fix9: do NOT await wait_vm() here either — that still blocked the
        // Shutdown RPC long enough for containerd to SIGKILL the shim, leaving
        // a live orphan QEMU under init (production B200+TDX, ~1.2 TiB guest).
        // PR_SET_PDEATHSIG kills QEMU if the shim process exits/dies first.
        //
        // fix10: take the Child here and reap on an OS thread (try_wait loop).
        // The old tokio::spawn(inner.wait_vm()) raced the start_vm waiter for
        // Child ownership; the loser logged "the process has been reaped" and
        // the winner's Child::wait() was abandoned when the shim task was
        // cancelled → zombie QEMU under an orphaned live shim (glm B200).
        let (kill_result, child) = {
            let mut inner = self.inner.write().await;
            let kill_result = inner.stop_vm().await;
            let child = if kill_result.is_ok() {
                inner.take_qemu_child().await
            } else {
                None
            };
            (kill_result, child)
        };
        if let Some(child) = child {
            info!(
                sl!(),
                "fix10: stop_vm took qemu Child pid={:?}; spawning OS reaper",
                child.id()
            );
            Self::spawn_os_reaper(child, self.reaped_exit.clone());
        } else if kill_result.is_ok() {
            info!(
                sl!(),
                "fix10: stop_vm Child already taken; start_vm waiter / prior reaper owns wait"
            );
        }
        kill_result
    }

    async fn wait_vm(&self) -> Result<i32> {
        info!(sl!(), "Wait QEMU VM");

        let mut waiter = self.exit_waiter.lock().await;

        // Wait until qemu stderr EOF (process exited / closed stderr).
        waiter.0.recv().await;

        // Already reaped by stop_vm's OS thread?
        if let Some(code) = *self.reaped_exit.borrow() {
            waiter.1 = code;
            return Ok(code);
        }

        // Take Child for OS-thread reaping (same fix10 path as stop_vm).
        // Do NOT Child::wait().await on this cancellable task.
        let child = {
            let inner = self.inner.read().await;
            inner.take_qemu_child().await
        };
        if let Some(child) = child {
            info!(
                sl!(),
                "fix10: wait_vm took qemu Child pid={:?}; spawning OS reaper",
                child.id()
            );
            Self::spawn_os_reaper(child, self.reaped_exit.clone());
        }

        let code = self.wait_for_reaped_exit().await;
        waiter.1 = code;
        Ok(code)
    }

    async fn pause_vm(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.pause_vm()
    }

    async fn resume_vm(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.resume_vm()
    }

    async fn save_vm(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.save_vm().await
    }

    async fn add_device(&self, device: DeviceType) -> Result<DeviceType> {
        let mut inner = self.inner.write().await;
        inner.add_device(device).await
    }

    async fn remove_device(&self, device: DeviceType) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.remove_device(device).await
    }

    async fn update_device(&self, device: DeviceType) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.update_device(device).await
    }

    async fn get_agent_socket(&self) -> Result<String> {
        let inner = self.inner.read().await;
        inner.get_agent_socket().await
    }

    async fn disconnect(&self) {
        let mut inner = self.inner.write().await;
        inner.disconnect().await
    }

    async fn hypervisor_config(&self) -> HypervisorConfig {
        let inner = self.inner.read().await;
        inner.hypervisor_config()
    }

    async fn get_thread_ids(&self) -> Result<VcpuThreadIds> {
        let mut inner = self.inner.write().await;
        inner.get_thread_ids().await
    }

    async fn get_vmm_master_tid(&self) -> Result<u32> {
        let inner = self.inner.read().await;
        inner.get_vmm_master_tid().await
    }

    async fn get_ns_path(&self) -> Result<String> {
        let inner = self.inner.read().await;
        inner.get_ns_path().await
    }

    async fn cleanup(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.cleanup().await
    }

    async fn resize_vcpu(&self, old_vcpus: u32, new_vcpus: u32) -> Result<(u32, u32)> {
        let mut inner = self.inner.write().await;
        inner.resize_vcpu(old_vcpus, new_vcpus).await
    }

    async fn get_pids(&self) -> Result<Vec<u32>> {
        let inner = self.inner.read().await;
        inner.get_pids().await
    }

    async fn check(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.check().await
    }

    async fn get_jailer_root(&self) -> Result<String> {
        let inner = self.inner.read().await;
        inner.get_jailer_root().await
    }

    async fn save_state(&self) -> Result<HypervisorState> {
        self.save().await
    }

    async fn capabilities(&self) -> Result<Capabilities> {
        let inner = self.inner.read().await;
        inner.capabilities().await
    }

    async fn get_hypervisor_metrics(&self) -> Result<String> {
        let inner = self.inner.read().await;
        inner.get_hypervisor_metrics().await
    }

    async fn set_capabilities(&self, flag: CapabilityBits) {
        let mut inner = self.inner.write().await;
        inner.set_capabilities(flag)
    }

    async fn set_guest_memory_block_size(&self, size: u32) {
        let mut inner = self.inner.write().await;
        inner.set_guest_memory_block_size(size);
    }

    async fn guest_memory_block_size(&self) -> u32 {
        let inner = self.inner.read().await;
        inner.guest_memory_block_size()
    }

    async fn resize_memory(&self, new_mem_mb: u32) -> Result<(u32, MemoryConfig)> {
        let mut inner = self.inner.write().await;
        inner.resize_memory(new_mem_mb)
    }

    async fn get_passfd_listener_addr(&self) -> Result<(String, u32)> {
        Err(anyhow::anyhow!("Not yet supported"))
    }

    async fn resolve_vfio_device_pci_path(&self, hostdev_id: &str) -> Result<PciPath> {
        self.inner
            .write()
            .await
            .resolve_vfio_device_pci_path(hostdev_id)
    }
}

#[async_trait]
impl Persist for Qemu {
    type State = HypervisorState;
    type ConstructorArgs = ();

    /// Save a state of the component.
    async fn save(&self) -> Result<Self::State> {
        let inner = self.inner.read().await;
        inner.save().await.context("save qemu hypervisor state")
    }

    /// Restore a component from a specified state.
    async fn restore(
        _hypervisor_args: Self::ConstructorArgs,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        let (exit_notify, exit_waiter) = mpsc::channel(1);
        let (reaped_exit, _) = watch::channel(None);

        let inner = QemuInner::restore(exit_notify, hypervisor_state).await?;
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
            exit_waiter: Mutex::new((exit_waiter, 0)),
            reaped_exit,
        })
    }
}
