// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::os::unix::io::RawFd;
use std::sync::Arc;

use anyhow::{Context, Result};
use common::message::{Action, Message};
use containerd_shim_protos::shim_async;
use kata_types::config::KATA_PATH;
use runtimes::RuntimeHandlerManager;
use tokio::sync::mpsc::{channel, Receiver};
use ttrpc::asynchronous::Server;

use crate::event::{new_event_publisher, Forwarder};
use crate::sandbox_service::SandboxService;
use crate::task_service::TaskService;
use containerd_shim_protos::sandbox_async;

/// message buffer size
const MESSAGE_BUFFER_SIZE: usize = 8;

pub struct ServiceManager {
    receiver: Option<Receiver<Message>>,
    handler: Arc<RuntimeHandlerManager>,
    server: Option<Server>,
    binary: String,
    address: String,
    namespace: String,
    event_publisher: Box<dyn Forwarder>,
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
        task_server_fd: RawFd,
    ) -> Result<Self> {
        // Regist service logger for later use.
        logging::register_subsystem_logger("runtimes", "service");

        let (sender, receiver) = channel::<Message>(MESSAGE_BUFFER_SIZE);
        let rt_mgr = RuntimeHandlerManager::new(id, sender).context("new runtime handler")?;
        let handler = Arc::new(rt_mgr);
        // SAFETY: containerd passes a valid unix listener fd when starting the shim.
        let server = unsafe { Server::new().add_unix_listener(task_server_fd)? };
        let event_publisher = new_event_publisher(namespace)
            .await
            .context("new event publisher")?;

        Ok(Self {
            receiver: Some(receiver),
            handler,
            server: Some(server),
            binary: containerd_binary.to_string(),
            address: address.to_string(),
            namespace: namespace.to_string(),
            event_publisher,
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
                if matches!(r.action, Action::Shutdown) {
                    self.stop_service().await.context("stop listen")?;
                    if let Err(err) = self.handle_message(r, true).await {
                        warn!(sl!(), "failed to acknowledge shutdown: {}", err);
                    }
                    let handler = self.handler.clone();
                    handler.finish_tracing(self.drain_requests(rx)).await;
                    break;
                }
                self.handle_message(r, false).await?;
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
        if let Some(s) = self.server.take() {
            let sandbox_service: Arc<dyn sandbox_async::Sandbox + Send + Sync> =
                Arc::new(SandboxService::new(self.handler.clone()));
            let s = s.register_service(sandbox_async::create_sandbox(sandbox_service));

            let task_service: Arc<dyn shim_async::Task + Send + Sync> =
                Arc::new(TaskService::new(self.handler.clone()));
            let s = s.register_service(shim_async::create_task(task_service));
            self.server = Some(s);
        }
        Ok(())
    }

    async fn handle_message(&mut self, message: Message, shutting_down: bool) -> Result<()> {
        let result = match message.action {
            Action::Start if shutting_down => Err(anyhow::anyhow!("service is shutting down")),
            Action::Start => self.start_service().await.context("start listen"),
            Action::Stop => self.stop_service().await.context("stop listen"),
            Action::Shutdown => Ok(()),
            Action::Event(event) => self
                .event_publisher
                .forward(event)
                .await
                .context("forward event"),
        };
        if let Err(err) = &result {
            warn!(sl!(), "failed to process message: {}", err);
        }
        if let Some(sender) = message.resp_sender {
            sender.send(result).await.context("send response")?;
        }
        Ok(())
    }

    async fn drain_requests(&mut self, receiver: &mut Receiver<Message>) {
        let server = self.server.take();
        let disconnect = async {
            if let Some(mut server) = server {
                server.disconnect().await;
            }
        };
        tokio::pin!(disconnect);
        let mut disconnected = false;
        loop {
            let message = tokio::select! {
                biased;
                _ = &mut disconnect, if !disconnected => {
                    disconnected = true;
                    receiver.close();
                    continue;
                }
                message = receiver.recv() => {
                    match message {
                        Some(message) => message,
                        None => {
                            if !disconnected {
                                disconnect.await;
                            }
                            break;
                        }
                    }
                }
            };
            if let Err(err) = self.handle_message(message, true).await {
                warn!(
                    sl!(),
                    "failed to acknowledge message during shutdown: {}", err
                );
            }
        }
    }

    async fn start_service(&mut self) -> Result<()> {
        if let Some(s) = self.server.as_mut() {
            s.start().await.context("task server start")?;
        }
        Ok(())
    }

    async fn stop_service(&mut self) -> Result<()> {
        if let Some(s) = self.server.as_mut() {
            s.stop_listen().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use common::message::Event;
    use containerd_shim_protos::events::task::TaskExit;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{mpsc::Sender, Notify};
    use ttrpc::asynchronous::{Client, MethodHandler, Service, TtrpcContext};

    #[derive(Default)]
    struct CountingForwarder(Arc<AtomicUsize>);

    #[async_trait]
    impl Forwarder for CountingForwarder {
        async fn forward(&self, _event: Arc<dyn Event + Send + Sync>) -> Result<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct EventProducingShutdown {
        sender: Sender<Message>,
        queue_full: Arc<Notify>,
    }

    #[async_trait]
    impl MethodHandler for EventProducingShutdown {
        async fn handler(
            &self,
            _context: TtrpcContext,
            _request: ttrpc::Request,
        ) -> ttrpc::Result<ttrpc::Response> {
            self.sender
                .send(Message::new(Action::Shutdown))
                .await
                .unwrap();
            for index in 0..MESSAGE_BUFFER_SIZE * 2 {
                if index == MESSAGE_BUFFER_SIZE {
                    self.queue_full.notify_one();
                }
                self.sender
                    .send(Message::new(Action::Event(Arc::new(TaskExit::new()))))
                    .await
                    .unwrap();
            }
            for action in [Action::Start, Action::Stop, Action::Shutdown] {
                let reject = matches!(action, Action::Start);
                let (mut response, message) = Message::new_with_receiver(action);
                self.sender.send(message).await.unwrap();
                assert_eq!(response.recv().await.unwrap().is_err(), reject);
            }
            self.sender
                .send(Message::new(Action::Event(Arc::new(TaskExit::new()))))
                .await
                .unwrap();
            Ok(ttrpc::Response {
                payload: b"shutdown completed".to_vec(),
                ..Default::default()
            })
        }
    }

    #[tokio::test]
    async fn test_shutdown_drains_request_events() {
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            let directory = tempfile::tempdir().unwrap();
            let address = format!("unix://{}", directory.path().join("service.sock").display());
            let (sender, mut receiver) = channel(MESSAGE_BUFFER_SIZE);
            let queue_full = Arc::new(Notify::new());
            let method: Box<dyn MethodHandler + Send + Sync> = Box::new(EventProducingShutdown {
                sender: sender.clone(),
                queue_full: queue_full.clone(),
            });
            let mut server = Server::new()
                .bind(&address)
                .unwrap()
                .register_service(HashMap::from([(
                    "test".to_owned(),
                    Service {
                        methods: HashMap::from([("Shutdown".to_owned(), method)]),
                        streams: Default::default(),
                    },
                )]));
            server.start().await.unwrap();
            let client = Client::connect(&address).await.unwrap();
            let response = tokio::spawn(async move {
                client
                    .request(ttrpc::Request {
                        service: "test".to_owned(),
                        method: "Shutdown".to_owned(),
                        ..Default::default()
                    })
                    .await
            });
            assert!(matches!(
                receiver.recv().await.unwrap().action,
                Action::Shutdown
            ));
            queue_full.notified().await;
            assert_eq!(receiver.len(), MESSAGE_BUFFER_SIZE);
            server.stop_listen().await;
            let publisher = CountingForwarder::default();
            let forwarded = publisher.0.clone();
            let mut service = ServiceManager {
                receiver: None,
                handler: Arc::new(RuntimeHandlerManager::new("test", sender.clone()).unwrap()),
                server: Some(server),
                binary: String::new(),
                address,
                namespace: "test".to_owned(),
                event_publisher: Box::new(publisher),
            };
            service.drain_requests(&mut receiver).await;
            assert_eq!(
                response.await.unwrap().unwrap().payload,
                b"shutdown completed"
            );
            assert_eq!(
                forwarded.load(Ordering::SeqCst),
                MESSAGE_BUFFER_SIZE * 2 + 1
            );
            assert!(receiver.is_empty());
            assert!(sender.is_closed());
        })
        .await
        .expect("event backpressure must not prevent shutdown requests from completing");
    }
}
