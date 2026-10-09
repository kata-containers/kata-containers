// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use agent::Agent;
use anyhow::Context;
use tokio::sync::{mpsc, Mutex};

/// monitor check interval 30s
const HEALTH_CHECK_TIMER_INTERVAL: u64 = 30;

/// A blocked in-process VMM can occupy runtime workers indefinitely.
/// Allow all stop fallbacks to finish before forcing the shim to exit.
const FAILURE_STOP_TIMEOUT: Duration = Duration::from_secs(60);

/// version check threshold 5min
const VERSION_CHECK_THRESHOLD: u64 = 5 * 60 / HEALTH_CHECK_TIMER_INTERVAL;

/// health check stop channel buffer size
const HEALTH_CHECK_STOP_CHANNEL_BUFFER_SIZE: usize = 1;

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
                        match agent
                            .check(agent::CheckRequest::new(""))
                            .await
                            .context("check health")
                        {
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
                                        let (done_tx, done_rx) = std::sync::mpsc::channel();
                                        // A blocked stop can occupy runtime workers, so use a separate thread.
                                        std::thread::spawn(move || {
                                            if done_rx.recv_timeout(FAILURE_STOP_TIMEOUT).is_err() {
                                                error!(sl!(), "VM stop did not finish, exiting");
                                                ::std::process::exit(1);
                                            }
                                        });
                                        if !on_failure().await {
                                            error!(sl!(), "VM exit was not confirmed, exiting");
                                            ::std::process::exit(1);
                                        }
                                        let _ = done_tx.send(());
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
