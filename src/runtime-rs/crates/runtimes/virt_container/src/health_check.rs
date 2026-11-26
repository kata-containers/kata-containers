// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use agent::Agent;
use anyhow::Context;
use tokio::sync::{mpsc, Mutex};

/// monitor check interval 30s
const HEALTH_CHECK_TIMER_INTERVAL: u64 = 30;

/// version check threshold 5min
const VERSION_CHECK_THRESHOLD: u64 = 5 * 60 / HEALTH_CHECK_TIMER_INTERVAL;

/// health check stop channel buffer size
const HEALTH_CHECK_STOP_CHANNEL_BUFFER_SIZE: usize = 1;

/// Controls whether Guest Agent health checks can run.
///
/// Health checks are paused while the Agent is intentionally disconnected,
/// such as during checkpoint or restore. Results from checks started before
/// the disconnect are ignored.
#[derive(Clone)]
struct ProbeGate {
    epoch: Arc<AtomicU64>,
}

/// Keeps Guest Agent health checks paused while this guard exists.
///
/// Dropping the guard automatically allows health checks to run again.
pub struct ProbeSuppressGuard {
    gate: ProbeGate,
}

impl ProbeGate {
    fn new() -> Self {
        Self {
            epoch: Arc::new(AtomicU64::new(0)),
        }
    }

    fn begin_probe(&self) -> Option<u64> {
        let epoch = self.epoch.load(Ordering::Acquire);
        (epoch & 1 == 0).then_some(epoch)
    }

    fn is_current(&self, epoch: u64) -> bool {
        self.epoch.load(Ordering::Acquire) == epoch
    }

    fn suppress(&self) -> ProbeSuppressGuard {
        self.epoch.fetch_add(1, Ordering::AcqRel);
        ProbeSuppressGuard { gate: self.clone() }
    }
}

impl Drop for ProbeSuppressGuard {
    fn drop(&mut self) {
        self.gate.epoch.fetch_add(1, Ordering::Release);
    }
}

pub struct HealthCheck {
    pub keep_alive: bool,
    keep_abnormal: bool,
    stop_tx: mpsc::Sender<()>,
    stop_rx: Arc<Mutex<mpsc::Receiver<()>>>,
    probe_gate: ProbeGate,
}

impl HealthCheck {
    pub fn new(keep_alive: bool, keep_abnormal: bool) -> HealthCheck {
        let (tx, rx) = mpsc::channel(HEALTH_CHECK_STOP_CHANNEL_BUFFER_SIZE);
        HealthCheck {
            keep_alive,
            keep_abnormal,
            stop_tx: tx,
            stop_rx: Arc::new(Mutex::new(rx)),
            probe_gate: ProbeGate::new(),
        }
    }

    pub fn suppress_for_disconnect(&self) -> ProbeSuppressGuard {
        self.probe_gate.suppress()
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
        let probe_gate = self.probe_gate.clone();
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
                        let Some(started_epoch) = probe_gate.begin_probe() else {
                            continue;
                        };
                        // check agent
                        let result = agent
                            .check(agent::CheckRequest::new(""))
                            .await
                            .context("check health");
                        // The disconnect may have completed before this probe
                        // returns. Discard its result before health/version work.
                        if !probe_gate.is_current(started_epoch) {
                            continue;
                        }
                        match result {
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

    #[test]
    fn suppression_invalidates_inflight_probe_after_disconnect() {
        let gate = ProbeGate::new();
        let started_epoch = gate.begin_probe().expect("probe should be enabled");
        assert!(gate.is_current(started_epoch));

        let probe_suppression = gate.suppress();
        assert!(gate.begin_probe().is_none());
        assert!(!gate.is_current(started_epoch));
        drop(probe_suppression);

        assert!(!gate.is_current(started_epoch));
        let resumed_epoch = gate.begin_probe().expect("probe should be enabled");
        assert!(gate.is_current(resumed_epoch));
    }
}
