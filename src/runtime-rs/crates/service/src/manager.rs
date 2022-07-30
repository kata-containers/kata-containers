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
    protobuf::{well_known_types::Any, Message as ProtobufMessage},
    shim_async,
};
use runtimes::RuntimeHandlerManager;
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    sync::mpsc::{channel, Receiver},
};
use ttrpc::asynchronous::Server;

use crate::task_service::TaskService;
/// message buffer size
const MESSAGE_BUFFER_SIZE: usize = 8;
use persist::KATA_PATH;

pub struct ServiceManager {
    receiver: Option<Receiver<Message>>,
    handler: Arc<RuntimeHandlerManager>,
    task_server: Option<Server>,
    binary: String,
    address: String,
    namespace: String,
}

async fn send_event(
    containerd_binary: String,
    address: String,
    namespace: String,
    event: Arc<dyn Event>,
) -> Result<()> {
    let any = Any {
        type_url: event.type_url(),
        value: event.value().context("get event value")?,
        ..Default::default()
    };
    let data = any.write_to_bytes().context("write to any")?;
    let mut child = Command::new(containerd_binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(&[
            "--address",
            &address,
            "publish",
            "--topic",
            &event.r#type(),
            "--namespace",
            &namespace,
        ])
        .spawn()
        .context("sawn cmd")?;

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

impl ServiceManager {
    pub async fn new(
        id: &str,
        containerd_binary: &str,
        address: &str,
        namespace: &str,
        task_server_fd: RawFd,
    ) -> Result<Self> {
        let (sender, receiver) = channel::<Message>(MESSAGE_BUFFER_SIZE);
        let handler = Arc::new(
            RuntimeHandlerManager::new(id, sender)
                .await
                .context("new runtime handler")?,
        );
        let mut task_server = unsafe { Server::from_raw_fd(task_server_fd) };
        task_server = task_server.set_domain_unix();
        Ok(Self {
            receiver: Some(receiver),
            handler,
            task_server: Some(task_server),
            binary: containerd_binary.to_string(),
            address: address.to_string(),
            namespace: namespace.to_string(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!(sl!(), "begin to run service");
        self.start().await.context("start")?;

        info!(sl!(), "wait server message");
        let mut rx = self.receiver.take();
        if let Some(rx) = rx.as_mut() {
            while let Some(r) = rx.recv().await {
                info!(sl!(), "receive action {:?}", &r.action);
                let result = match r.action {
                    Action::Start => self.start().await.context("start listen"),
                    Action::Stop => self.stop_listen().await.context("stop listen"),
                    Action::Shutdown => {
                        self.stop_listen().await.context("stop listen")?;
                        break;
                    }
                    Action::Event(event) => {
                        info!(sl!(), "get event {:?}", &event);
                        send_event(
                            self.binary.clone(),
                            self.address.clone(),
                            self.namespace.clone(),
                            event,
                        )
                        .await
                        .context("send event")?;
                        Ok(())
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
        let handler = RuntimeHandlerManager::new(sid, sender)
            .await
            .context("new runtime handler")?;
        handler.cleanup().await?;
        let temp_dir = [KATA_PATH, sid].join("/");
        if std::fs::metadata(temp_dir.as_str()).is_ok() {
            // try to remove dir and skip the result
            fs::remove_dir_all(temp_dir)
                .map_err(|err| {
                    warn!(sl!(), "failed to clean up sandbox tmp dir");
                    err
                })
                .ok();
        }
        Ok(())
    }

    async fn start(&mut self) -> Result<()> {
        let task_service = Arc::new(Box::new(TaskService::new(self.handler.clone()))
            as Box<dyn shim_async::Task + Send + Sync>);
        let task_server = self.task_server.take();
        let task_server = match task_server {
            Some(t) => {
                let mut t = t.register_service(shim_async::create_task(task_service));
                t.start().await.context("task server start")?;
                Some(t)
            }
            None => None,
        };
        self.task_server = task_server;
        Ok(())
    }

    async fn stop_listen(&mut self) -> Result<()> {
        let task_server = self.task_server.take();
        let task_server = match task_server {
            Some(mut t) => {
                t.stop_listen().await;
                Some(t)
            }
            None => None,
        };
        self.task_server = task_server;
        Ok(())
    }
}
