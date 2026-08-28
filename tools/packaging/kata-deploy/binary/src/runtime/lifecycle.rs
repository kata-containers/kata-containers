// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::k8s;
use crate::utils;

use super::manager;
use anyhow::Result;
use log::info;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

pub async fn wait_till_node_is_ready_timeout(
    config: &Config,
    timeout_secs: Option<u64>,
) -> Result<()> {
    let start = std::time::Instant::now();
    let mut check_count = 0;
    loop {
        check_count += 1;
        let ready = k8s::get_node_ready_status(config)
            .await
            .unwrap_or_else(|e| {
                info!(
                    "wait_till_node_is_ready: Error getting node status (attempt {}): {}",
                    check_count, e
                );
                "False".to_string()
            });

        info!(
            "wait_till_node_is_ready: Node {} ready status = '{}' (attempt {})",
            config.node_name, ready, check_count
        );

        if ready == "True" {
            info!("Node {} is ready", config.node_name);
            return Ok(());
        }

        if let Some(timeout) = timeout_secs {
            if start.elapsed().as_secs() >= timeout {
                return Err(anyhow::anyhow!(
                    "Timed out after {}s waiting for node {} to become ready",
                    timeout,
                    config.node_name
                ));
            }
        }

        info!("wait_till_node_is_ready: Node not ready yet, sleeping 2 seconds...");
        sleep(Duration::from_secs(2)).await;
    }
}

/// Wait until the CRI runtime's systemd unit is active again, up to `timeout_secs`.
///
/// Scoped to what this stage is answerable for: the runtime it bounced serving
/// again. Whether the node as a whole is Ready is checked by the stage that
/// labels it kata-capable.
pub async fn wait_till_cri_unit_active(runtime: &str, timeout_secs: u64) -> Result<()> {
    let unit = manager::cri_systemd_unit(runtime);
    let start = std::time::Instant::now();
    let mut attempt = 0;

    loop {
        attempt += 1;
        if utils::host_systemctl(&["is-active", "--quiet", &unit])
            .await
            .is_ok()
        {
            info!("Unit {unit} is active again (attempt {attempt})");
            return Ok(());
        }

        if start.elapsed().as_secs() >= timeout_secs {
            return Err(anyhow::anyhow!(
                "Timed out after {timeout_secs}s waiting for {unit} to become active again"
            ));
        }

        info!("wait_till_cri_unit_active: {unit} not active yet, sleeping 2 seconds...");
        sleep(Duration::from_secs(2)).await;
    }
}

/// What the node says about the kata handlers the CRI runtime is serving.
#[derive(Debug, PartialEq, Eq)]
pub enum HandlerReport {
    AllLoaded,
    Missing(Vec<String>),
    /// The node does not report handlers, or could not be read. Never a
    /// failure: this check exists to catch a runtime that ignored our
    /// configuration, not to make installs depend on being able to ask.
    Unknown,
}

/// Ask the node which kata handlers its CRI runtime is serving, waiting up to
/// `timeout_secs` for `expected` to show up.
///
/// The only view we have of what the runtime actually *loaded*, as opposed to
/// what we wrote and hoped it would read. It trails the runtime by a node status
/// sync or two, hence the wait - but a cluster that cannot answer at all says so
/// immediately, rather than spending the timeout on it.
pub async fn kata_handlers_loaded(
    config: &Config,
    expected: &[String],
    timeout_secs: u64,
) -> HandlerReport {
    if expected.is_empty() {
        return HandlerReport::Unknown;
    }

    // A best-effort check is not worth the full timeout to give up on.
    const READ_FAILURES_BEFORE_GIVING_UP: u32 = 3;

    let start = std::time::Instant::now();
    let mut last_missing: Option<Vec<String>> = None;
    let mut failures = 0;

    loop {
        match k8s::get_node_runtime_handlers(config).await {
            Ok(Some(loaded)) => {
                let missing = missing_handlers(expected, &loaded);

                if missing.is_empty() {
                    return HandlerReport::AllLoaded;
                }

                info!(
                    "kata_handlers_loaded: node {} reports {loaded:?}; still waiting for {missing:?}",
                    config.node_name
                );
                failures = 0;
                last_missing = Some(missing);
            }
            Ok(None) => {
                info!(
                    "kata_handlers_loaded: node {} reports no runtime handlers, so what the \
                     runtime loaded cannot be checked here",
                    config.node_name
                );
                return HandlerReport::Unknown;
            }
            Err(e) => {
                info!("kata_handlers_loaded: could not read the node's handlers: {e}");
                failures += 1;
                if failures >= READ_FAILURES_BEFORE_GIVING_UP {
                    return HandlerReport::Unknown;
                }
            }
        }

        if start.elapsed().as_secs() >= timeout_secs {
            return match last_missing {
                Some(missing) => HandlerReport::Missing(missing),
                None => HandlerReport::Unknown,
            };
        }

        sleep(Duration::from_secs(2)).await;
    }
}

