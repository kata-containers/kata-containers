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
use semver::Version;

/// Env var name used to thread the detected container runtime through the
/// post-install re-exec. Avoids re-querying the apiserver after we've already
/// committed to a runtime.
const DETECTED_RUNTIME_ENV: &str = "KATA_DEPLOY_DETECTED_RUNTIME";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Override kata-deploy log verbosity.
    #[arg(long, value_enum)]
    log_level: Option<LogLevel>,

    #[arg(value_enum)]
    action: Action,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for log::LevelFilter {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Action {
    Install,
    Cleanup,
    Reset,
    /// Stage 0 of a staged (JobSet) install: validate host/node prerequisites
    /// without mutating the host. Fails fast with actionable diagnostics when
    /// the node cannot support installation.
    #[clap(name = "install-stage-host-check")]
    InstallStageHostCheck,
    /// Stage 1 of a staged (JobSet) install: install kata artifacts/config on
    /// the host and set up configured snapshotters. Does not touch CRI
    /// configuration, but is still privileged (host writes + snapshotter setup
    /// shell into the host via nsenter).
    #[clap(name = "install-stage-artifacts")]
    InstallStageArtifacts,
    /// Stage 2 of a staged (JobSet) install: write CRI drop-ins, restart the
    /// runtime, and wait for node readiness. Privileged + short-lived.
    #[clap(name = "install-stage-cri")]
    InstallStageCri,
    /// Stage 3 of a staged (JobSet) install: apply the kata-runtime node label.
    /// Unprivileged, Kubernetes API only.
    #[clap(name = "install-stage-label")]
    InstallStageLabel,
    /// Cleanup stage 1 of a staged (JobSet) uninstall: remove the kata-runtime
    /// node label first so the scheduler stops placing kata workloads here.
    /// Unprivileged, Kubernetes API only.
    #[clap(name = "cleanup-stage-unlabel")]
    CleanupStageUnlabel,
    /// Cleanup stage 2 of a staged (JobSet) uninstall: remove CRI drop-ins,
    /// restart the runtime, and wait for readiness. Privileged + short-lived.
    #[clap(name = "cleanup-stage-revert-cri")]
    CleanupStageRevertCri,
    /// Cleanup stage 3 of a staged (JobSet) uninstall: remove kata
    /// artifacts/config/symlinks from the host. Privileged (mutates the host
    /// filesystem under the install dir).
    #[clap(name = "cleanup-stage-remove-artifacts")]
    CleanupStageRemoveArtifacts,
    /// Internal: entered via re-exec after install completes. Holds the
    /// DaemonSet pod alive waiting for SIGTERM, then runs cleanup. Hidden
    /// from `--help`; users should never invoke this directly.
    #[clap(name = "internal-post-install-wait", hide = true)]
    InternalPostInstallWait,
}

/// Node label applied to mark a node as kata-capable. Shared across the
/// install/cleanup label stages so the key stays consistent.
const KATA_RUNTIME_LABEL: &str = "katacontainers.io/kata-runtime";
const SUGGESTED_KUBELET_RUNTIME_REQUEST_TIMEOUT_SECS: u64 = 10 * 60;
const MIN_EROFS_UTILS_VERSION: &str = "1.8.2";

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
    let args = Args::parse();

    // Set log level based on DEBUG environment variable
    // unless explicitly overridden via --log-level.
    let debug_enabled = std::env::var("DEBUG")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);

    let log_level = match args.log_level {
        Some(level) => level.into(),
        None if debug_enabled => log::LevelFilter::Debug,
        None => log::LevelFilter::Info,
    };

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .init();

    // Check if running as root (UID 0)
    if unsafe { libc::geteuid() } != 0 {
        return Err(anyhow::anyhow!("This program must be run as root"));
    }

    let config = config::Config::from_env()?;
    let action_str = match args.action {
        Action::Install => "install",
        Action::Cleanup => "cleanup",
        Action::Reset => "reset",
        Action::InstallStageHostCheck => "install-stage-host-check",
        Action::InstallStageArtifacts => "install-stage-artifacts",
        Action::InstallStageCri => "install-stage-cri",
        Action::InstallStageLabel => "install-stage-label",
        Action::CleanupStageUnlabel => "cleanup-stage-unlabel",
        Action::CleanupStageRevertCri => "cleanup-stage-revert-cri",
        Action::CleanupStageRemoveArtifacts => "cleanup-stage-remove-artifacts",
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
        // Staged (JobSet) install actions. Each runs one step of the install
        // pipeline as a short-lived Job/initContainer and exits. The DaemonSet
        // path does not use these directly; it goes through `install` above,
        // which composes the same stage functions.
        Action::InstallStageHostCheck => {
            install_stage_host_check(&config, &runtime).await?;
            info!("Install host-check stage completed, exiting");
        }
        Action::InstallStageArtifacts => {
            install_stage_artifacts(&config, &runtime).await?;
            info!("Install artifacts stage completed, exiting");
        }
        Action::InstallStageCri => {
            install_stage_cri(&config, &runtime).await?;
            info!("Install CRI stage completed, exiting");
        }
        Action::InstallStageLabel => {
            install_stage_label(&config).await?;
            info!("Install label stage completed, exiting");
        }
        // Staged (JobSet) cleanup actions. These run in reverse order
        // (unlabel -> revert-cri -> remove-artifacts) and, unlike the DaemonSet
        // `cleanup` above, do not perform DaemonSet-presence gating: the JobSet
        // workflow only schedules these when an uninstall is actually intended.
        Action::CleanupStageUnlabel => {
            cleanup_stage_unlabel(&config).await?;
            info!("Cleanup unlabel stage completed, exiting");
        }
        Action::CleanupStageRevertCri => {
            cleanup_stage_revert_cri(&config, &runtime).await?;
            info!("Cleanup revert-cri stage completed, exiting");
        }
        Action::CleanupStageRemoveArtifacts => {
            cleanup_stage_remove_artifacts(&config).await?;
            info!("Cleanup remove-artifacts stage completed, exiting");
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

/// Full install pipeline. Used by the DaemonSet deployment model. Composes the
/// same per-stage functions the staged JobSet workflow invokes individually, in
/// the canonical order: host-check -> artifacts -> cri -> label.
async fn install(config: &config::Config, runtime: &str) -> Result<()> {
    info!("Installing Kata Containers");

    install_stage_host_check(config, runtime).await?;
    install_stage_artifacts(config, runtime).await?;
    install_stage_cri(config, runtime).await?;
    install_stage_label(config).await?;

    info!("Kata Containers installation completed successfully");
    Ok(())
}

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

/// Install stage 0 (host-check): validate that this node can support a Kata
/// installation before any host mutation happens. This is read-only and safe
/// to run repeatedly; it fails fast with actionable diagnostics so a staged
/// JobSet can abort the per-node pipeline before the privileged stages run.
async fn install_stage_host_check(config: &config::Config, runtime: &str) -> Result<()> {
    info!("install (host-check): validating node prerequisites for runtime {runtime}");

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
                            validate_erofs_prerequisites(config).await?;
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

    if config_uses_guest_pull(config) {
        validate_kubelet_runtime_request_timeout(config, "guest pull").await?;
    }

    info!("install (host-check): node prerequisites satisfied");
    Ok(())
}

async fn validate_erofs_prerequisites(config: &config::Config) -> Result<()> {
    info!("Validating EROFS snapshotter prerequisites");

    runtime::containerd::containerd_erofs_snapshotter_version_check(config).await?;

    validate_host_kernel_feature_available(
        HostKernelFeature::Erofs,
        "Load or enable EROFS filesystem support before installing Kata and \
         make it persistent across reboots.",
    )?;

    if config.erofs_dmverity {
        validate_host_kernel_feature_available(
            HostKernelFeature::DeviceMapper,
            "Load or enable device-mapper support before installing Kata and \
             make it persistent across reboots.",
        )?;
        validate_host_kernel_feature_available(
            HostKernelFeature::DmVerity,
            "Load or enable the dm-verity target before installing Kata and \
             make it persistent across reboots.",
        )?;
    }

    validate_mkfs_erofs_version()?;

    // kata-deploy currently configures the EROFS snapshotter with
    // enable_fsverity=true, but this host check does not know the final
    // containerd configuration after user drop-ins, and it does not validate
    // the backing filesystem's fs-verity feature. Keep this check warning-only.
    warn_if_erofs_fsverity_may_be_unavailable();

    validate_kubelet_runtime_request_timeout(config, "EROFS layer conversion").await?;

    Ok(())
}

#[derive(Clone, Copy)]
enum HostKernelFeature {
    Erofs,
    DeviceMapper,
    DmVerity,
    FsVerity,
}

impl HostKernelFeature {
    fn name(self) -> &'static str {
        match self {
            Self::Erofs => "erofs",
            Self::DeviceMapper => "device-mapper",
            Self::DmVerity => "dm-verity",
            Self::FsVerity => "fs-verity",
        }
    }

    fn module_name(self) -> &'static str {
        match self {
            Self::Erofs => "erofs",
            Self::DeviceMapper => "dm_mod",
            Self::DmVerity => "dm_verity",
            Self::FsVerity => "fsverity",
        }
    }

    fn config_symbol(self) -> &'static str {
        match self {
            Self::Erofs => "CONFIG_EROFS_FS",
            Self::DeviceMapper => "CONFIG_BLK_DEV_DM",
            Self::DmVerity => "CONFIG_DM_VERITY",
            Self::FsVerity => "CONFIG_FS_VERITY",
        }
    }
}

