// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use crate::container_manager::agent_disconnected;

use agent::Agent;
use anyhow::Context;
use tokio::sync::{mpsc, Mutex};

/// monitor check interval 30s
const HEALTH_CHECK_TIMER_INTERVAL: u64 = 30;

/// version check threshold 5min
const VERSION_CHECK_THRESHOLD: u64 = 5 * 60 / HEALTH_CHECK_TIMER_INTERVAL;

/// health check stop channel buffer size
const HEALTH_CHECK_STOP_CHANNEL_BUFFER_SIZE: usize = 1;

async fn check_agent(
    agent: &dyn Agent,
    budget: Duration,
) -> anyhow::Result<agent::HealthCheckResponse> {
    tokio::time::timeout(budget, async {
        let check = || agent.check(agent::CheckRequest::new(""));
        match check().await {
            Err(error) if agent_disconnected(&error) => check().await,
            result => result,
        }
    })
    .await
    .context("agent health check timed out")?
}

pub struct HealthCheck {
    pub keep_alive: bool,
    keep_abnormal: bool,
    stop_tx: mpsc::Sender<()>,
    stop_rx: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl HealthCheck {
    pub fn new(keep_alive: bool, keep_abnormal: bool) -> HealthCheck {
        let (tx, rx) = mpsc::channel(HEALTH_CHECK_STOP_CHANNEL_BUFFER_SIZE);
        HealthCheck {
            keep_alive,
            keep_abnormal,
            stop_tx: tx,
            stop_rx: Arc::new(Mutex::new(rx)),
        }
    }

    pub fn start<F, Fut>(&self, id: &str, agent: Arc<dyn Agent>, on_failure: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        if !self.keep_alive {
            return;
        }
        let id = id.to_string();

        info!(sl!(), "start runtime keep alive");

        let stop_rx = self.stop_rx.clone();
        let keep_abnormal = self.keep_abnormal;
        tokio::spawn(async move {
            let configured = agent.agent_config().await.health_check_request_timeout_ms;
            let budget = Duration::from_millis(u64::from(if configured == 0 {
                kata_types::config::Agent::default().health_check_request_timeout_ms
            } else {
                configured
            }));
            let mut version_check_threshold_count = 0;

            loop {
                tokio::time::sleep(std::time::Duration::from_secs(HEALTH_CHECK_TIMER_INTERVAL))
                    .await;
                let mut stop_rx = stop_rx.lock().await;
                match stop_rx.try_recv() {
                    Ok(_) => {
                        info!(sl!(), "revive stop {} monitor signal", id);
                        break;
                    }

                    Err(mpsc::error::TryRecvError::Empty) => {
                        // check agent
                        match tokio::select! {
                            biased;
                            _ = stop_rx.recv() => break,
                            result = check_agent(agent.as_ref(), budget) => result,
                        } {
                            Ok(_) => {
                                debug!(sl!(), "check {} agent health successfully", id);
                                version_check_threshold_count += 1;
                                if version_check_threshold_count >= VERSION_CHECK_THRESHOLD {
                                    // need to check version
                                    version_check_threshold_count = 0;
                                    if let Ok(v) = agent
                                        .version(agent::CheckRequest::new(""))
                                        .await
                                        .context("check version")
                                    {
                                        info!(sl!(), "agent {}", v.agent_version)
                                    }
                                }
                                continue;
                            }
                            Err(e) => {
                                error!(sl!(), "failed to do {} agent health check: {}", id, e);
                                if let Err(mpsc::error::TryRecvError::Empty) = stop_rx.try_recv() {
                                    error!(sl!(), "failed to receive stop monitor signal");
                                    if !keep_abnormal {
                                        // Exiting the shim here would leave an external VMM running.
                                        warn!(sl!(), "stopping the VM of sandbox {}", id);
                                        if !on_failure().await {
                                            error!(sl!(), "VM exit was not confirmed, exiting");
                                            ::std::process::exit(1);
                                        }
                                        break;
                                    }
                                } else {
                                    info!(sl!(), "wait to exit {}", id);
                                    break;
                                }
                            }
                        }
                    }

                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        warn!(sl!(), "{} monitor channel has broken", id);
                        break;
                    }
                }
            }
        });
    }

    pub async fn stop(&self) {
        if !self.keep_alive {
            return;
        }
        info!(sl!(), "stop runtime keep alive");
        match self.stop_tx.try_send(()) {
            Ok(()) => {}
            // Both teardown paths signal this, and a blocking send on the
            // single-slot channel would stall the second one.
            Err(mpsc::error::TrySendError::Full(_)) => {
                debug!(sl!(), "monitor stop already signalled");
            }
            Err(e) => {
                warn!(sl!(), "failed send monitor channel. {:?}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent::{kata::KataAgent, AgentManager};
    use tokio::net::UnixListener;
    use ttrpc::proto::{Codec, GenMessage, MessageHeader};

    #[rstest::rstest]
    #[case::recovers(Some(ttrpc::Code::OK))]
    #[case::rpc_failure(Some(ttrpc::Code::PERMISSION_DENIED))]
    #[case::deadline(None)]
    #[tokio::test]
    async fn probe_after_disconnect(#[case] reply: Option<ttrpc::Code>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("agent.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let agent = KataAgent::new(Default::default());
        agent
            .start(&format!("remote://{}", path.display()))
            .await
            .unwrap();
        agent.stop().await;
        let probe = check_agent(&agent, Duration::from_secs(2));
        let serve = async {
            let (mut socket, _) = listener.accept().await.unwrap();
            let first = GenMessage::read_from(&mut socket).await.unwrap();
            assert_eq!(
                ttrpc::Request::decode(first.payload).unwrap().method,
                "Check"
            );
            drop(socket);
            let (mut socket, _) = listener.accept().await.unwrap();
            let second = GenMessage::read_from(&mut socket).await.unwrap();
            assert_eq!(
                ttrpc::Request::decode(second.payload).unwrap().method,
                "Check"
            );
            if let Some(code) = reply {
                let mut response = ttrpc::Response::default();
                response.set_status(ttrpc::get_status(code, ""));
                let payload = response.encode().unwrap();
                GenMessage {
                    header: MessageHeader::new_response(
                        second.header.stream_id,
                        payload.len() as u32,
                    ),
                    payload,
                }
                .write_to(&mut socket)
                .await
                .unwrap();
            }
            socket
        };
        let (result, _socket) =
            tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(probe, serve) })
                .await
                .unwrap();
        match reply {
            Some(ttrpc::Code::OK) => assert!(result.is_ok()),
            Some(code) => assert!(
                matches!(result.err().unwrap().downcast_ref::<ttrpc::Error>(),
                Some(ttrpc::Error::RpcStatus(status)) if status.code() == code)
            ),
            None => assert!(result.err().unwrap().is::<tokio::time::error::Elapsed>()),
        }
    }
}
