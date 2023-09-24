// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs,
    os::unix::io::{FromRawFd, RawFd},
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context, Result};
use common::message::{Action, Event, Message};
use containerd_shim_protos::{
    protobuf::{well_known_types::any::Any, Message as ProtobufMessage},
    sandbox_async, shim_async,
};
use kata_types::config::KATA_PATH;
use runtimes::RuntimeHandlerManager;
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    sync::mpsc::{channel, Receiver},
};
use ttrpc::asynchronous::Server;

use crate::sandbox_service::SandboxService;
use crate::task_service::TaskService;

/// message buffer size
const MESSAGE_BUFFER_SIZE: usize = 8;

pub struct ServiceManager {
    receiver: Option<Receiver<Message>>,
    handler: Arc<RuntimeHandlerManager>,
    server: Option<Server>,
    binary: String,
    address: String,
    namespace: String,
}

impl std::fmt::Debug for ServiceManager {
    // todo: some how to implement debug for handler
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceManager")
            .field("receiver", &self.receiver)
            .field("server.is_some()", &self.server.is_some())
            .field("binary", &self.binary)
            .field("address", &self.address)
            .field("namespace", &self.namespace)
            .finish()
    }
}

impl ServiceManager {
    // TODO: who manages lifecycle for `task_server_fd`?
    pub async fn new(
        id: &str,
        containerd_binary: &str,
        address: &str,
        namespace: &str,
        server_fd: RawFd,
    ) -> Result<Self> {
        let (sender, receiver) = channel::<Message>(MESSAGE_BUFFER_SIZE);
        let rt_mgr = RuntimeHandlerManager::new(id, sender).context("new runtime handler")?;
        let handler = Arc::new(rt_mgr);
        let mut server = unsafe { Server::from_raw_fd(server_fd) };
        server = server.set_domain_unix();
        Ok(Self {
            receiver: Some(receiver),
            handler,
            server: Some(server),
            binary: containerd_binary.to_string(),
            address: address.to_string(),
            namespace: namespace.to_string(),
        })
    }

    pub async fn run(mut self) -> Result<()> {
        info!(sl!(), "begin to run service");
        self.registry_service().context("registry service")?;
        self.start_service().await.context("start service")?;

        info!(sl!(), "wait server message");
        let mut rx = self.receiver.take();
        if let Some(rx) = rx.as_mut() {
            while let Some(r) = rx.recv().await {
                info!(sl!(), "receive action {:?}", &r.action);
                let result = match r.action {
                    Action::Start => self.start_service().await.context("start listen"),
                    Action::Stop => self.stop_service().await.context("stop listen"),
                    Action::Shutdown => {
                        self.stop_service().await.context("stop listen")?;
                        break;
                    }
                    Action::Event(event) => {
                        info!(sl!(), "get event {:?}", &event);
                        self.send_event(event).await.context("send event")
                    }
                };

                if let Some(ref sender) = r.resp_sender {
                    if let Err(err) = result.as_ref() {
                        error!(sl!(), "failed to process action {:?}", err);
                    }
                    sender.send(result).await.context("send response")?;
                }
            }
        }

        info!(sl!(), "end to run service");

        Ok(())
    }

    pub async fn cleanup(sid: &str) -> Result<()> {
        let (sender, _receiver) = channel::<Message>(MESSAGE_BUFFER_SIZE);
        let handler = RuntimeHandlerManager::new(sid, sender).context("new runtime handler")?;
        if let Err(e) = handler.cleanup().await {
            warn!(sl!(), "failed to clean up runtime state, {}", e);
        }

        let temp_dir = [KATA_PATH, sid].join("/");
        if fs::metadata(temp_dir.as_str()).is_ok() {
            // try to remove dir and skip the result
            if let Err(e) = fs::remove_dir_all(temp_dir) {
                warn!(sl!(), "failed to clean up sandbox tmp dir, {}", e);
            }
        }

        Ok(())
    }

    fn registry_service(&mut self) -> Result<()> {
        if let Some(t) = self.server.take() {
            let sandbox_service = Arc::new(Box::new(SandboxService::new(self.handler.clone()))
                as Box<dyn sandbox_async::Sandbox + Send + Sync>);
            let t = t.register_service(sandbox_async::create_sandbox(sandbox_service));

            let task_service = Arc::new(Box::new(TaskService::new(self.handler.clone()))
                as Box<dyn shim_async::Task + Send + Sync>);
            let t = t.register_service(shim_async::create_task(task_service));

            self.server = Some(t);
        }
        Ok(())
    }

    async fn start_service(&mut self) -> Result<()> {
        if let Some(t) = self.server.as_mut() {
            t.start().await.context("task server start")?;
        }
        Ok(())
    }

    async fn stop_service(&mut self) -> Result<()> {
        if let Some(t) = self.server.as_mut() {
            t.stop_listen().await;
        }
        Ok(())
    }

    async fn send_event(&self, event: Arc<dyn Event>) -> Result<()> {
        let any = Any {
            type_url: event.type_url(),
            value: event.value().context("get event value")?,
            ..Default::default()
        };
        let data = any.write_to_bytes().context("write to any")?;
        let mut child = Command::new(&self.binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args([
                "--address",
                &self.address,
                "publish",
                "--topic",
                &event.r#type(),
                "--namespace",
                &self.namespace,
            ])
            .spawn()
            .context("spawn containerd cmd to publish event")?;

        let stdin = child.stdin.as_mut().context("failed to open stdin")?;
        stdin
            .write_all(&data)
            .await
            .context("failed to write to stdin")?;
        let output = child
            .wait_with_output()
            .await
            .context("failed to read stdout")?;
        info!(sl!(), "get output: {:?}", output);
        Ok(())
    }
}
