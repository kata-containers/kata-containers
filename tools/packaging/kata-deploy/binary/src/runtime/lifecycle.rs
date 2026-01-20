// Copyright (c) 2019 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::k8s;
use crate::utils;
use anyhow::Result;
use log::info;
use std::time::Duration;
use tokio::time::sleep;

pub async fn wait_till_node_is_ready(config: &Config) -> Result<()> {
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

        info!("wait_till_node_is_ready: Node not ready yet, sleeping 2 seconds...");
        sleep(Duration::from_secs(2)).await;
    }
}

pub async fn restart_runtime(config: &Config, runtime: &str) -> Result<()> {
    info!("restart_runtime: Starting restart for runtime={}", runtime);
    match runtime {
        "k0s-worker" | "k0s-controller" => {
            // k0s automatically reloads containerd config when drop-ins change
            info!("k0s runtime - no restart needed");
        }
        "microk8s" => {
            info!("restart_runtime: Restarting microk8s containerd service");
            utils::host_systemctl(&["restart", "snap.microk8s.daemon-containerd.service"])?;
            info!("restart_runtime: Successfully restarted microk8s containerd");
        }
        _ => {
            info!("restart_runtime: Running daemon-reload");
            utils::host_systemctl(&["daemon-reload"])?;
            info!("restart_runtime: Restarting {} service", runtime);
            utils::host_systemctl(&["restart", runtime])?;
            info!(
                "restart_runtime: Successfully restarted {} service",
                runtime
            );
        }
    }

    info!("restart_runtime: Waiting for node to become ready");
    wait_till_node_is_ready(config).await?;
    info!("restart_runtime: Node is ready");
    Ok(())
}

pub async fn restart_cri_runtime(_config: &Config, runtime: &str) -> Result<()> {
    match runtime {
        "k0s-worker" | "k0s-controller" => {
            // k0s automatically reloads containerd config when drop-ins change
            info!("k0s runtime - no restart needed");
        }
        "microk8s" => {
            utils::host_systemctl(&["restart", "snap.microk8s.daemon-containerd.service"])?;
        }
        _ => {
            utils::host_systemctl(&["daemon-reload"])?;
            utils::host_systemctl(&["restart", runtime])?;
        }
    }

    Ok(())
}
