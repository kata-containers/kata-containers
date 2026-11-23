// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
use std::{sync::Arc, time::SystemTime};

use agent::types::KernelModule;
use anyhow::{Context, Result};
use common::{
    error::is_normal_oom_shutdown_error,
    message::{Action, Message},
    types::SandboxConfig,
    Sandbox,
};
use containerd_shim_protos::events::task::TaskOOM;
use kata_sys_util::spec::load_oci_spec;
use resource::network::{dan_config_path, NetworkConfig, NetworkWithNetNsConfig};

use crate::UNKNOWN_EXIT_STATUS;

use super::{SandboxInner, VirtSandbox};

pub(super) struct StartedVm {
    defer_network: bool,
}

impl VirtSandbox {
    pub(super) async fn prepare_and_start_vm(
        &self,
        inner: &mut SandboxInner,
        sandbox_config: &SandboxConfig,
    ) -> Result<StartedVm> {
        let id = &self.sid;
        let selinux_label = load_oci_spec().ok().and_then(|spec| {
            spec.process()
                .as_ref()
                .and_then(|process| process.selinux_label().clone())
        });

        let prepared = self
            .hypervisor
            .prepare_vm(
                id,
                sandbox_config.network_env.netns.clone(),
                &sandbox_config.annotations,
                selinux_label,
            )
            .await
            .context("prepare vm");

        self.save_after(prepared).await?;

        let defer_network = self.should_defer_network().await?;

        // generate device and setup before start vm
        // should after hypervisor.prepare_vm
        self.setup_network_before_start_vm(sandbox_config).await?;
        let resources = self.prepare_for_start_sandbox(id, sandbox_config).await?;

        let set_up = self
            .resource_manager
            .prepare_before_start_vm(resources)
            .await
            .context("set up device before start vm");
        self.save_after(set_up).await?;

        // start vm
        inner.vmm_start_attempted = true;
        let started = self
            .hypervisor
            .start_vm(hypervisor::VmStartOptions {
                timeout: 10_000,
                restore: None,
            })
            .await
            .context("start vm");

        // A failed start can still have spawned a VMM.  Record its identity
        // before propagating the error so sandbox teardown can confirm exit.
        self.remember_vmm_process().await;
        self.save_after(started).await?;

        info!(sl!(), "start vm");

        let sandbox = self.clone();
        // wait for vm exit in background, and record the exit status and time when vm exited.
        tokio::spawn(async move {
            sandbox
                .finish_vm_exit(sandbox.hypervisor.wait_vm().await)
                .await;
        });
        inner.vmm_exit_watched = true;

        Ok(StartedVm { defer_network })
    }

    pub(super) async fn run_hooks_and_finalize_network(
        &self,
        sandbox_config: &SandboxConfig,
        started: StartedVm,
    ) -> Result<()> {
        let StartedVm { defer_network } = started;
        // execute pre-start hook functions, including Prestart Hooks and CreateRuntime Hooks
        let (prestart_hooks, create_runtime_hooks) =
            if let Some(hooks) = sandbox_config.hooks.as_ref() {
                (
                    hooks.prestart().clone().unwrap_or_default(),
                    hooks.create_runtime().clone().unwrap_or_default(),
                )
            } else {
                (Vec::new(), Vec::new())
            };

        self.execute_oci_hook_functions(
            &prestart_hooks,
            &create_runtime_hooks,
            &sandbox_config.state,
        )
        .await?;

        // 1. if there are pre-start hook functions, network config might have been changed.
        //    We need to rescan the netns to handle the change.
        // 2. Do not scan the netns if we want no network for the VM.
        // QEMU and Cloud Hypervisor advertise network hotplug support, so
        // factory VMs using them defer network setup until after VM startup.
        // Backends without this capability retain pre-start setup.
        let config = self.resource_manager.config().await;
        if self.has_prestart_hooks(&prestart_hooks, &create_runtime_hooks)
            && !defer_network
            && !config.runtime.disable_new_netns
            && !dan_config_path(&config, &self.sid).exists()
        {
            if let Some(netns_path) = &sandbox_config.network_env.netns {
                let network_resource = NetworkConfig::NetNs(NetworkWithNetNsConfig {
                    network_model: config.runtime.internetworking_model.clone(),
                    netns_path: super::recordable_netns_path(&self.sid, netns_path)?,
                    queues: self
                        .hypervisor
                        .hypervisor_config()
                        .await
                        .network_info
                        .network_queues as usize,
                    network_created: sandbox_config.network_env.network_created,
                });
                let created = self
                    .resource_manager
                    .create_network(network_resource)
                    .await
                    .context("create the network");
                self.save_after(created).await?;
                let attached = self
                    .resource_manager
                    .attach_network()
                    .await
                    .context("set up device after start vm");
                self.save_after(attached).await?;
            }
        }

        if defer_network {
            let set_up = self
                .setup_deferred_network_after_start(sandbox_config)
                .await;
            self.save_after(set_up).await?;
        }

        Ok(())
    }

