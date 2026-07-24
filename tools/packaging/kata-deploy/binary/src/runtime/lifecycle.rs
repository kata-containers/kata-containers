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

    active_since > written_at
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
