// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use super::sandbox_inner::WasmSandboxInner;
use common::{
    message::{Action, Message},
    Sandbox, SandboxNetworkEnv,
};
use shim_interface::KATA_PATH;

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::{collections::HashMap, fs, sync::Arc};
use tokio::sync::{mpsc::Sender, Mutex, RwLock};

use crate::{container_manager::container::container::Container, CONTAINER_BASE};

#[derive(Clone)]
pub struct WasmSandbox {
    pub sid: String,
    msg_sender: Arc<Mutex<Sender<Message>>>,
    pub inner: Arc<RwLock<WasmSandboxInner>>,
    containers: Arc<RwLock<HashMap<String, Container>>>,
}

impl WasmSandbox {
    pub fn new(
        sid: &str,
        msg_sender: Sender<Message>,
        containers: Arc<RwLock<HashMap<String, Container>>>,
    ) -> Result<Self> {
        let logger = sl!().new(o!("subsystem" => "wasm_sandbox"));

        Ok(Self {
            sid: sid.to_string(),
            msg_sender: Arc::new(Mutex::new(msg_sender)),
            inner: Arc::new(RwLock::new(WasmSandboxInner::new(&logger, sid))),
            containers,
        })
    }

    async fn install_signal_handler(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.install_signal_handler(self.containers.clone()).await
    }

    pub async fn update_container_namespaces(&self, spec: &mut oci::Spec) -> Result<()> {
        let inner = self.inner.read().await;
        inner.update_container_namespaces(spec).await
    }
}

#[async_trait]
impl Sandbox for WasmSandbox {
    async fn start(
        &self,
        _dns: Vec<String>,
        spec: &oci::Spec,
        _state: &oci::State,
        _network_env: SandboxNetworkEnv,
    ) -> Result<()> {
        let _ = self.install_signal_handler().await;

        let mut inner = self.inner.write().await;
        inner.start(_dns, spec, _state, _network_env).await
    }

    async fn stop(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.stop().await
    }

    async fn cleanup(&self) -> Result<()> {
        let _ = fs::remove_dir_all([CONTAINER_BASE.as_str(), self.sid.as_str()].join("/"));
        let _ = fs::remove_dir_all([KATA_PATH, &self.sid].join("/"));

        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.stop().await?;

        self.cleanup().await?;

        let msg = Message::new(Action::Shutdown);
        let sender = self.msg_sender.clone();
        let sender = sender.lock().await;
        sender.send(msg).await.context("send shutdown msg")?;

        Ok(())
    }

    // agent function
    async fn agent_sock(&self) -> Result<String> {
        unreachable!()
    }

    // utils
    async fn set_iptables(&self, _is_ipv6: bool, _data: Vec<u8>) -> Result<Vec<u8>> {
        todo!()
    }

    async fn get_iptables(&self, _is_ipv6: bool) -> Result<Vec<u8>> {
        todo!()
    }

    async fn direct_volume_stats(&self, _volume_path: &str) -> Result<String> {
        todo!()
    }

    async fn direct_volume_resize(&self, _resize_req: agent::ResizeVolumeRequest) -> Result<()> {
        todo!()
    }
}
