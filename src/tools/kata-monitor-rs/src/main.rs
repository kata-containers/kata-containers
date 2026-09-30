// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

mod config;
mod server;
mod collectos;
mod cache;
mod metrics;
mod client;
mod watcher;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;
use tracing::{info, error};
use crate::config::{CliArgs, RuntimeConfig};
use crate::server::AppState;
use crate::collectos::MetricsCollector;
use crate::cache::SandboxCache;
use crate::metrics::MonitorMetrics;
use crate::watcher::{SandboxEvent, SandboxWatcher};

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&args.log_level)),
        )
        .init();

    let runtime_config = RuntimeConfig::new();
    info!(
        sandbox_path = %runtime_config.sandbox_path,
        "runtime-rs mode"
    );

    let sandbox_cache = SandboxCache::new();
    let monitor_metrics = Arc::new(MonitorMetrics::new());
    let metrics_collector = Arc::new(MetricsCollector::new(
        runtime_config.clone(),
        sandbox_cache.clone(),
        Duration::from_secs(3),
    ));

    // Start sandbox filesystem watcher as fsinotify does
    let (event_tx, mut event_rx) = mpsc::channel(256);
    let watcher_path = runtime_config.sandbox_path.to_string();
    tokio::spawn(async move {
        let watcher = SandboxWatcher::new(&watcher_path, event_tx);
        if let Err(e) = watcher.start().await {
            error!(error = %e, path = %watcher_path, "sandbox watcher failed");
        }
    });

    // Event processing loop
    let cache_for_events = sandbox_cache.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                SandboxEvent::Created(id) => {
                    cache_for_events.insert(id.clone()).await;
                    info!(sandbox_id = %id, "sandbox created");
                }
                SandboxEvent::Removed(id) => {
                    cache_for_events.remove(&id).await;
                    info!(sandbox_id = %id, "sandbox removed");
                }
            }
        }
    });

    // Start HTTP server with graceful shutdown
    let app_state = Arc::new(AppState {
        sandbox_cache,
        metrics_collector,
        monitor_metrics,
        runtime_config,
    });

    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl+c handler");
        info!("received shutdown signal");
    };

    server::start_server(&args.listen_address, app_state, shutdown).await?;

    Ok(())
}