fn validate_host_kernel_feature_available(
    feature: HostKernelFeature,
    remediation: &str,
) -> Result<()> {
    if host_module_visible(feature.module_name())
        || host_proc_config_has_builtin_feature(feature.config_symbol())
        || host_boot_config_has_builtin_feature(feature.config_symbol())
    {
        return Ok(());
    }

    anyhow::bail!(
        "Required host kernel feature `{}` is not available. {remediation}",
        feature.name()
    )
}

fn host_module_visible(module_name: &str) -> bool {
    let sys_module_path = format!("/sys/module/{module_name}");
    if utils::host_exec(&["test", "-d", &sys_module_path]).is_ok() {
        return true;
    }

    let proc_modules_pattern = format!("^{module_name} ");
    utils::host_exec(&["grep", "-q", &proc_modules_pattern, "/proc/modules"]).is_ok()
}

fn host_proc_config_has_builtin_feature(config_symbol: &str) -> bool {
    let config_value = format!("{config_symbol}=y");

    if utils::host_exec(&["test", "-r", "/proc/config.gz"]).is_err() {
        return false;
    }

    let Ok(output) = utils::host_exec(&["gzip", "-dc", "/proc/config.gz"]) else {
        return false;
    };

    output.lines().any(|line| line == config_value)
}

