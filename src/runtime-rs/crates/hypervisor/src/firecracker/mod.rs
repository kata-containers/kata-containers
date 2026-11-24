//Copyright (c) 2019-2022 Alibaba Cloud
//Copyright (c) 2023 Nubificus Ltd
//
//SPDX-License-Identifier: Apache-2.0

mod fc_api;
mod inner;
mod inner_device;
mod inner_hypervisor;

use super::HypervisorState;
use crate::MemoryConfig;
use crate::{device::DeviceType, Hypervisor, HypervisorConfig, VcpuThreadIds};
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use inner::FcInner;
use kata_types::capabilities::Capabilities;
use kata_types::capabilities::CapabilityBits;
use persist::sandbox_persist::Persist;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct Firecracker {
    inner: Arc<RwLock<FcInner>>,
    exit_waiter: Mutex<(mpsc::Receiver<()>, i32)>,
}

// Convenience function to set the scope.
pub fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "firecracker"))
}

impl Default for Firecracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Firecracker {
    pub fn new() -> Self {
        let (exit_notify, exit_waiter) = mpsc::channel(1);

        Self {
            inner: Arc::new(RwLock::new(FcInner::new(exit_notify))),
            exit_waiter: Mutex::new((exit_waiter, 0)),
        }
    }

    pub async fn set_hypervisor_config(&self, config: HypervisorConfig) {
        let mut inner = self.inner.write().await;
        inner.set_hypervisor_config(config)
    }
}

