// Copyright (c) 2024 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

mod artifacts;
mod config;
mod k8s;
mod runtime;
mod utils;

use anyhow::Result;
use clap::Parser;
use log::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(value_enum)]
    action: Action,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Action {
    Install,
    Cleanup,
    Reset,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let args = Args::parse();

    // Check if running as root (UID 0)
    if unsafe { libc::geteuid() } != 0 {
        return Err(anyhow::anyhow!("This program must be run as root"));
    }

    let config = config::Config::from_env()?;
    let action_str = match args.action {
        Action::Install => "install",
        Action::Cleanup => "cleanup",
        Action::Reset => "reset",
    };
    config.print_info(action_str);

    let runtime = runtime::get_container_runtime(&config).await?;
    info!("Detected container runtime: {runtime}");

    match args.action {
        Action::Install => {
            install(&config, &runtime).await?;
            
            // DEPLOYMENT MODEL: Install runs as DaemonSet
            // After installation completes, the pod must stay alive to maintain
            // the kata-runtime label and artifacts. Sleep forever to keep pod running.
            info!("Install completed, daemonset mode: sleeping forever");
            std::future::pending::<()>().await;
        }
        Action::Cleanup => {
            cleanup(&config, &runtime).await?;
            
            // DEPLOYMENT MODEL: Cleanup runs as Job or Helm post-delete hook
            // For Helm post-delete hooks, exit immediately.
            // This ensures the pod terminates cleanly without waiting
            if config.helm_post_delete_hook {
                info!("Cleanup completed (Helm post-delete hook), exiting with status 0");
                std::process::exit(0);
            }
            
            // For regular cleanup jobs, exit normally after completion
            info!("Cleanup completed, exiting");
        }
        Action::Reset => {
            reset(&config, &runtime).await?;
            
            // DEPLOYMENT MODEL: Reset runs as Job
            // Exit after completion so the job can complete
            info!("Reset completed, exiting");
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

async fn install(config: &config::Config, runtime: &str) -> Result<()> {
    info!("Installing Kata Containers");

    const SUPPORTED_RUNTIMES: &[&str] = &[
        "crio",
        "containerd",
        "k3s",
        "k3s-agent",
        "rke2-agent",
        "rke2-server",
        "k0s-worker",
        "k0s-controller",
        "microk8s",
    ];

    if !SUPPORTED_RUNTIMES.contains(&runtime) {
        error!("Runtime {runtime} not supported, skipping installation");
        return Ok(());
    }

    if runtime != "crio" {
        runtime::containerd::containerd_snapshotter_version_check(config).await?;
        runtime::containerd::snapshotter_handler_mapping_validation_check(config)?;
    }

    let use_drop_in =
        runtime::is_containerd_capable_of_using_drop_in_files(config, runtime).await?;
    info!("Using containerd drop-in files: {use_drop_in}");

    let has_multi_install_suffix = config
        .multi_install_suffix
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false);

    if has_multi_install_suffix
        && !use_drop_in
        && !matches!(runtime, "k0s-worker" | "k0s-controller")
    {
        return Err(anyhow::anyhow!(
            "Multi installation can only be done if {runtime} supports drop-in configuration files"
        ));
    }

    // Validate snapshotter if needed
    match config.experimental_setup_snapshotter.as_ref() {
        Some(snapshotter) => {
            let non_empty_snapshotters: Vec<_> =
                snapshotter.iter().filter(|s| !s.is_empty()).collect();

            if !non_empty_snapshotters.is_empty() {
                if runtime == "crio" {
                    log::warn!("EXPERIMENTAL_SETUP_SNAPSHOTTER is being ignored!");
                    log::warn!("Snapshotter is a containerd specific option.");
                } else {
                    for s in &non_empty_snapshotters {
                        match s.as_str() {
                            "erofs" => {
                                runtime::containerd::containerd_erofs_snapshotter_version_check(
                                    config,
                                )
                                .await?;
                            }
                            "nydus" => {}
                            _ => {
                                return Err(anyhow::anyhow!(
                                    "{s} is not a supported snapshotter by kata-deploy"
                                ));
                            }
                        }
                    }
                }
            }
        }
        None => {}
    }

    runtime::containerd::setup_containerd_config_files(runtime, config)?;

    artifacts::install_artifacts(config).await?;

    runtime::configure_cri_runtime(config, runtime).await?;

    match config.experimental_setup_snapshotter.as_ref() {
        Some(snapshotters) => {
            for snapshotter in snapshotters {
                artifacts::snapshotters::install_snapshotter(snapshotter, config).await?;
                artifacts::snapshotters::configure_snapshotter(snapshotter, runtime, config)
                    .await?;
            }
        }
        None => {}
    }

    info!("About to restart runtime: {}", runtime);
    runtime::lifecycle::restart_runtime(config, runtime).await?;
    info!("Runtime restart completed successfully");

    k8s::label_node(config, "katacontainers.io/kata-runtime", Some("true"), true).await?;

    // Annotate node with installation status for upgrade orchestration
    let version = std::env::var("KATA_VERSION").unwrap_or_else(|_| "unknown".to_string());
    let timestamp = chrono::Utc::now().to_rfc3339();
    
    k8s::annotate_node(
        config,
        "katacontainers.io/kata-deploy-installed-version",
        Some(&version),
    )
    .await?;
    k8s::annotate_node(
        config,
        "katacontainers.io/kata-deploy-installed-at",
        Some(&timestamp),
    )
    .await?;
    info!("Node annotated with Kata version {} at {}", version, timestamp);

    info!("Kata Containers installation completed successfully");
    Ok(())
}

async fn cleanup(config: &config::Config, runtime: &str) -> Result<()> {
    info!("Cleaning up Kata Containers");

    info!("Counting kata-deploy daemonsets");
    let kata_deploy_installations = k8s::count_kata_deploy_daemonsets(config).await?;
    info!(
        "Found {} kata-deploy daemonset(s)",
        kata_deploy_installations
    );

    if config.helm_post_delete_hook && kata_deploy_installations == 0 {
        info!("Helm post-delete hook: removing kata-runtime label and annotations");
        k8s::label_node(config, "katacontainers.io/kata-runtime", None, false).await?;
        k8s::annotate_node(
            config,
            "katacontainers.io/kata-deploy-installed-version",
            None,
        )
        .await?;
        k8s::annotate_node(
            config,
            "katacontainers.io/kata-deploy-installed-at",
            None,
        )
        .await?;
        info!("Successfully removed kata-runtime label and install annotations");
    }

    match config.experimental_setup_snapshotter.as_ref() {
        Some(snapshotters) => {
            for snapshotter in snapshotters {
                info!("Uninstalling snapshotter: {}", snapshotter);
                artifacts::snapshotters::uninstall_snapshotter(snapshotter, config).await?;
                info!("Successfully uninstalled snapshotter: {}", snapshotter);
            }
        }
        None => {
            info!("No experimental snapshotters to uninstall");
        }
    }

    info!("Cleaning up CRI runtime configuration");
    runtime::cleanup_cri_runtime(config, runtime).await?;
    info!("Successfully cleaned up CRI runtime configuration");

    if !config.helm_post_delete_hook && kata_deploy_installations == 0 {
        info!("Setting cleanup label on node");
        k8s::label_node(
            config,
            "katacontainers.io/kata-runtime",
            Some("cleanup"),
            true,
        )
        .await?;
        info!("Successfully set cleanup label");
    }

    info!("Removing kata artifacts from host");
    artifacts::remove_artifacts(config).await?;
    info!("Successfully removed kata artifacts");

    info!("Kata Containers cleanup completed successfully");
    Ok(())
}

async fn reset(config: &config::Config, runtime: &str) -> Result<()> {
    info!("Resetting Kata Containers");

    k8s::label_node(config, "katacontainers.io/kata-runtime", None, false).await?;
    k8s::annotate_node(
        config,
        "katacontainers.io/kata-deploy-installed-version",
        None,
    )
    .await?;
    k8s::annotate_node(
        config,
        "katacontainers.io/kata-deploy-installed-at",
        None,
    )
    .await?;

    runtime::lifecycle::restart_cri_runtime(config, runtime).await?;
    if matches!(runtime, "crio" | "containerd") {
        utils::host_systemctl(&["restart", "kubelet"])?;
    }
    runtime::lifecycle::wait_till_node_is_ready(config).await?;

    info!("Kata Containers reset completed successfully");
    Ok(())
}
