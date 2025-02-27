// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use super::HypervisorState;
use crate::device::DeviceType;
use crate::VmmState;
use anyhow::Result;
use async_trait::async_trait;
use kata_sys_util::protection::GuestProtection;
use kata_types::capabilities::{Capabilities, CapabilityBits};
use kata_types::config::hypervisor::Hypervisor as HypervisorConfig;
use kata_types::config::hypervisor::HYPERVISOR_NAME_CH;
use persist::sandbox_persist::Persist;
use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use tokio::sync::watch::{channel, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::{process::Child, sync::mpsc};

#[derive(Debug)]
pub struct CloudHypervisorInner {
    pub(crate) state: VmmState,
    pub(crate) id: String,

    pub(crate) api_socket: Option<UnixStream>,
    pub(crate) extra_args: Option<Vec<String>>,

    pub(crate) config: HypervisorConfig,

    pub(crate) process: Option<Child>,
    pub(crate) pid: Option<u32>,

    pub(crate) timeout_secs: i32,

    pub(crate) netns: Option<String>,

    // Sandbox-specific directory
    pub(crate) vm_path: String,

    // Hypervisor runtime directory
    pub(crate) run_dir: String,

    // Subdirectory of vm_path.
    pub(crate) jailer_root: String,

    /// List of devices that will be added to the VM once it boots
    pub(crate) pending_devices: Vec<DeviceType>,

    pub(crate) _capabilities: Capabilities,

    pub(crate) shutdown_tx: Option<Sender<bool>>,
    pub(crate) shutdown_rx: Option<Receiver<bool>>,
    pub(crate) tasks: Option<Vec<JoinHandle<Result<()>>>>,

    // Set if the hardware supports creating a protected guest *AND* if the
    // user has requested creating a protected guest.
    //
    // For example, on Intel TDX capable systems with `confidential_guest=true`,
    // this will be set to "tdx".
    pub(crate) guest_protection_to_use: GuestProtection,

    // Store mapping between device-ids created by runtime-rs device manager
    // and device-ids returned by cloud-hypervisor when the device is added to the VM.
    //
    // The cloud-hypervisor device-id is later looked up and used while
    // removing the device.
    pub(crate) device_ids: HashMap<String, String>,

    // List of Cloud Hypervisor features enabled at Cloud Hypervisor build-time.
    //
    // If the version of CH does not provide these details, the value will be
    // None.
    pub(crate) ch_features: Option<Vec<String>>,

    /// Size of memory block of guest OS in MB (currently unused)
    pub(crate) _guest_memory_block_size_mb: u32,

    pub(crate) exit_notify: Option<mpsc::Sender<i32>>,
}

const CH_DEFAULT_TIMEOUT_SECS: u32 = 10;

impl CloudHypervisorInner {
    pub fn new(exit_notify: Option<mpsc::Sender<i32>>) -> Self {
        let mut capabilities = Capabilities::new();
        capabilities.set(
            CapabilityBits::BlockDeviceSupport
                | CapabilityBits::BlockDeviceHotplugSupport
                | CapabilityBits::FsSharingSupport
                | CapabilityBits::HybridVsockSupport,
        );

        let (tx, rx) = channel(true);

        Self {
            api_socket: None,
            extra_args: None,

            process: None,
            pid: None,

            config: Default::default(),
            state: VmmState::NotReady,
            timeout_secs: CH_DEFAULT_TIMEOUT_SECS as i32,
            id: String::default(),
            jailer_root: String::default(),
            vm_path: String::default(),
            run_dir: String::default(),
            netns: None,
            pending_devices: vec![],
            device_ids: HashMap::<String, String>::new(),
            _capabilities: capabilities,
            shutdown_tx: Some(tx),
            shutdown_rx: Some(rx),
            tasks: None,
            guest_protection_to_use: GuestProtection::NoProtection,
            ch_features: None,
            _guest_memory_block_size_mb: 0,

            exit_notify,
        }
    }

    pub fn set_hypervisor_config(&mut self, config: HypervisorConfig) {
        self.config = config;
    }

    pub fn hypervisor_config(&self) -> HypervisorConfig {
        self.config.clone()
    }
}

impl Default for CloudHypervisorInner {
    fn default() -> Self {
        Self::new(None)
    }
}

#[async_trait]
impl Persist for CloudHypervisorInner {
    type State = HypervisorState;
    type ConstructorArgs = mpsc::Sender<i32>;

    // Return a state object that will be saved by the caller.
    async fn save(&self) -> Result<Self::State> {
        Ok(HypervisorState {
            hypervisor_type: HYPERVISOR_NAME_CH.to_string(),
            id: self.id.clone(),
            vm_path: self.vm_path.clone(),
            jailed: false,
            jailer_root: String::default(),
            netns: self.netns.clone(),
            config: self.hypervisor_config(),
            run_dir: self.run_dir.clone(),
            guest_protection_to_use: self.guest_protection_to_use.clone(),

            ..Default::default()
        })
    }

    // Set the hypervisor state to the specified state
    async fn restore(
        exit_notify: mpsc::Sender<i32>,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        let (tx, rx) = channel(true);

        let mut ch = Self {
            config: hypervisor_state.config,
            state: VmmState::NotReady,
            id: hypervisor_state.id,
            vm_path: hypervisor_state.vm_path,
            run_dir: hypervisor_state.run_dir,
            netns: hypervisor_state.netns,
            guest_protection_to_use: hypervisor_state.guest_protection_to_use.clone(),

            pending_devices: vec![],
            device_ids: HashMap::<String, String>::new(),
            tasks: None,
            shutdown_tx: Some(tx),
            shutdown_rx: Some(rx),
            timeout_secs: CH_DEFAULT_TIMEOUT_SECS as i32,
            jailer_root: String::default(),
            ch_features: None,
            exit_notify: Some(exit_notify),

            ..Default::default()
        };
        ch._capabilities = ch.capabilities().await?;

        Ok(ch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_sys_util::protection::TDXDetails;

    #[actix_rt::test]
    async fn test_save_clh() {
        let (exit_notify, _exit_waiter) = mpsc::channel(1);

        let mut clh = CloudHypervisorInner::new(Some(exit_notify.clone()));
        clh.id = String::from("123456");
        clh.netns = Some(String::from("/var/run/netns/testnet"));
        clh.vm_path = String::from("/opt/kata/bin/cloud-hypervisor");
        clh.run_dir = String::from("/var/run/kata-containers/") + &clh.id;

        let details = TDXDetails {
            major_version: 1,
            minor_version: 0,
        };

        clh.guest_protection_to_use = GuestProtection::Tdx(details);

        let state = clh.save().await.unwrap();
        assert_eq!(state.id, clh.id);
        assert_eq!(state.netns, clh.netns);
        assert_eq!(state.vm_path, clh.vm_path);
        assert_eq!(state.run_dir, clh.run_dir);
        assert_eq!(state.guest_protection_to_use, clh.guest_protection_to_use);
        assert!(!state.jailed);
        assert_eq!(state.hypervisor_type, HYPERVISOR_NAME_CH.to_string());

        let clh = CloudHypervisorInner::restore(exit_notify, state.clone())
            .await
            .unwrap();
        assert_eq!(clh.id, state.id);
        assert_eq!(clh.netns, state.netns);
        assert_eq!(clh.vm_path, state.vm_path);
        assert_eq!(clh.run_dir, state.run_dir);
        assert_eq!(clh.guest_protection_to_use, state.guest_protection_to_use);
    }
}