fn host_boot_config_has_builtin_feature(config_symbol: &str) -> bool {
    let config_pattern = format!("^{config_symbol}=y");

    let output = utils::host_exec(&["uname", "-r"]);
    let Ok(kernel_release) = output else {
        return false;
    };

    let kernel_config_path = format!("/boot/config-{}", kernel_release.trim());
    utils::host_exec(&["grep", "-Eq", &config_pattern, &kernel_config_path]).is_ok()
}

fn validate_mkfs_erofs_version() -> Result<()> {
    let output = utils::host_exec(&["mkfs.erofs", "--version"]).with_context(|| {
        "Required host command `mkfs.erofs` is not available. Install \
         erofs-utils >= 1.8.2 before enabling the EROFS snapshotter."
    })?;

    let version = parse_erofs_utils_version(&output).with_context(|| {
        format!("Could not parse erofs-utils version from `mkfs.erofs --version`: {output}")
    })?;
    let minimum_version = Version::parse(MIN_EROFS_UTILS_VERSION)?;

    if version < minimum_version {
        anyhow::bail!(
            "Host erofs-utils version {} is too old. kata-deploy configures \
             EROFS fsmerge mkfs_options that require erofs-utils >= {}.",
            version,
            MIN_EROFS_UTILS_VERSION
        );
    }

    info!(
        "host erofs-utils version {} satisfies minimum {}",
        version, MIN_EROFS_UTILS_VERSION
    );

    Ok(())
}

