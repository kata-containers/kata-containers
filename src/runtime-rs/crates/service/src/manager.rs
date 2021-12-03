// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    os::unix::io::{FromRawFd, RawFd},
    sync::Arc,
};

use anyhow::{Context, Result};
use common::message::{Action, Message};
use containerd_shim_protos::shim_async;
use runtimes::RuntimeHandlerManager;
use tokio::sync::mpsc::{channel, Receiver};
use ttrpc::asynchronous::Server;

use crate::task_service::TaskService;

/// message buffer size
const MESSAGE_BUFFER_SIZE: usize = 8;

pub struct ServiceManager {
    receiver: Option<Receiver<Message>>,
    handler: Arc<RuntimeHandlerManager>,
    task_server: Option<Server>,
}

impl ServiceManager {
    pub async fn new(id: &str, task_server_fd: RawFd) -> Result<Self> {
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
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!(sl!(), "begin to run service");

        self.start().await.context("start")?;
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
                };

                if let Some(ref sender) = r.resp_sender {
                    sender.send(result).await.context("send response")?;
                }
            }
        }

        info!(sl!(), "end to run service");

        Ok(())
    }

    pub fn cleanup(id: &str) -> Result<()> {
        RuntimeHandlerManager::cleanup(id)
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