/// Which of `expected` the runtime is not serving.
///
/// Only ever looks for ours: a runtime is free to serve handlers we know
/// nothing about.
fn missing_handlers(expected: &[String], loaded: &[String]) -> Vec<String> {
    expected
        .iter()
        .filter(|handler| !loaded.contains(handler))
        .cloned()
        .collect()
}

/// Whether the CRI runtime has been running continuously since `written_at`, and
/// is therefore serving the configuration that was on disk at that moment.
///
/// Finding the configuration a job-mode retry would have written proves it was
/// written, not that anything restarted to read it: an attempt that died in
/// between leaves the two indistinguishable on disk. Only systemd can tell them
/// apart, having recorded the restart regardless of who asked for it.
///
/// Anything that cannot be established is answered `false`, costing a restart
/// that may turn out to be unnecessary. The opposite mistake labels a node
/// kata-capable while its runtime knows nothing about kata.
pub async fn cri_serving_config_from(runtime: &str, written_at: Option<SystemTime>) -> bool {
    // k0s reloads without a restart, so there is none for a retry to have missed.
    if matches!(runtime, "k0s-worker" | "k0s-controller") {
        return true;
    }

    let Some(written_at) = written_at else {
        info!("Could not tell when the CRI config was written; assuming a restart is needed");
        return false;
    };

    let unit = manager::cri_systemd_unit(runtime);
    let active_since = match utils::host_unit_active_since(&unit).await {
        Ok(Some(active_since)) => active_since,
        Ok(None) => {
            info!("{unit} has not been active since boot; a restart is still needed");
            return false;
        }
        Err(e) => {
            info!("Could not tell when {unit} last started ({e}); assuming a restart is needed");
            return false;
        }
    };

    if active_since <= written_at {
        info!(
            "{unit} started at {}, not after the CRI config was written at {}; a restart is still \
             needed",
            epoch_secs(active_since),
            epoch_secs(written_at)
        );
        return false;
    }

    true
}

/// Timestamps are only ever reported to explain a restart decision, so seconds
/// since the epoch is enough and needs no date formatting dependency.
fn epoch_secs(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Restart the CRI runtime, then wait for it to come back.
///
/// `staged` selects how that wait is done: the DaemonSet survives the bounce and
/// can wait for the node's Ready condition, while a staged per-node Job is torn
/// down along with the runtime it restarts and waits on the systemd unit instead.
pub async fn restart_runtime(config: &Config, runtime: &str, staged: bool) -> Result<()> {
    info!("restart_runtime: Starting restart for runtime={}", runtime);
    match runtime {
        "k0s-worker" | "k0s-controller" => {
            // k0s automatically loads config on the fly
            info!("k0s runtime - no restart needed");
        }
        _ => {
            let unit = manager::cri_systemd_unit(runtime);
            info!("restart_runtime: Running daemon-reload");
            utils::host_systemctl(&["daemon-reload"]).await?;
            info!("restart_runtime: Restarting {}", unit);
            utils::host_systemctl(&["restart", &unit]).await?;
            info!("restart_runtime: Successfully restarted {}", unit);
        }
    }

    if staged {
        // k0s never restarted anything above, so there is nothing to wait for.
        if !matches!(runtime, "k0s-worker" | "k0s-controller") {
            info!("restart_runtime: Waiting for the CRI runtime unit to come back");
            wait_till_cri_unit_active(runtime, 300).await?;
        }
        return Ok(());
    }

    info!("restart_runtime: Waiting for node to become ready");
    wait_till_node_is_ready_timeout(config, Some(300)).await?;
    info!("restart_runtime: Node is ready");
    Ok(())
}

pub async fn restart_cri_runtime(_config: &Config, runtime: &str) -> Result<()> {
    match runtime {
        "k0s-worker" | "k0s-controller" => {
            // k0s automatically unloads config on the fly
            info!("k0s runtime - no restart needed");
        }
        _ => {
            utils::host_systemctl(&["daemon-reload"]).await?;
            utils::host_systemctl(&["restart", &manager::cri_systemd_unit(runtime)]).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handlers(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn a_runtime_serving_everything_we_wrote_is_missing_nothing() {
        assert!(missing_handlers(
            &handlers(&["kata-qemu", "kata-clh"]),
            &handlers(&["runc", "kata-qemu", "kata-clh"]),
        )
        .is_empty());
    }

    #[test]
    fn a_runtime_that_never_read_our_config_is_missing_all_of_them() {
        assert_eq!(
            missing_handlers(&handlers(&["kata-qemu", "kata-clh"]), &handlers(&["runc"])),
            handlers(&["kata-qemu", "kata-clh"]),
        );
    }

    #[test]
    fn another_installs_handlers_do_not_count_as_ours() {
        // A multi-install alongside us serves kata handlers under its own suffix.
        assert_eq!(
            missing_handlers(
                &handlers(&["kata-qemu"]),
                &handlers(&["kata-qemu-tee", "kata-qemu-debug"]),
            ),
            handlers(&["kata-qemu"]),
        );
    }
}