fn parse_erofs_utils_version(output: &str) -> Result<Version> {
    let version_re = regex::Regex::new(r"([0-9]+)\.([0-9]+)(?:\.([0-9]+))?")?;
    let captures = version_re
        .captures(output)
        .ok_or_else(|| anyhow::anyhow!("erofs-utils version not found"))?;

    let major = captures[1].parse::<u64>()?;
    let minor = captures[2].parse::<u64>()?;
    let patch = captures
        .get(3)
        .map(|patch| patch.as_str().parse::<u64>())
        .transpose()?
        .unwrap_or(0);

    Version::parse(&format!("{major}.{minor}.{patch}")).map_err(Into::into)
}

fn warn_if_erofs_fsverity_may_be_unavailable() {
    if let Err(err) = validate_host_kernel_feature_available(
        HostKernelFeature::FsVerity,
        "Install, load, or enable fs-verity support if the final EROFS \
         snapshotter configuration keeps enable_fsverity=true.",
    ) {
        log::warn!(
            "kata-deploy's default EROFS snapshotter configuration sets \
             enable_fsverity=true, but host fs-verity support was not detected \
             ({err}). This is warning-only because the final containerd \
             configuration may be changed by user drop-ins, and kata-deploy \
             does not yet validate the backing filesystem's fs-verity feature."
        );
    } else {
        log::warn!(
            "kata-deploy's default EROFS snapshotter configuration sets \
             enable_fsverity=true and host fs-verity support was detected, but \
             kata-deploy does not yet validate the backing filesystem's \
             fs-verity feature."
        );
    }
}

async fn validate_kubelet_runtime_request_timeout(
    config: &config::Config,
    operation: &str,
) -> Result<()> {
    let runtime_request_timeout = match k8s::get_kubelet_runtime_request_timeout(config).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            warn_runtime_request_timeout(
                operation,
                "kubelet /configz did not include runtimeRequestTimeout",
            );
            return Ok(());
        }
        Err(err) => {
            warn_runtime_request_timeout(
                operation,
                &format!("could not query kubelet runtimeRequestTimeout from /configz: {err}"),
            );
            return Ok(());
        }
    };

    let timeout_secs = match humantime::parse_duration(&runtime_request_timeout) {
        Ok(timeout) => timeout.as_secs(),
        Err(err) => {
            warn_runtime_request_timeout(
                operation,
                &format!(
                    "could not parse kubelet runtimeRequestTimeout value \
                     `{runtime_request_timeout}` from /configz: {err}"
                ),
            );
            return Ok(());
        }
    };

    if timeout_secs < SUGGESTED_KUBELET_RUNTIME_REQUEST_TIMEOUT_SECS {
        warn_runtime_request_timeout(
            operation,
            &format!(
                "kubelet runtimeRequestTimeout from /configz is \
                 `{runtime_request_timeout}` ({timeout_secs}s)"
            ),
        );
    }

    info!(
        "kubelet runtimeRequestTimeout from /configz is {runtime_request_timeout} ({timeout_secs}s)"
    );
    Ok(())
}

fn warn_runtime_request_timeout(operation: &str, detail: &str) {
    log::warn!(
        "{detail}. {operation} may run during CreateContainer; consider \
         configuring kubelet runtimeRequestTimeout to at least {}s on nodes \
         that run large images.",
        SUGGESTED_KUBELET_RUNTIME_REQUEST_TIMEOUT_SECS
    );
}

fn config_uses_guest_pull(config: &config::Config) -> bool {
    !config.experimental_force_guest_pull_for_arch.is_empty()
        || mapping_contains_value(config.pull_type_mapping_for_arch.as_deref(), "guest-pull")
        || config
            .custom_runtimes
            .iter()
            .any(|runtime| runtime.crio_pull_type.as_deref() == Some("guest-pull"))
}

fn mapping_contains_value(mapping: Option<&str>, expected_value: &str) -> bool {
    mapping.is_some_and(|mapping| {
        mapping.split(',').any(|entry| {
            let value = entry
                .split_once(':')
                .map(|(_, value)| value)
                .unwrap_or(entry)
                .trim();
            value == expected_value
        })
    })
}