    pub(super) async fn initialize_guest(&self, sandbox_config: &SandboxConfig) -> Result<()> {
        let id = &self.sid;
        // connect agent
        // set agent socket
        let address = self
            .hypervisor
            .get_agent_socket()
            .await
            .context("get agent socket")?;

        self.agent
            .start(&address)
            .await
            .context(format!("connect to address {:?}", address))?;

        let hypervisor_config = self.hypervisor.hypervisor_config().await;
        Self::reseed_rng(self.agent.as_ref(), &hypervisor_config).await?;
        Self::sync_time(self.agent.as_ref(), &hypervisor_config).await?;
        self.set_agent_policy().await.context("set agent policy")?;

        self.resource_manager
            .setup_after_start_vm()
            .await
            .context("setup device after start vm")?;

        // create sandbox in vm
        let agent_config = self.agent.agent_config().await;
        let kernel_modules = KernelModule::set_kernel_modules(agent_config.kernel_modules)?;
        let req = agent::CreateSandboxRequest {
            hostname: sandbox_config.hostname.clone(),
            dns: sandbox_config.dns.clone(),
            storages: self
                .resource_manager
                .get_storage_for_sandbox(self.shm_size)
                .await
                .context("get storages for sandbox")?,
            sandbox_pidns: false,
            sandbox_id: id.to_string(),
            guest_hook_path: self
                .hypervisor
                .hypervisor_config()
                .await
                .security_info
                .guest_hook_path,
            kernel_modules,
        };

        self.agent
            .create_sandbox(req)
            .await
            .context("create sandbox")?;

        Ok(())
    }

    pub(super) async fn start_oom_watcher(&self) {
        let agent = self.agent.clone();
        let sender = self.msg_sender.clone();
        let cancel_token = self.cancel_token.clone();
        let oom_notifier = self.oom_notifier.clone();

        info!(sl!(), "oom watcher start");
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        // Sandbox or VM is shutting down, gracefully exit watcher
                        info!(sl!(), "oom watcher cancelled, sandbox is stopping");
                        break;
                    }
                    res = agent.get_oom_event(agent::Empty::new()) => {
                        match res.context("get oom event") {
                            Ok(resp) => {
                                let cid = &resp.container_id;
                                warn!(sl!(), "send oom event for container {}", &cid);
                                // CRI-O reads a file rather than the event below.
                                oom_notifier.notify(cid).await;
                                let event = TaskOOM {
                                    container_id: cid.to_string(),
                                    ..Default::default()
                                };
                                let msg = Message::new(Action::Event(Arc::new(event)));
                                let lock_sender = sender.lock().await;
                                if let Err(err) = lock_sender.send(msg).await.context("send event") {
                                    error!(
                                        sl!(),
                                        "failed to send oom event for {} error {:?}", cid, err
                                    );
                                }
                            }
                            Err(err) => {
                                // Handle errors by type
                                if is_normal_oom_shutdown_error(&err) {
                                    info!(sl!(), "oom watcher exit on sandbox shutdown: {:?}", err);
                                    break;
                                } else {
                                    warn!(sl!(), "failed to get oom event error {:?}", err);
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn start_health_monitor(&self) {
        let sandbox = self.clone();
        self.monitor
            .start(&self.sid, self.agent.clone(), move || async move {
                // Report the failure now: the stop waits for the VMM to exit,
                // which a VMM stuck in uninterruptible sleep can hold back.
                sandbox
                    .record_stop(UNKNOWN_EXIT_STATUS as u32, SystemTime::now(), false)
                    .await;
                if let Err(e) = sandbox.stop().await {
                    error!(sl!(), "failed to stop sandbox: {:?}", e);
                }
                sandbox.vmm_exit_confirmed.is_cancelled()
            });
    }
}