#[async_trait]
impl Hypervisor for Firecracker {
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
        let mut inner = self.inner.write().await;
        inner.stop_vm().await
    }

    async fn wait_vm(&self) -> Result<i32> {
        debug!(sl(), "Wait fc sandbox");
        let mut waiter = self.exit_waiter.lock().await;

        //wait until the fc process exited.
        waiter.0.recv().await;

        let inner = self.inner.read().await;
        if let Ok(exit_code) = inner.wait_vm().await {
            waiter.1 = exit_code;
        }

        Ok(waiter.1)
    }

    async fn pause_vm(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.pause_vm()
    }

    async fn resume_vm(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.resume_vm()
    }

    async fn add_device(&self, device: DeviceType) -> Result<DeviceType> {
        let mut inner = self.inner.write().await;
        match inner.add_device(device.clone()).await {
            Ok(_) => Ok(device),
            Err(err) => Err(err),
        }
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
        let inner = self.inner.read().await;
        inner.get_thread_ids().await
    }

    async fn cleanup(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.cleanup().await
    }

    async fn resize_vcpu(&self, old_vcpu: u32, new_vcpu: u32) -> Result<(u32, u32)> {
        let inner = self.inner.read().await;
        inner.resize_vcpu(old_vcpu, new_vcpu).await
    }

    async fn get_pids(&self) -> Result<Vec<u32>> {
        let inner = self.inner.read().await;
        inner.get_pids().await
    }

    async fn get_vmm_master_tid(&self) -> Result<u32> {
        let inner = self.inner.read().await;
        inner.get_vmm_master_tid().await
    }

    async fn get_ns_path(&self) -> Result<String> {
        let inner = self.inner.read().await;
        inner.get_ns_path().await
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
        inner.guest_memory_block_size_mb()
    }

    async fn resize_memory(&self, new_mem_mb: u32) -> Result<(u32, MemoryConfig)> {
        let mut inner = self.inner.write().await;
        inner.resize_memory(new_mem_mb)
    }

    async fn get_passfd_listener_addr(&self) -> Result<(String, u32)> {
        Err(anyhow::anyhow!("Not yet supported"))
    }
}
#[async_trait]
impl Persist for Firecracker {
    type State = HypervisorState;
    type ConstructorArgs = ();
    /// Save a state of the component.
    async fn save(&self) -> Result<Self::State> {
        let inner = self.inner.read().await;
        inner.save().await.context("save hypervisor state")
    }
    /// Restore a component from a specified state.
    async fn restore(
        _hypervisor_args: Self::ConstructorArgs,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        let (exit_notify, exit_waiter) = mpsc::channel(1);
        let inner = FcInner::restore(exit_notify, hypervisor_state).await?;

        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
            exit_waiter: Mutex::new((exit_waiter, 0)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{
        device_manager::{do_handle_device, DeviceManager},
        DeviceConfig,
    };
    use crate::{BlockConfigModern, VmmState, VIRTIO_BLOCK_MMIO};
    use bytes::Bytes;
    use http_body_util::{BodyExt, Empty};
    use hyper::{server::conn::http1, service::service_fn, Response, StatusCode};
    use hyper_util::rt::TokioIo;
    use std::convert::Infallible;

    #[rstest::rstest]
    #[case(false)]
    #[case(true)]
    #[tokio::test]
    async fn shared_block_device_releases_slot_on_final_detach(#[case] is_readonly: bool) {
        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("fc.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let (requests, mut received) = mpsc::unbounded_channel();
        let server = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let requests = requests.clone();
                http1::Builder::new()
                    .keep_alive(false)
                    .serve_connection(
                        TokioIo::new(stream),
                        service_fn(move |request: hyper::Request<hyper::body::Incoming>| {
                            let requests = requests.clone();
                            async move {
                                assert_eq!(request.method(), hyper::Method::PATCH);
                                let body = request.into_body().collect().await.unwrap().to_bytes();
                                requests
                                    .send(
                                        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                                    )
                                    .unwrap();
                                Ok::<_, Infallible>(
                                    Response::builder()
                                        .status(StatusCode::NO_CONTENT)
                                        .body(Empty::<Bytes>::new())
                                        .unwrap(),
                                )
                            }
                        }),
                    )
                    .await
                    .unwrap();
            }
        });

        let fc = Arc::new(Firecracker::new());
        {
            let mut inner = fc.inner.write().await;
            inner.state = VmmState::VmRunning;
            inner.asock_path = socket.to_string_lossy().into_owned();
        }
        let manager = RwLock::new(DeviceManager::new(fc.clone(), None).await.unwrap());
        let config = |path: &str| {
            DeviceConfig::BlockCfgModern(BlockConfigModern {
                path_on_host: path.into(),
                is_readonly,
                driver_option: VIRTIO_BLOCK_MMIO.into(),
                ..Default::default()
            })
        };
        // Reserve logical index zero for the guest OS disk, as sandbox startup does.
        do_handle_device(&manager, &config("guest-root"))
            .await
            .unwrap();
        let DeviceType::BlockModern(shared) = do_handle_device(&manager, &config("shared-volume"))
            .await
            .unwrap()
        else {
            panic!("expected a block device");
        };
        let first_request = received.recv().await.unwrap();
        do_handle_device(&manager, &config("shared-volume"))
            .await
            .unwrap();
        let (id, index) = {
            let block = shared.lock().await;
            (block.device_id.clone(), block.config.index)
        };

        manager.write().await.try_remove_device(&id).await.unwrap();
        assert_eq!(shared.lock().await.attach_count, 1);
        // A new user must find the still-attached device without consuming another slot.
        let DeviceType::BlockModern(reused) = do_handle_device(&manager, &config("shared-volume"))
            .await
            .unwrap()
        else {
            panic!("expected a block device");
        };
        assert!(Arc::ptr_eq(&shared, &reused));
        assert_eq!(shared.lock().await.attach_count, 2);
        assert!(received.try_recv().is_err());

        manager.write().await.try_remove_device(&id).await.unwrap();
        manager.write().await.try_remove_device(&id).await.unwrap();
        assert_eq!(shared.lock().await.attach_count, 0);
        // Check before the logical index is reused: a stale mapping would also
        // let the next attachment select the same physical slot.
        {
            let inner = fc.inner.read().await;
            assert!(!inner.readonly_drive_slots.contains_key(&index));
            assert!(!inner.writable_drive_slots.contains_key(&index));
        }

        let DeviceType::BlockModern(next) = do_handle_device(&manager, &config("next-volume"))
            .await
            .unwrap()
        else {
            panic!("expected a block device");
        };
        let next_request = received.recv().await.unwrap();
        assert_eq!(next_request["drive_id"], first_request["drive_id"]);
        assert_eq!(next_request["path_on_host"], "next-volume");
        assert_eq!(
            next.lock().await.config.virt_path,
            shared.lock().await.config.virt_path
        );
        server.abort();
    }
}