/// Install stage 1 (artifacts): place kata artifacts/config on the host and set
/// up any configured snapshotters. This does not touch CRI configuration, but it
/// still needs privileged host access: writing under the host install dir and
/// the snapshotter setup (e.g. nydus) shell into the host via nsenter.
async fn install_stage_artifacts(config: &config::Config, runtime: &str) -> Result<()> {
    info!("install (artifacts): installing kata artifacts on host");

    artifacts::install_artifacts(config, runtime).await?;

    if runtime != "crio" {
        if let Some(snapshotters) = config.experimental_setup_snapshotter.as_ref() {
            for snapshotter in snapshotters {
                artifacts::snapshotters::install_snapshotter(snapshotter, config).await?;
            }
        }
    }

    info!("install (artifacts): artifacts installed");
    Ok(())
}

/// Install stage 2 (cri): write CRI drop-ins, configure snapshotters, restart
/// the runtime, and wait for the node to become ready. This is the privileged,
/// node-disrupting stage and is kept short-lived.
async fn install_stage_cri(config: &config::Config, runtime: &str) -> Result<()> {
    info!("install (cri): configuring CRI runtime");

    runtime::containerd::setup_containerd_config_files(runtime, config).await?;

    runtime::configure_cri_runtime(config, runtime).await?;

    if runtime != "crio" {
        if let Some(snapshotters) = config.experimental_setup_snapshotter.as_ref() {
            for snapshotter in snapshotters {
                artifacts::snapshotters::configure_snapshotter(snapshotter, runtime, config)
                    .await?;
            }
        }
    }

    info!("About to restart runtime: {}", runtime);
    runtime::lifecycle::restart_runtime(config, runtime).await?;
    info!("Runtime restart completed successfully");

    Ok(())
}

/// Install stage 3 (label): apply the kata-runtime node label. Unprivileged,
/// Kubernetes API only. Skips re-applying when the label is already correct.
///
/// As the very last action, once the label is confirmed present, remove any
/// configured startup taints (`STARTUP_TAINTS`). This is what makes the
/// scheduling handshake safe: a node can be provisioned with a startup taint
/// that keeps kata workloads off it until the runtime exists, and that taint is
/// only lifted here, strictly after artifacts are installed, the CRI runtime is
/// configured and restarted, and the node is labeled kata-capable.
async fn install_stage_label(config: &config::Config) -> Result<()> {
    info!("install (label): applying node label");

    match k8s::get_node_label(config, KATA_RUNTIME_LABEL).await {
        Ok(Some(ref val)) if val == "true" => {
            info!(
                "install (label): node already labeled {}=true, skipping",
                KATA_RUNTIME_LABEL
            );
        }
        // Any other state (absent, different value, or a transient read error)
        // falls through to label_node_with_retry, which applies and verifies.
        _ => {
            label_node_with_retry(config, KATA_RUNTIME_LABEL, "true").await?;
        }
    }

    remove_startup_taints(config).await;

    Ok(())
}

/// Remove the configured startup taints from this node, if any.
///
/// Best-effort by design: failing to remove a taint must not fail the install
/// (the runtime is already in place and the node is labeled). We log a warning
/// and let the next reconcile/retry try again. Leaving the taint in place is the
/// safe failure mode, since it only keeps workloads off the node rather than
/// admitting them prematurely.
async fn remove_startup_taints(config: &config::Config) {
    if config.startup_taints.is_empty() {
        return;
    }

    info!(
        "install (label): removing startup taint(s): {}",
        config.startup_taints.join(", ")
    );

    match k8s::remove_node_taints(config, &config.startup_taints).await {
        Ok(removed) if removed.is_empty() => {
            info!(
                "install (label): no matching startup taint present on node {} (nothing to remove)",
                config.node_name
            );
        }
        Ok(removed) => {
            info!(
                "install (label): removed startup taint(s) [{}] from node {}",
                removed.join(", "),
                config.node_name
            );
        }
        Err(e) => {
            log::warn!(
                "install (label): failed to remove startup taint(s) [{}] from node {}: {}; \
                 leaving them in place (workloads stay gated). Will retry on next install run.",
                config.startup_taints.join(", "),
                config.node_name,
                e
            );
        }
    }
}

