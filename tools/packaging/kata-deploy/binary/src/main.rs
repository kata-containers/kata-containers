// Copyright (c) 2024 Kata Containers community
// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

mod artifacts;
mod config;
mod health;
mod k8s;
mod runtime;
mod utils;

use anyhow::{Context, Result};
use clap::Parser;
use log::{error, info};

/// Env var name used to thread the detected container runtime through the
/// post-install re-exec. Avoids re-querying the apiserver after we've already
/// committed to a runtime.
const DETECTED_RUNTIME_ENV: &str = "KATA_DEPLOY_DETECTED_RUNTIME";

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
    /// Internal: entered via re-exec after install completes. Holds the
    /// DaemonSet pod alive waiting for SIGTERM, then runs cleanup. Hidden
    /// from `--help`; users should never invoke this directly.
    #[clap(name = "internal-post-install-wait", hide = true)]
    InternalPostInstallWait,
}

// Cap the tokio runtime to a small fixed number of worker threads. The default
// multi-thread runtime allocates `num_cpus()` workers (each with a ~2 MiB
// stack), which on a 200+ vCPU GPU node is the dominant contributor to the
// DaemonSet pod's VmData reservation (~440 MiB). Two workers is plenty:
//
//   - the install path is overwhelmingly I/O-bound,
//   - it shells out to `nsenter ... systemctl restart …` (synchronous,
//     blocking calls that wedge the thread they run on for tens of seconds);
//     a second worker keeps the health server able to answer kubelet probes
//     within timeoutSeconds while the first is blocked.
//
// `current_thread` would be tighter still, but starves the health server the
// moment a host_systemctl call runs — the kubelet then fails the readiness
// probe and the pod is restarted before install can finish.
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    // Set log level based on DEBUG environment variable
    // This must be done before initializing the logger
    let debug_enabled = std::env::var("DEBUG")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    let log_level = if debug_enabled {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
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
        Action::InternalPostInstallWait => "internal-post-install-wait",
    };
    config.print_info(action_str);

    // After re-exec we already know which runtime we committed to during
    // install — trust the env var and skip the apiserver round-trip. For
    // every other action we always detect from the cluster.
    let runtime = match args.action {
        Action::InternalPostInstallWait => std::env::var(DETECTED_RUNTIME_ENV)
            .with_context(|| format!("missing {DETECTED_RUNTIME_ENV} env var after re-exec"))?,
        _ => {
            let r = runtime::get_container_runtime(&config).await?;
            info!("Detected container runtime: {r}");
            r
        }
    };

    match args.action {
        Action::Install => {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(e) => {
                    log::warn!(
                        "Failed to register SIGTERM handler: {}, sleeping forever",
                        e
                    );
                    std::future::pending::<()>().await;
                    return Ok(());
                }
            };

            let health_state = health::HealthState::new();
            let health_port = health::health_port_from_env();
            let health_listener = health::bind_health(health_port).await?;
            // Clear FD_CLOEXEC now (before we hand the listener to the
            // spawned task) so that the kernel keeps the socket open across
            // the post-install re-exec below. Without this, the child
            // process would have to re-bind the port, briefly exposing
            // the kubelet's startup/liveness probes to bind races.
            let health_fd = health::prepare_listener_for_exec(&health_listener)?;
            tokio::spawn(health::serve_health(health_listener, health_state.clone()));

            // Race install against SIGTERM so cleanup always runs, even if
            // SIGTERM arrives during install (e.g. helm uninstall while the
            // container is restarting after a failed install attempt).
            let install_result = tokio::select! {
                result = install(&config, &runtime) => result,
                _ = sigterm.recv() => {
                    info!("Received SIGTERM during install, running cleanup before exit");
                    if let Err(e) = cleanup(&config, &runtime).await {
                        error!("Cleanup on SIGTERM failed: {}", e);
                    }
                    return Ok(());
                }
            };

            install_result?;
            health_state.set(health::State::Ready);

            // DEPLOYMENT MODEL: Install runs as DaemonSet. Stay alive to
            // maintain the kata-runtime label and artifacts. On SIGTERM
            // (pod termination), run cleanup to undo install before exit.
            //
            // Memory note: `install` builds up substantial peak heap
            // (kube clients, deserialised Node/RuntimeClass objects, TLS
            // pools). Neither musl nor glibc returns most of that to the
            // kernel after free, so a long-running idle wait here would
            // pin the DaemonSet's RSS at the install peak for the
            // lifetime of the pod. Re-exec into a tiny post-install
            // waiter instead: the kernel discards the entire address
            // space and we come back up holding only what cleanup
            // actually needs.
            //
            // The health-server listening socket is inherited across the
            // exec so kubelet probes don't see a single failure during
            // the handover.
            info!("Install completed, re-exec'ing into post-install waiter");
            reexec_into_post_install_wait(&runtime, health_fd)?;
            // reexec_into_post_install_wait only returns on failure —
            // bubble that up so the pod restarts and retries install.
            unreachable!("reexec_into_post_install_wait returned unexpectedly");
        }
        Action::InternalPostInstallWait => {
            use tokio::signal::unix::{signal, SignalKind};

            // Resume the health server on the listener inherited from the
            // install process so the kubelet keeps seeing /readyz=200
            // across the re-exec. The state is `Ready` from the start —
            // we only ever reach this action *after* a successful install.
            if let Some(fd_str) = std::env::var(health::HEALTH_FD_ENV)
                .ok()
                .filter(|s| !s.is_empty())
            {
                let fd: std::os::fd::RawFd = fd_str.parse().with_context(|| {
                    format!("invalid {} value: {fd_str:?}", health::HEALTH_FD_ENV)
                })?;
                let listener = health::listener_from_inherited_fd(fd)?;
                let state = health::HealthState::new();
                state.set(health::State::Ready);
                tokio::spawn(health::serve_health(listener, state));
            } else {
                log::warn!(
                    "{} not set on re-exec; post-install waiter will not serve health probes",
                    health::HEALTH_FD_ENV
                );
            }

            let mut sigterm = signal(SignalKind::terminate())
                .context("failed to register SIGTERM handler in post-install waiter")?;
            info!("Post-install waiter ready, blocking on SIGTERM");
            sigterm.recv().await;
            info!("Received SIGTERM, running cleanup before exit");
            if let Err(e) = cleanup(&config, &runtime).await {
                error!("Cleanup on SIGTERM failed: {}", e);
            }
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

    Ok(())
}

