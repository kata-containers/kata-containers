// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;
use std::time::Duration;

use agent::Agent;
use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

/// monitor check interval 30s
const HEALTH_CHECK_TIMER_INTERVAL: u64 = 30;

/// version check threshold 5min
const VERSION_CHECK_THRESHOLD: u64 = 5 * 60 / HEALTH_CHECK_TIMER_INTERVAL;

pub struct HealthCheck {
    pub keep_alive: bool,
    keep_abnormal: bool,
    // A token rather than a channel: stop() may run more than once, e.g. from
    // concurrent sandbox stops and again from shutdown, and must never block.
    stop_token: CancellationToken,
}

/// Why the health check loop ended.
#[derive(Debug, PartialEq)]
enum MonitorExit {
    /// stop() was called.
    Stopped,
    /// The agent failed a check while the sandbox was not being stopped.
    Unhealthy,
}

/// The agent calls the health check loop makes.
#[async_trait]
trait HealthProbe: Send + Sync {
    async fn check(&self) -> Result<()>;
    async fn version(&self) -> Result<String>;
}

#[async_trait]
impl HealthProbe for Arc<dyn Agent> {
    async fn check(&self) -> Result<()> {
        self.as_ref()
            .check(agent::CheckRequest::new(""))
            .await
            .context("check health")
            .map(|_| ())
    }

    async fn version(&self) -> Result<String> {
        self.as_ref()
            .version(agent::CheckRequest::new(""))
            .await
            .context("check version")
            .map(|v| v.agent_version)
    }
}

impl HealthCheck {
    pub fn new(keep_alive: bool, keep_abnormal: bool) -> HealthCheck {
        HealthCheck {
            keep_alive,
            keep_abnormal,
            stop_token: CancellationToken::new(),
        }
    }

    pub fn start(&self, id: &str, agent: Arc<dyn Agent>) {
        if !self.keep_alive {
            return;
        }
        let id = id.to_string();

        info!(sl!(), "start runtime keep alive");

        let stop_token = self.stop_token.clone();
        let keep_abnormal = self.keep_abnormal;
        tokio::spawn(async move {
            let interval = Duration::from_secs(HEALTH_CHECK_TIMER_INTERVAL);
            if run_monitor(&id, &agent, &stop_token, keep_abnormal, interval).await
                == MonitorExit::Unhealthy
            {
                ::std::process::exit(1);
            }
        });
    }

    /// Stop the health check. Idempotent and non-blocking. Call it before the
    /// VM is stopped: afterwards the agent is unreachable by design, and a
    /// failed check must not be mistaken for an agent failure.
    pub async fn stop(&self) {
        if !self.keep_alive {
            return;
        }
        info!(sl!(), "stop runtime keep alive");
        self.stop_token.cancel();
    }
}

async fn run_monitor(
    id: &str,
    probe: &dyn HealthProbe,
    stop_token: &CancellationToken,
    keep_abnormal: bool,
    interval: Duration,
) -> MonitorExit {
    let mut version_check_threshold_count = 0;

    loop {
        tokio::select! {
            _ = stop_token.cancelled() => {
                info!(sl!(), "revive stop {} monitor signal", id);
                return MonitorExit::Stopped;
            }
            _ = tokio::time::sleep(interval) => {}
        }

        match probe.check().await {
            Ok(_) => {
                debug!(sl!(), "check {} agent health successfully", id);
                version_check_threshold_count += 1;
                if version_check_threshold_count >= VERSION_CHECK_THRESHOLD {
                    // need to check version
                    version_check_threshold_count = 0;
                    if let Ok(v) = probe.version().await {
                        info!(sl!(), "agent {}", v)
                    }
                }
            }
            Err(e) => {
                // A check that was due or in flight when the sandbox began to
                // stop fails once the VM is gone. That is the expected end of
                // the monitor, not an agent failure: exiting here would skip
                // the sandbox cleanup and the reply to the pending request.
                if stop_token.is_cancelled() {
                    info!(sl!(), "wait to exit {}", id);
                    return MonitorExit::Stopped;
                }
                error!(sl!(), "failed to do {} agent health check: {}", id, e);
                error!(sl!(), "failed to receive stop monitor signal");
                if !keep_abnormal {
                    return MonitorExit::Unhealthy;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const INTERVAL: Duration = Duration::from_millis(5);

    /// Fails every check. With a token, the first check stops the monitor
    /// before failing, as when the sandbox stop kills the VM mid-check.
    struct FailingProbe {
        checks: AtomicUsize,
        stop_during_check: Option<CancellationToken>,
    }

    #[async_trait]
    impl HealthProbe for FailingProbe {
        async fn check(&self) -> Result<()> {
            self.checks.fetch_add(1, Ordering::SeqCst);
            if let Some(token) = &self.stop_during_check {
                token.cancel();
            }
            Err(anyhow!("agent unreachable"))
        }

        async fn version(&self) -> Result<String> {
            Ok("test".to_string())
        }
    }

    fn probe(stop_during_check: Option<CancellationToken>) -> FailingProbe {
        FailingProbe {
            checks: AtomicUsize::new(0),
            stop_during_check,
        }
    }

    #[tokio::test]
    async fn test_failure_while_stopping_is_not_unhealthy() {
        let token = CancellationToken::new();
        let probe = probe(Some(token.clone()));
        let exit = run_monitor("sandbox", &probe, &token, false, INTERVAL).await;
        assert_eq!(exit, MonitorExit::Stopped);
        assert_eq!(probe.checks.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_failure_without_stop_is_unhealthy() {
        let token = CancellationToken::new();
        let probe = probe(None);
        let exit = run_monitor("sandbox", &probe, &token, false, INTERVAL).await;
        assert_eq!(exit, MonitorExit::Unhealthy);
        assert_eq!(probe.checks.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_keep_abnormal_keeps_checking_until_stopped() {
        let token = CancellationToken::new();
        let probe = probe(None);
        let stopper = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(INTERVAL * 20).await;
            stopper.cancel();
        });
        let exit = run_monitor("sandbox", &probe, &token, true, INTERVAL).await;
        assert_eq!(exit, MonitorExit::Stopped);
        assert!(probe.checks.load(Ordering::SeqCst) > 1);
    }

    #[tokio::test]
    async fn test_stop_is_idempotent_and_ends_an_idle_monitor() {
        let health_check = HealthCheck::new(true, false);
        health_check.stop().await;
        health_check.stop().await;
        let probe = probe(None);
        let exit = run_monitor(
            "sandbox",
            &probe,
            &health_check.stop_token,
            false,
            Duration::from_secs(3600),
        )
        .await;
        assert_eq!(exit, MonitorExit::Stopped);
        assert_eq!(probe.checks.load(Ordering::SeqCst), 0);
    }
}