/// Label the node and verify the label sticks, retrying if necessary.
///
/// On rke2/k3s a CRI restart also restarts the kubelet, and `wait_till_node_is_ready`
/// can return on a *stale* Ready=True observation from before the kubelet has
/// actually finished restarting (the kubelet only re-publishes node status every
/// ~10 s by default). That means a naive "apply + verify once" round-trips entirely
/// inside the window where the kubelet hasn't re-registered yet: we'd happily
/// confirm the label is set, declare install done, and only then would the kubelet
/// come back up and clobber the label with its cached set.
///
/// To outlive that race we require the label to remain at `label_value` for
/// `STABILITY_CHECKS` consecutive observations spaced `CHECK_INTERVAL` apart
/// (≈ 15 s by default — comfortably more than the kubelet's status-update period).
/// If it ever drifts inside that window we re-apply and restart the stability
/// counter. The whole thing is bounded by `MAX_APPLY_ATTEMPTS`.
async fn label_node_with_retry(
    config: &config::Config,
    label_key: &str,
    label_value: &str,
) -> Result<()> {
    const MAX_APPLY_ATTEMPTS: u32 = 12;
    const STABILITY_CHECKS: u32 = 6;
    const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
    const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

    for attempt in 1..=MAX_APPLY_ATTEMPTS {
        k8s::label_node(config, label_key, Some(label_value), true).await?;
        info!(
            "Applied label {}={} (attempt {}/{}); verifying stability ({} checks @ {}s)",
            label_key,
            label_value,
            attempt,
            MAX_APPLY_ATTEMPTS,
            STABILITY_CHECKS,
            CHECK_INTERVAL.as_secs(),
        );

        let mut stable_count: u32 = 0;
        let mut needs_reapply = false;
        while stable_count < STABILITY_CHECKS {
            tokio::time::sleep(CHECK_INTERVAL).await;

            match k8s::get_node_label(config, label_key).await {
                Ok(Some(val)) if val == label_value => {
                    stable_count += 1;
                    info!(
                        "Label {}={} stable {}/{}",
                        label_key, label_value, stable_count, STABILITY_CHECKS
                    );
                }
                Ok(actual) => {
                    log::warn!(
                        "Label {}={} drifted to {:?} after {}/{} stable observation(s); \
                         re-applying (attempt {}/{})",
                        label_key,
                        label_value,
                        actual,
                        stable_count,
                        STABILITY_CHECKS,
                        attempt,
                        MAX_APPLY_ATTEMPTS,
                    );
                    needs_reapply = true;
                    break;
                }
                Err(e) => {
                    log::warn!(
                        "Failed to verify label {} during stability check \
                         (attempt {}/{}): {}; will re-apply",
                        label_key,
                        attempt,
                        MAX_APPLY_ATTEMPTS,
                        e,
                    );
                    needs_reapply = true;
                    break;
                }
            }
        }

        if !needs_reapply {
            info!(
                "Label {}={} confirmed stable on node after {} apply attempt(s)",
                label_key, label_value, attempt
            );
            return Ok(());
        }

        if attempt < MAX_APPLY_ATTEMPTS {
            tokio::time::sleep(RETRY_DELAY).await;
        }
    }

    anyhow::bail!(
        "Label {}={} did not remain stable after {} apply attempts",
        label_key,
        label_value,
        MAX_APPLY_ATTEMPTS
    );
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
    k8s::label_node(config, KATA_RUNTIME_LABEL, None, false).await?;
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

/// Cleanup stage 1 (unlabel): remove the kata-runtime node label first so the
/// scheduler stops placing kata workloads on this node before any host
/// mutation. Unprivileged, Kubernetes API only. Skips when already absent.
async fn cleanup_stage_unlabel(config: &config::Config) -> Result<()> {
    info!("cleanup (unlabel): removing node label");

    // If the label is already absent, there is nothing to do. Any other state
    // (present, or unknown due to a transient read error) falls through to the
    // removal below.
    if let Ok(None) = k8s::get_node_label(config, KATA_RUNTIME_LABEL).await {
        info!(
            "cleanup (unlabel): label {} already absent, skipping",
            KATA_RUNTIME_LABEL
        );
        return Ok(());
    }

    k8s::label_node(config, KATA_RUNTIME_LABEL, None, false).await?;
    info!("cleanup (unlabel): label removed");
    Ok(())
}

/// Cleanup stage 2 (revert-cri): remove CRI drop-ins (and any snapshotter
/// config), then restart the runtime and wait for readiness. This is the
/// privileged, node-disrupting cleanup stage and is kept short-lived. Skips
/// entirely when the CRI drop-ins are already absent, avoiding an unnecessary
/// runtime restart.
async fn cleanup_stage_revert_cri(config: &config::Config, runtime: &str) -> Result<()> {
    info!("cleanup (revert-cri): reverting CRI configuration");

    if !cri_drop_in_present(config, runtime).await {
        info!("cleanup (revert-cri): CRI drop-ins already absent, skipping");
        return Ok(());
    }

    if runtime != "crio" {
        if let Some(snapshotters) = config.experimental_setup_snapshotter.as_ref() {
            for snapshotter in snapshotters {
                info!("cleanup (revert-cri): uninstalling snapshotter {snapshotter}");
                artifacts::snapshotters::uninstall_snapshotter(snapshotter, config).await?;
            }
        }
    }

    runtime::cleanup_cri_runtime_config(config, runtime).await?;

    info!("cleanup (revert-cri): restarting runtime");
    runtime::restart_and_wait_for_ready(config, runtime).await?;
    info!("cleanup (revert-cri): runtime restarted");

    Ok(())
}

/// Cleanup stage 3 (remove-artifacts): delete kata artifacts/config/symlinks
/// from the host. Skips when the install directory is already gone.
async fn cleanup_stage_remove_artifacts(config: &config::Config) -> Result<()> {
    info!("cleanup (remove-artifacts): removing kata artifacts from host");

    if !std::path::Path::new(&config.host_install_dir).exists() {
        info!(
            "cleanup (remove-artifacts): install dir {} already absent, skipping",
            config.host_install_dir
        );
        return Ok(());
    }

    artifacts::remove_artifacts(config).await?;
    info!("cleanup (remove-artifacts): artifacts removed");
    Ok(())
}

/// Best-effort check for whether kata's CRI drop-in configuration is present on
/// the host for this runtime. Used by the staged cleanup to skip a disruptive
/// runtime restart when there is nothing to revert. On any uncertainty (e.g.
/// the containerd paths cannot be resolved) this returns `true` so the caller
/// errs on the side of running the revert rather than incorrectly skipping it.
async fn cri_drop_in_present(config: &config::Config, runtime: &str) -> bool {
    if runtime == "crio" {
        return std::path::Path::new(&config.crio_drop_in_conf_file).exists();
    }

    match config.get_containerd_paths(runtime).await {
        Ok(paths) => {
            // /etc/containerd is mounted directly; other paths live under /host.
            let resolved = if paths.drop_in_file.starts_with("/etc/containerd/") {
                std::path::PathBuf::from(&paths.drop_in_file)
            } else {
                std::path::Path::new("/host").join(paths.drop_in_file.trim_start_matches('/'))
            };
            resolved.exists()
        }
        Err(e) => {
            log::warn!(
                "cleanup (revert-cri): could not resolve containerd paths to check drop-in \
                 presence ({e}); proceeding with revert"
            );
            true
        }
    }
}

async fn reset(config: &config::Config, runtime: &str) -> Result<()> {
    info!("Resetting Kata Containers");

    k8s::label_node(config, KATA_RUNTIME_LABEL, None, false).await?;
    runtime::lifecycle::restart_cri_runtime(config, runtime).await?;
    if matches!(runtime, "crio" | "containerd") {
        utils::host_systemctl(&["restart", "kubelet"])?;
    }
    runtime::lifecycle::wait_till_node_is_ready_timeout(config, Some(300)).await?;

    info!("Kata Containers reset completed successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Tests for CLI action wiring. The staged install/cleanup actions are the
    //! entrypoints the JobSet workflow invokes per node, so we lock in their
    //! exact subcommand names (a rename would silently break the chart) and the
    //! mapping into the `Action` enum.

    use super::*;
    use clap::ValueEnum;
    use rstest::rstest;

    /// Every staged subcommand name parses into the expected `Action` variant.
    /// Keep this in sync with the `#[clap(name = ...)]` attributes above.
    #[rstest]
    #[case("install", Action::Install)]
    #[case("cleanup", Action::Cleanup)]
    #[case("reset", Action::Reset)]
    #[case("install-stage-host-check", Action::InstallStageHostCheck)]
    #[case("install-stage-artifacts", Action::InstallStageArtifacts)]
    #[case("install-stage-cri", Action::InstallStageCri)]
    #[case("install-stage-label", Action::InstallStageLabel)]
    #[case("cleanup-stage-unlabel", Action::CleanupStageUnlabel)]
    #[case("cleanup-stage-revert-cri", Action::CleanupStageRevertCri)]
    #[case("cleanup-stage-remove-artifacts", Action::CleanupStageRemoveArtifacts)]
    #[case("internal-post-install-wait", Action::InternalPostInstallWait)]
    fn test_action_parses_from_arg(#[case] arg: &str, #[case] expected: Action) {
        let args = Args::try_parse_from(["kata-deploy", arg])
            .unwrap_or_else(|e| panic!("failed to parse action {arg:?}: {e}"));
        assert_eq!(
            std::mem::discriminant(&args.action),
            std::mem::discriminant(&expected),
            "arg {arg:?} parsed into the wrong Action variant",
        );
    }

    /// Unknown actions must be rejected rather than silently accepted.
    #[rstest]
    #[case("install-stage")]
    #[case("cleanup-stage")]
    #[case("install-stage-foo")]
    #[case("bogus")]
    fn test_unknown_action_is_rejected(#[case] arg: &str) {
        assert!(
            Args::try_parse_from(["kata-deploy", arg]).is_err(),
            "expected action {arg:?} to be rejected",
        );
    }

    /// The hidden internal waiter must stay hidden from `--help` so users never
    /// invoke it directly, while still being parseable (asserted above).
    #[test]
    fn test_internal_action_is_hidden() {
        let internal = Action::InternalPostInstallWait
            .to_possible_value()
            .expect("internal action should have a possible value");
        assert!(
            internal.is_hide_set(),
            "internal-post-install-wait should be hidden from --help",
        );
    }

    #[rstest]
    #[case("mkfs.erofs (erofs-utils) 1.9\navailable compressors: lz4\n", "1.9.0")]
    #[case("mkfs.erofs (erofs-utils) 1.8.2\n", "1.8.2")]
    #[case("erofs-utils 1.8\n", "1.8.0")]
    fn test_parse_erofs_utils_version(#[case] output: &str, #[case] expected: &str) {
        assert_eq!(
            parse_erofs_utils_version(output).unwrap(),
            Version::parse(expected).unwrap()
        );
    }

    #[test]
    fn test_parse_erofs_utils_version_rejects_invalid_output() {
        assert!(parse_erofs_utils_version("mkfs.erofs unknown").is_err());
    }

    /// All non-internal staged actions remain visible in `--help` so operators
    /// can discover and run individual stages.
    #[rstest]
    #[case(Action::InstallStageHostCheck)]
    #[case(Action::InstallStageArtifacts)]
    #[case(Action::InstallStageCri)]
    #[case(Action::InstallStageLabel)]
    #[case(Action::CleanupStageUnlabel)]
    #[case(Action::CleanupStageRevertCri)]
    #[case(Action::CleanupStageRemoveArtifacts)]
    fn test_staged_actions_are_visible(#[case] action: Action) {
        let value = action
            .to_possible_value()
            .expect("staged action should have a possible value");
        assert!(
            !value.is_hide_set(),
            "staged action {:?} should be visible in --help",
            value.get_name(),
        );
    }
}