/// Re-exec the current binary into the hidden `internal-post-install-wait`
/// action. Propagates the detected runtime (so we don't have to re-query the
/// apiserver) and the health-listener FD (so kubelet probes don't see a gap
/// during the handover) through the environment. Only returns on failure.
fn reexec_into_post_install_wait(
    runtime: &str,
    health_fd: std::os::fd::RawFd,
) -> Result<std::convert::Infallible> {
    use std::os::unix::process::CommandExt;

    let me = std::env::current_exe().context("failed to resolve current_exe for re-exec")?;
    let err = std::process::Command::new(&me)
        .arg("internal-post-install-wait")
        .env(DETECTED_RUNTIME_ENV, runtime)
        .env(health::HEALTH_FD_ENV, health_fd.to_string())
        .exec();
    Err(anyhow::anyhow!(
        "failed to re-exec {} into post-install waiter: {}",
        me.display(),
        err
    ))
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
        return Err(anyhow::anyhow!(
            "Runtime {runtime} is not supported for Kata Containers installation"
        ));
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
    if let Some(snapshotter) = config.experimental_setup_snapshotter.as_ref() {
        let non_empty_snapshotters: Vec<_> = snapshotter.iter().filter(|s| !s.is_empty()).collect();

        if !non_empty_snapshotters.is_empty() {
            if runtime == "crio" {
                log::warn!("EXPERIMENTAL_SETUP_SNAPSHOTTER is being ignored!");
                log::warn!("Snapshotter is a containerd specific option.");
            } else {
                for s in &non_empty_snapshotters {
                    match s.as_str() {
                        "erofs" => {
                            runtime::containerd::containerd_erofs_snapshotter_version_check(config)
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

    runtime::containerd::setup_containerd_config_files(runtime, config).await?;

    artifacts::install_artifacts(config, runtime).await?;

    runtime::configure_cri_runtime(config, runtime).await?;

    if runtime != "crio" {
        if let Some(snapshotters) = config.experimental_setup_snapshotter.as_ref() {
            for snapshotter in snapshotters {
                artifacts::snapshotters::install_snapshotter(snapshotter, config).await?;
                artifacts::snapshotters::configure_snapshotter(snapshotter, runtime, config)
                    .await?;
            }
        }
    }

    info!("About to restart runtime: {}", runtime);
    runtime::lifecycle::restart_runtime(config, runtime).await?;
    info!("Runtime restart completed successfully");

    k8s::label_node(config, "katacontainers.io/kata-runtime", Some("true"), true).await?;

    info!("Kata Containers installation completed successfully");
    Ok(())
}

async fn cleanup(config: &config::Config, runtime: &str) -> Result<()> {
    info!("Cleaning up Kata Containers");

    // Step 1: Check if THIS pod's owning DaemonSet still exists.
    // If it does, this is a pod restart (rolling update, label change, etc.),
    // not an uninstall — skip everything so running kata pods are not disrupted.
    info!(
        "Checking if DaemonSet '{}' still exists",
        config.daemonset_name
    );
    if k8s::own_daemonset_exists(config).await? {
        info!(
            "DaemonSet '{}' still exists, \
             skipping all cleanup to avoid disrupting running kata pods",
            config.daemonset_name
        );
        return Ok(());
    }

    // Step 2: Our DaemonSet is gone (uninstall). Perform instance-specific
    // cleanup: snapshotters, CRI config, and artifacts for this instance.
    info!(
        "DaemonSet '{}' not found, proceeding with instance cleanup",
        config.daemonset_name
    );

    if runtime != "crio" {
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
    } else {
        info!("Skipping snapshotter uninstall on CRI-O (containerd-specific feature)");
    }

    info!("Cleaning up CRI runtime configuration");
    runtime::cleanup_cri_runtime_config(config, runtime).await?;
    info!("Successfully cleaned up CRI runtime configuration");

    info!("Removing kata artifacts from host");
    artifacts::remove_artifacts(config).await?;
    info!("Successfully removed kata artifacts");

    // Step 3: Check if ANY other kata-deploy DaemonSets still exist.
    // Shared resources (node label, CRI restart) are only safe to touch
    // when no other kata-deploy instance remains.
    let other_ds_count = k8s::count_any_kata_deploy_daemonsets(config).await?;
    if other_ds_count > 0 {
        info!(
            "{} other kata-deploy DaemonSet(s) still exist, \
             skipping node label removal and CRI restart",
            other_ds_count
        );
        return Ok(());
    }

    info!("No other kata-deploy DaemonSets found, performing full shared cleanup");

    info!("Removing kata-runtime label from node");
    k8s::label_node(config, "katacontainers.io/kata-runtime", None, false).await?;
    info!("Successfully removed kata-runtime label");

    // Restart the CRI runtime last. On k3s/rke2 this restarts the entire
    // server process, which kills this (terminating) pod. By doing it after
    // all other cleanup, we ensure config and artifacts are already gone.
    info!("Restarting CRI runtime");
    runtime::restart_and_wait_for_ready(config, runtime).await?;
    info!("CRI runtime restarted successfully");

    info!("Kata Containers cleanup completed successfully");
    Ok(())
}

async fn reset(config: &config::Config, runtime: &str) -> Result<()> {
    info!("Resetting Kata Containers");

    k8s::label_node(config, "katacontainers.io/kata-runtime", None, false).await?;
    runtime::lifecycle::restart_cri_runtime(config, runtime).await?;
    if matches!(runtime, "crio" | "containerd") {
        utils::host_systemctl(&["restart", "kubelet"])?;
    }
    runtime::lifecycle::wait_till_node_is_ready(config).await?;

    info!("Kata Containers reset completed successfully");
    Ok(())
}
