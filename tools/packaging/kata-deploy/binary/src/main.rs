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
use flate2::read::GzDecoder;
use log::{error, info};
use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, Write};

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
    /// Load the SELinux policy module the confined stages need. Runs first in
    /// both the install and the cleanup pipeline, privileged, since a stage
    /// asking for a type the node does not define cannot start at all.
    #[clap(name = "install-stage-selinux-policy")]
    InstallStageSelinuxPolicy,
    /// Stage 0 of a staged (JobSet) install: load the host kernel modules the
    /// enabled runtimes and snapshotters need. The only privileged stage, and
    /// the only one the DaemonSet path does not share.
    #[clap(name = "install-stage-load-kernel-modules")]
    InstallStageLoadKernelModules,
    /// Stage 1 of a staged (JobSet) install: validate host/node prerequisites
    /// without mutating the host. Fails fast with actionable diagnostics when
    /// the node cannot support installation.
    #[clap(name = "install-stage-host-check")]
    InstallStageHostCheck,
    /// Stage 2 of a staged (JobSet) install: install kata artifacts/config on
    /// the host and set up configured snapshotters. Does not touch CRI
    /// configuration.
    #[clap(name = "install-stage-artifacts")]
    InstallStageArtifacts,
    /// Stage 3 of a staged (JobSet) install: write CRI drop-ins, restart the
    /// runtime, and wait for node readiness.
    #[clap(name = "install-stage-cri")]
    InstallStageCri,
    /// Cleanup stage 2 of a staged (JobSet) uninstall: remove CRI drop-ins,
    /// restart the runtime, and wait for readiness.
    #[clap(name = "cleanup-stage-revert-cri")]
    CleanupStageRevertCri,
    /// Cleanup stage 3 of a staged (JobSet) uninstall: remove kata
    /// artifacts/config/symlinks from the host.
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
/// The value [`KATA_RUNTIME_LABEL`] carries while an install is under way: this
/// node has to be cleaned up, but cannot run kata workloads yet.
const KATA_RUNTIME_PENDING: &str = "false";
/// Every install marks its own nodes with a label named after its
/// `MULTI_INSTALL_SUFFIX`. Without it, an uninstall removing the shared
/// [`KATA_RUNTIME_LABEL`] could not tell whether it leaves another install's
/// workloads with nowhere to run.
const INSTANCE_LABEL_PREFIX: &str = "kata-deploy.katacontainers.io/";
/// The instance name of an install that set no `MULTI_INSTALL_SUFFIX`.
const DEFAULT_INSTANCE: &str = "default";
const SUGGESTED_KUBELET_RUNTIME_REQUEST_TIMEOUT_SECS: u64 = 10 * 60;
const MKFS_EROFS: &str = "mkfs.erofs";
const MIN_EROFS_UTILS_VERSION: &str = "1.8.2";
/// The `mkfs.erofs` options kata-deploy configures containerd's EROFS differ to
/// use, both added in erofs-utils 1.8.2. Spelled as the binary's own usage text
/// does, which is where `validate_mkfs_erofs_options` looks for them.
const REQUIRED_MKFS_EROFS_OPTIONS: &[&str] = &["--mkfs-time", "--sort"];

// Cap the tokio runtime to a small fixed number of worker threads. The default
// multi-thread runtime allocates `num_cpus()` workers (each with a ~2 MiB
// stack), which on a 200+ vCPU GPU node is the dominant contributor to the
// DaemonSet pod's VmData reservation (~440 MiB). Two workers is plenty:
//
//   - the install path is overwhelmingly I/O-bound,
//   - host command and systemd D-Bus operations are synchronous and may block
//     for tens of seconds;
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
    if matches!(
        args.action,
        Action::InstallStageSelinuxPolicy
            | Action::InstallStageLoadKernelModules
            | Action::InstallStageHostCheck
            | Action::InstallStageArtifacts
            | Action::InstallStageCri
            | Action::CleanupStageRevertCri
            | Action::CleanupStageRemoveArtifacts
    ) {
        verify_node_machine_id()?;
    }
    let action_str = match args.action {
        Action::Install => "install",
        Action::Cleanup => "cleanup",
        Action::Reset => "reset",
        Action::InstallStageSelinuxPolicy => "install-stage-selinux-policy",
        Action::InstallStageLoadKernelModules => "install-stage-load-kernel-modules",
        Action::InstallStageHostCheck => "install-stage-host-check",
        Action::InstallStageArtifacts => "install-stage-artifacts",
        Action::InstallStageCri => "install-stage-cri",
        Action::CleanupStageRevertCri => "cleanup-stage-revert-cri",
        Action::CleanupStageRemoveArtifacts => "cleanup-stage-remove-artifacts",
        Action::InternalPostInstallWait => "internal-post-install-wait",
    };
    // Every stage of a staged run resolves the same pod env, so repeating the
    // configuration in each one buries the few lines that differ. Only the stage
    // that opens a run prints it; keep this in sync with the chart's stage order.
    let opens_a_run = matches!(
        args.action,
        Action::Install
            | Action::Cleanup
            | Action::Reset
            | Action::InstallStageLoadKernelModules
            | Action::CleanupStageRevertCri
    );
    config.print_info(action_str, opens_a_run);

    // After re-exec we already know which runtime we committed to during
    // install — trust the env var and skip the apiserver round-trip. For
    // every other action we always detect from the cluster.
    let runtime = match args.action {
        Action::InternalPostInstallWait => std::env::var(DETECTED_RUNTIME_ENV)
            .with_context(|| format!("missing {DETECTED_RUNTIME_ENV} env var after re-exec"))?,
        // Loading a policy module is the same work whatever the CRI is, and this
        // runs before every stage that would need one detected.
        Action::InstallStageSelinuxPolicy => String::new(),
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
        Action::InstallStageSelinuxPolicy => {
            install_stage_selinux_policy()?;
            info!("Install SELinux-policy stage completed, exiting");
        }
        Action::InstallStageLoadKernelModules => {
            install_stage_load_kernel_modules(&config)?;
            info!("Install kernel-module stage completed, exiting");
        }
        Action::InstallStageHostCheck => {
            install_stage_host_check(&config, &runtime, true).await?;
            info!("Install host-check stage completed, exiting");
        }
        Action::InstallStageArtifacts => {
            install_stage_artifacts(&config, &runtime, true).await?;
            info!("Install artifacts stage completed, exiting");
        }
        Action::InstallStageCri => {
            install_stage_cri(&config, &runtime, true).await?;
            info!("Install CRI stage completed, exiting");
        }
        // No DaemonSet check here, unlike `cleanup` above: these stages only run
        // when an uninstall is what the dispatcher was asked for.
        Action::CleanupStageRevertCri => {
            cleanup_stage_revert_cri(&config, &runtime, true).await?;
            info!("Cleanup revert-cri stage completed, exiting");
        }
        Action::CleanupStageRemoveArtifacts => {
            cleanup_stage_remove_artifacts(&config).await?;
            info!("Cleanup remove-artifacts stage completed, exiting");
        }
    }

    Ok(())
}

/// Confirm this pod is on the node the dispatcher meant.
///
/// A Job is bound to a node by name, and a name can outlive the machine that
/// carried it, so refuse to mutate a host whose machine ID is not the one the
/// dispatcher passed down. An older chart passes none, and then there is nothing to
/// compare against.
fn verify_node_machine_id() -> Result<()> {
    const EXPECTED_ENV: &str = "NODE_MACHINE_ID";
    const HOST_MACHINE_ID: &str = "/host-machine-id";

    let Ok(expected) = std::env::var(EXPECTED_ENV) else {
        return Ok(());
    };
    let actual = std::fs::read_to_string(HOST_MACHINE_ID)
        .with_context(|| format!("failed to read the host identity from {HOST_MACHINE_ID}"))?;
    anyhow::ensure!(
        actual.trim() == expected.trim(),
        "target node identity changed before host mutation: expected machine ID {}, found {}",
        expected.trim(),
        actual.trim()
    );
    Ok(())
}

/// Serialize host mutation across every kata-deploy running on this node.
///
/// Two releases working on the same node at once would interleave their
/// configuration edits and restarts, leaving the runtime reading configuration that
/// is only half written. The lock lives on the host because that is all two
/// releases share, and is held until the returned file is dropped.
fn acquire_node_mutation_lock() -> Result<std::fs::File> {
    use std::os::fd::AsRawFd;

    const LOCK_PATH: &str = "/host-run-lock/kata-deploy.lock";
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(LOCK_PATH)
        .with_context(|| format!("failed to open the node mutation lock {LOCK_PATH}"))?;
    let result = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) };
    if result != 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("failed to acquire the node mutation lock {LOCK_PATH}"));
    }
    Ok(lock)
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

    install_stage_host_check(config, runtime, false).await?;
    install_stage_artifacts(config, runtime, false).await?;
    install_stage_cri(config, runtime, false).await?;
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

const HOST_ROOT: &str = "/host";
const HOST_MODULES_LOAD_DIR: &str = "/host-modules-load.d";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HostModule {
    name: &'static str,
    required: bool,
}

impl HostModule {
    const fn required(name: &'static str) -> Self {
        Self {
            name,
            required: true,
        }
    }

    const fn optional(name: &'static str) -> Self {
        Self {
            name,
            required: false,
        }
    }
}

/// The modules the stage can load itself, kept apart from the x86 backend
/// requirement because no module can satisfy the latter on its own.
#[derive(Debug, Default)]
struct HostModulePlan {
    modules: Vec<HostModule>,
    needs_x86_virtualization: bool,
}

/// Not composed into [`install`], so the DaemonSet path stays unprivileged.
fn install_stage_load_kernel_modules(config: &config::Config) -> Result<()> {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo")
        .context("failed to read /proc/cpuinfo while selecting host kernel modules")?;
    let custom_bases = config
        .custom_runtimes
        .iter()
        .map(|runtime| runtime.base_config.as_str())
        .collect::<Vec<_>>();
    let erofs_enabled = config
        .experimental_setup_snapshotter
        .as_ref()
        .is_some_and(|snapshotters| snapshotters.iter().any(|s| s == "erofs"));
    let plan = host_modules_for_install(
        std::env::consts::ARCH,
        &cpuinfo,
        &config
            .shims_for_arch
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        &custom_bases,
        erofs_enabled,
        config.erofs_dmverity,
    )?;

    if plan.modules.is_empty() && !plan.needs_x86_virtualization {
        info!("install (kernel-modules): no host modules are needed");
        return Ok(());
    }

    let _node_lock = acquire_node_mutation_lock()?;
    // Lazily, so modules that are already loaded need no modprobe.
    let mut modprobe = None;
    let mut loaded = Vec::new();
    for module in &plan.modules {
        if host_module_visible(module.name) {
            info!(
                "install (kernel-modules): host module {} is already loaded",
                module.name
            );
            loaded.push(module.name);
            continue;
        }

        info!(
            "install (kernel-modules): loading host module {}",
            module.name
        );
        let path = match &modprobe {
            Some(path) => path,
            None => match find_host_modprobe() {
                Ok(path) => modprobe.insert(path),
                Err(error) => {
                    handle_module_load_failure(*module, error)?;
                    continue;
                }
            },
        };
        match run_host_modprobe(path, module.name) {
            Ok(()) => loaded.push(module.name),
            Err(error) => handle_module_load_failure(*module, error)?,
        }
    }

    if plan.needs_x86_virtualization {
        ensure_x86_virtualization_backend()?;
    }

    // Persisting what loaded, rather than what was asked for, keeps a module
    // that does not apply to this node from being retried at every boot.
    persist_modules_load_config(config, &loaded)?;
    Ok(())
}

/// Follows the per-architecture `kata-runtime check` maps.
fn host_modules_for_install(
    arch: &str,
    cpuinfo: &str,
    shims: &[&str],
    custom_bases: &[&str],
    erofs_enabled: bool,
    erofs_dmverity: bool,
) -> Result<HostModulePlan> {
    let has_local_runtime = shims.iter().any(|shim| *shim != "remote")
        || custom_bases.iter().any(|base| *base != "remote");
    let mut plan = HostModulePlan::default();
    let modules = &mut plan.modules;

    if has_local_runtime {
        let vhost_vsock = if wants_host_vsock_device(shims, custom_bases) {
            HostModule::required("vhost_vsock")
        } else {
            HostModule::optional("vhost_vsock")
        };

        match arch {
            "x86_64" => {
                // Every x86 VMM we ship runs on either KVM or MSHV, picking at
                // run time, so KVM is worth trying but never the requirement:
                // a Hyper-V root partition cannot load it and does not need to.
                plan.needs_x86_virtualization = true;
                modules.push(HostModule::optional("kvm"));
                if cpuinfo.contains("GenuineIntel") {
                    modules.push(HostModule::optional("kvm_intel"));
                } else if cpuinfo.contains("AuthenticAMD") {
                    modules.push(HostModule::optional("kvm_amd"));
                }
                modules.extend([
                    HostModule::required("vhost"),
                    HostModule::required("vhost_net"),
                    vhost_vsock,
                ]);
            }
            "aarch64" | "riscv64" => modules.extend([
                HostModule::required("kvm"),
                HostModule::required("vhost"),
                HostModule::required("vhost_net"),
                vhost_vsock,
            ]),
            "powerpc64" | "powerpc64le" => modules.extend([
                HostModule::required("kvm"),
                HostModule::required("kvm_hv"),
                vhost_vsock,
            ]),
            "s390x" => modules.extend([HostModule::required("kvm"), vhost_vsock]),
            unsupported => anyhow::bail!(
                "cannot select Kata host kernel modules for unsupported architecture {unsupported}"
            ),
        }
    }

    if erofs_enabled {
        // fs-verity is deliberately absent: CONFIG_FS_VERITY is a bool, so it is
        // either built in or unavailable, and the host check already says which.
        modules.extend([HostModule::required("erofs"), HostModule::required("loop")]);
        if erofs_dmverity {
            modules.extend([
                HostModule::required("dm_mod"),
                HostModule::required("dm_verity"),
            ]);
        }
    }

    let mut seen = HashSet::new();
    modules.retain(|module| seen.insert(module.name));
    Ok(plan)
}

/// QEMU is the only shim that talks to the guest through the host's
/// /dev/vhost-vsock; the rest tunnel VSOCK over a UNIX socket and do not care
/// whether the module is there.
fn wants_host_vsock_device(shims: &[&str], custom_bases: &[&str]) -> bool {
    shims
        .iter()
        .chain(custom_bases.iter())
        .any(|runtime| runtime.starts_with("qemu"))
}

/// The device nodes are the honest test. `kvm` alone loads happily on a machine
/// with virtualization switched off in firmware and creates no /dev/kvm, and
/// MSHV cannot be loaded at all: mshv_root only binds when the kernel booted as
/// the Hyper-V root partition.
fn ensure_x86_virtualization_backend() -> Result<()> {
    if host_device_exists("kvm") {
        info!("install (kernel-modules): the host provides KVM");
        return Ok(());
    }

    if host_device_exists("mshv") {
        info!("install (kernel-modules): the host provides MSHV instead of KVM");
        return Ok(());
    }

    anyhow::bail!(
        "this node has no usable virtualization backend: neither /dev/kvm nor /dev/mshv is \
         present. Check the warnings above for why the KVM modules would not load, and that \
         virtualization is enabled in firmware or exposed to this VM."
    )
}

fn host_device_exists(device: &str) -> bool {
    std::path::Path::new(HOST_ROOT)
        .join("dev")
        .join(device)
        .exists()
        || std::path::Path::new("/dev").join(device).exists()
}

fn modules_load_config_path(
    base: &std::path::Path,
    multi_install_suffix: Option<&str>,
) -> Result<std::path::PathBuf> {
    let suffix = multi_install_suffix.unwrap_or("default");
    anyhow::ensure!(
        suffix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
        "MULTI_INSTALL_SUFFIX {suffix:?} cannot be used in a modules-load.d filename"
    );
    Ok(base.join(format!("kata-containers-{suffix}.conf")))
}

fn persist_modules_load_config(config: &config::Config, modules: &[&str]) -> Result<()> {
    let path = modules_load_config_path(
        std::path::Path::new(HOST_MODULES_LOAD_DIR),
        config.multi_install_suffix.as_deref(),
    )?;
    let content = modules_load_config_content(modules);
    let temp_path = path.with_extension(format!("conf.{}.tmp", std::process::id()));
    let write_result = (|| -> Result<()> {
        use std::os::unix::fs::OpenOptionsExt;

        let mut temp = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o644)
            .open(&temp_path)
            .with_context(|| {
                format!(
                    "failed to create temporary modules-load.d file {}",
                    temp_path.display()
                )
            })?;
        temp.write_all(content.as_bytes())
            .with_context(|| format!("failed to write {}", temp_path.display()))?;
        temp.sync_all()
            .with_context(|| format!("failed to sync {}", temp_path.display()))?;
        std::fs::rename(&temp_path, &path).with_context(|| {
            format!(
                "failed to atomically install modules-load.d file {}",
                path.display()
            )
        })?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result?;

    info!(
        "install (kernel-modules): persisted the loaded modules in {}",
        path.display()
    );
    Ok(())
}

fn modules_load_config_content(modules: &[&str]) -> String {
    format!(
        "# Managed by kata-deploy; removed when this installation is uninstalled.\n{}\n",
        modules.join("\n")
    )
}

fn remove_modules_load_config(config: &config::Config) -> Result<()> {
    let path = modules_load_config_path(
        std::path::Path::new(HOST_MODULES_LOAD_DIR),
        config.multi_install_suffix.as_deref(),
    )?;
    match std::fs::remove_file(&path) {
        Ok(()) => info!(
            "cleanup (remove-artifacts): removed modules-load.d file {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!("failed to remove modules-load.d file {}", path.display())
            })
        }
    }
    Ok(())
}

fn handle_module_load_failure(module: HostModule, error: anyhow::Error) -> Result<()> {
    if module.required {
        return Err(error);
    }

    log::warn!(
        "install (kernel-modules): optional host module {} could not be loaded: {error}",
        module.name
    );
    Ok(())
}

/// The path is returned as it looks after the chroot, not as mounted here.
fn find_host_modprobe() -> Result<String> {
    const CANDIDATES: &[&str] = &[
        "/usr/sbin/modprobe",
        "/sbin/modprobe",
        "/usr/bin/modprobe",
        "/bin/modprobe",
    ];

    CANDIDATES
        .iter()
        .find(|path| host_path_is_file(std::path::Path::new(HOST_ROOT), std::path::Path::new(path)))
        .map(|path| (*path).to_string())
        .with_context(|| {
            format!(
                "host modprobe was not found under {HOST_ROOT}; install kmod on the node before \
                 deploying Kata"
            )
        })
}

/// An absolute symlink target belongs to the host, not to this image.
fn host_path_is_file(root: &std::path::Path, path: &std::path::Path) -> bool {
    // The kernel's MAXSYMLINKS: fewer would reject chains the chroot resolves.
    const MAX_HOPS: usize = 40;

    let mut current = path.to_path_buf();
    for _ in 0..MAX_HOPS {
        let mounted = root.join(current.strip_prefix("/").unwrap_or(&current));
        let Ok(metadata) = std::fs::symlink_metadata(&mounted) else {
            return false;
        };
        if !metadata.is_symlink() {
            return metadata.is_file();
        }
        let Ok(target) = std::fs::read_link(&mounted) else {
            return false;
        };
        current = if target.is_absolute() {
            target
        } else {
            // ".." is left for the kernel to resolve against the real dir.
            current
                .parent()
                .unwrap_or(std::path::Path::new("/"))
                .join(target)
        };
    }
    false
}

fn run_host_modprobe(modprobe: &str, module: &str) -> Result<()> {
    use std::os::unix::process::CommandExt;

    let host_root = std::ffi::CString::new(HOST_ROOT).expect("HOST_ROOT contains no NUL");
    let root_dir = std::ffi::CString::new("/").expect("root path contains no NUL");
    let mut command = std::process::Command::new(modprobe);
    command.arg(module);

    // This image ships no kmod, and only the host's own modprobe matches the
    // running kernel's modules and compression.
    unsafe {
        command.pre_exec(move || {
            if libc::chroot(host_root.as_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::chdir(root_dir.as_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let output = command
        .output()
        .with_context(|| format!("failed to execute host {modprobe} for module {module}"))?;
    if output.status.success() {
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    anyhow::bail!(
        "host modprobe failed for module {module} (status {}): stdout={stdout:?}, stderr={stderr:?}",
        output.status
    )
}

/// The policy module shipped in this image, and the domains the chart names in
/// each stage's `seLinuxOptions`.
const SELINUX_POLICY_PATH: &str = "/opt/kata-artifacts/selinux/kata-deploy.cil";
const SELINUX_POLICY_DOMAINS: &[&str] = &[
    "kata_deploy_check_t",
    "kata_deploy_artifacts_t",
    "kata_deploy_cri_t",
    "kata_deploy_node_binaries_t",
    "kata_deploy_t",
];

/// Load the SELinux policy module the confined stages need.
///
/// Installed with the *host's* own `semodule`, for the same reason the kernel
/// modules use the host's `modprobe`: the policy store belongs to the node and
/// only the node's tooling matches its version.
fn install_stage_selinux_policy() -> Result<()> {
    let Some(selinuxfs) = host_selinuxfs() else {
        // Not an error, so the chart's flag can be left on for a mixed cluster:
        // runc ignores labels where SELinux is off.
        info!("install (selinux-policy): SELinux is disabled on this node, nothing to load");
        return Ok(());
    };
    info!(
        "install (selinux-policy): SELinux is enabled (selinuxfs at {})",
        selinuxfs.display()
    );

    let semodule = find_host_semodule()?;

    // The node has one policy store, so two releases installing at once would
    // drive it concurrently.
    let _node_lock = acquire_node_mutation_lock()?;

    let staged = stage_selinux_policy_on_host()?;
    // Unconditional rather than skipped when the module looks present: a newer
    // image may carry new rules under the same domain names, and `semodule -i`
    // is idempotent.
    let result = run_host_semodule(&semodule, &staged.chroot_path);
    if let Err(error) = std::fs::remove_file(&staged.staged_path) {
        log::debug!(
            "install (selinux-policy): could not remove staged policy {}: {error}",
            staged.staged_path.display()
        );
    }
    result?;

    verify_selinux_domains(&selinuxfs)
}

/// Where the node's selinuxfs is, or `None` when SELinux is disabled.
///
/// One kernel-wide filesystem, so the container's own view of it is the node's;
/// the host mount is tried first in case a runtime stops offering a writable one.
fn host_selinuxfs() -> Option<std::path::PathBuf> {
    ["/host/sys/fs/selinux", "/sys/fs/selinux"]
        .iter()
        .map(std::path::PathBuf::from)
        .find(|path| path.join("enforce").exists())
}

/// The path is returned as it looks after the chroot, not as mounted here.
fn find_host_semodule() -> Result<String> {
    const CANDIDATES: &[&str] = &[
        "/usr/sbin/semodule",
        "/sbin/semodule",
        "/usr/bin/semodule",
        "/bin/semodule",
    ];

    CANDIDATES
        .iter()
        .find(|path| host_path_is_file(std::path::Path::new(HOST_ROOT), std::path::Path::new(path)))
        .map(|path| (*path).to_string())
        .with_context(|| {
            format!(
                "this node has SELinux enabled but no semodule under {HOST_ROOT}; install \
                 policycoreutils on the node, or deploy with selinux.enabled=false and accept \
                 that the install needs privileged containers instead"
            )
        })
}

/// The policy file as this container sees it, and as the chroot will.
struct StagedPolicy {
    staged_path: std::path::PathBuf,
    chroot_path: String,
}

/// Copy the policy somewhere the chroot can reach, since the host's `semodule`
/// cannot see this image's filesystem.
fn stage_selinux_policy_on_host() -> Result<StagedPolicy> {
    const CHROOT_DIR: &str = "/run/kata-deploy";
    const FILE_NAME: &str = "kata-deploy.cil";

    let dir = std::path::Path::new(HOST_ROOT).join(CHROOT_DIR.trim_start_matches('/'));
    std::fs::create_dir_all(&dir).with_context(|| {
        format!(
            "failed to create the policy staging directory {}",
            dir.display()
        )
    })?;
    let staged_path = dir.join(FILE_NAME);
    std::fs::copy(SELINUX_POLICY_PATH, &staged_path).with_context(|| {
        format!(
            "failed to stage the SELinux policy {SELINUX_POLICY_PATH} at {}",
            staged_path.display()
        )
    })?;

    Ok(StagedPolicy {
        staged_path,
        chroot_path: format!("{CHROOT_DIR}/{FILE_NAME}"),
    })
}

fn run_host_semodule(semodule: &str, policy: &str) -> Result<()> {
    use std::os::unix::process::CommandExt;

    let host_root = std::ffi::CString::new(HOST_ROOT).expect("HOST_ROOT contains no NUL");
    let root_dir = std::ffi::CString::new("/").expect("root path contains no NUL");
    let mut command = std::process::Command::new(semodule);
    command.arg("--install").arg(policy);

    unsafe {
        command.pre_exec(move || {
            if libc::chroot(host_root.as_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::chdir(root_dir.as_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    info!("install (selinux-policy): loading {policy} with the host's {semodule}");
    let output = command
        .output()
        .with_context(|| format!("failed to execute host {semodule} for policy {policy}"))?;
    if output.status.success() {
        info!("install (selinux-policy): policy module loaded");
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    anyhow::bail!(
        "host semodule failed to install {policy} (status {}): stdout={stdout:?}, stderr={stderr:?}",
        output.status
    )
}

/// Confirm every domain the chart asks for now resolves, so that a stage which
/// would otherwise fail with an opaque runc error fails here by name instead.
///
/// Asked of the kernel through selinuxfs, which needs no `seinfo`: nodes are not
/// obliged to carry setools, and RHEL 9 does not install it.
fn verify_selinux_domains(selinuxfs: &std::path::Path) -> Result<()> {
    let context_path = selinuxfs.join("context");
    // An unwritable interface would read as "every domain missing", so check it
    // before trusting its rejections.
    if let Err(error) = std::fs::OpenOptions::new().write(true).open(&context_path) {
        log::warn!(
            "install (selinux-policy): cannot use {} to verify the policy's domains ({error}); \
             the module loaded, but a stage requesting a missing domain will fail with an opaque \
             runc error instead of a clear one here",
            context_path.display()
        );
        return Ok(());
    }

    let missing: Vec<&str> = SELINUX_POLICY_DOMAINS
        .iter()
        .copied()
        .filter(|domain| !selinux_context_is_valid(&context_path, domain))
        .collect();

    anyhow::ensure!(
        missing.is_empty(),
        "the SELinux policy module loaded but does not define {}; the node's policy is missing \
         what the install stages ask for, so they would fail to start. This means the loaded \
         module is not the one this image ships: check for a kata-deploy module installed at a \
         higher priority with `semodule --list-modules=full`",
        missing.join(", ")
    );

    info!(
        "install (selinux-policy): all {} domains resolve",
        SELINUX_POLICY_DOMAINS.len()
    );
    Ok(())
}

fn selinux_context_is_valid(context_path: &std::path::Path, domain: &str) -> bool {
    use std::io::Write;

    let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(context_path) else {
        return false;
    };
    // A whole context, as the kernel validates no single field of one. The role
    // and level are those the chart pairs each domain with.
    file.write_all(format!("system_u:system_r:{domain}:s0").as_bytes())
        .is_ok()
}

/// Install stage 1 (host-check): validate that this node can support a Kata
/// installation before artifacts or CRI configuration are changed. This is
/// read-only and safe to run repeatedly; it fails fast with actionable
/// diagnostics so a staged Job can abort before persistent host changes.
///
/// `staged` marks the job-mode pipeline, whose containers hold no Kubernetes
/// credentials: the one check that needs the apiserver (reading the kubelet's
/// `runtimeRequestTimeout` out of `/configz`, which is advisory) is left to the
/// dispatcher, which runs it per node before dispatching.
async fn install_stage_host_check(
    config: &config::Config,
    runtime: &str,
    staged: bool,
) -> Result<()> {
    info!("install (host-check): validating node prerequisites for runtime {runtime}");

    if !SUPPORTED_RUNTIMES.contains(&runtime) {
        return Err(anyhow::anyhow!(
            "Runtime {runtime} is not supported for Kata Containers installation"
        ));
    }

    runtime::validate_declared_distribution(config, runtime)?;

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
                            validate_erofs_prerequisites(config, staged).await?;
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

    if config_uses_guest_pull(config) && !staged {
        validate_kubelet_runtime_request_timeout(config, "guest pull").await?;
    }

    info!("install (host-check): node prerequisites satisfied");
    Ok(())
}

async fn validate_erofs_prerequisites(config: &config::Config, staged: bool) -> Result<()> {
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

    validate_mkfs_erofs_options()?;

    // kata-deploy currently configures the EROFS snapshotter with
    // enable_fsverity=true, but this host check does not know the final
    // containerd configuration after user drop-ins, and it does not validate
    // the backing filesystem's fs-verity feature. Keep this check warning-only.
    warn_if_erofs_fsverity_may_be_unavailable();

    if !staged {
        validate_kubelet_runtime_request_timeout(config, "EROFS layer conversion").await?;
    }

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
    if std::path::Path::new(&format!("/sys/module/{module_name}")).is_dir() {
        return true;
    }

    std::fs::read_to_string("/proc/modules")
        .map(|content| {
            content.lines().any(|line| {
                line.split_whitespace()
                    .next()
                    .is_some_and(|name| name == module_name)
            })
        })
        .unwrap_or(false)
}

fn host_proc_config_has_builtin_feature(config_symbol: &str) -> bool {
    let Ok(config_gz) = std::fs::File::open("/proc/config.gz") else {
        return false;
    };

    let config_value = format!("{config_symbol}=y");
    std::io::BufReader::new(GzDecoder::new(config_gz))
        .lines()
        .map_while(Result::ok)
        .any(|line| line == config_value)
}

fn host_boot_config_has_builtin_feature(config_symbol: &str) -> bool {
    let config_value = format!("{config_symbol}=y");
    let Ok(kernel_release) = std::fs::read_to_string("/proc/sys/kernel/osrelease") else {
        return false;
    };

    let kernel_config_path = format!("/boot/config-{}", kernel_release.trim());
    std::fs::read_to_string(kernel_config_path)
        .map(|content| content.lines().any(|line| line == config_value))
        .unwrap_or(false)
}

/// Verify that the host's `mkfs.erofs` accepts the options containerd's EROFS
/// differ will pass to it (see `configure_erofs_snapshotter`).
///
/// Running the host binary to ask it is not possible: it is linked against the
/// host's loader and libraries, which the container does not have. So this
/// searches the binary for the options its usage text lists. What it documents
/// is what it accepts, which is what we care about — the erofs-utils version
/// only ever stood in for it.
fn validate_mkfs_erofs_options() -> Result<()> {
    let mkfs_erofs = utils::find_host_program(MKFS_EROFS).with_context(|| {
        format!(
            "Required host command `{MKFS_EROFS}` is not available. Install \
             erofs-utils >= {MIN_EROFS_UTILS_VERSION} before enabling the \
             EROFS snapshotter, or add a nodeBinaries entry taking it from \
             an image."
        )
    })?;

    let binary = std::fs::read(&mkfs_erofs)
        .with_context(|| format!("Failed to read host command {}", mkfs_erofs.display()))?;

    let missing: Vec<&str> = REQUIRED_MKFS_EROFS_OPTIONS
        .iter()
        .copied()
        .filter(|option| !documents_option(&binary, option))
        .collect();

    if !missing.is_empty() {
        anyhow::bail!(
            "Host {} does not support the {} option(s) that kata-deploy \
             configures the EROFS differ to use. Install erofs-utils >= {}.",
            mkfs_erofs.display(),
            missing.join(", "),
            MIN_EROFS_UTILS_VERSION
        );
    }

    info!(
        "host {} supports the required mkfs options",
        mkfs_erofs.display()
    );

    Ok(())
}

/// Whether `binary` documents `option`, dashes and all, as a word of its own.
///
/// Looking for the bare `getopt_long` name instead does not work: it is short
/// enough for the linker to fold into the tail of an unrelated string, which
/// nothing can tell from `sort` inside `qsort`. aarch64 builds of erofs-utils
/// keep no `sort` but the one in glibc's `rfc3484_sort`.
fn documents_option(binary: &[u8], option: &str) -> bool {
    let option = option.as_bytes();
    let bounds_word = |byte: &u8| !byte.is_ascii_alphanumeric() && !b"-_".contains(byte);

    binary
        .windows(option.len())
        .enumerate()
        .any(|(start, window)| {
            window == option
                // Both ends, so that `--sort` inside a longer word is no more
                // accepted than `sort` inside `qsort` was.
                && start
                    .checked_sub(1)
                    .is_none_or(|before| bounds_word(&binary[before]))
                && binary.get(start + option.len()).is_none_or(bounds_word)
        })
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

/// The marker label of one install.
fn instance_label(suffix: Option<&str>) -> String {
    format!(
        "{INSTANCE_LABEL_PREFIX}{}",
        suffix.unwrap_or(DEFAULT_INSTANCE)
    )
}

/// What the marks of the other installs on a node say about the label they share.
#[derive(Debug, PartialEq, Eq)]
enum SharedLabel {
    /// At least one other install is serving Kata here: the shared label stays `true`.
    Keep,
    /// The other installs here are all mid-flight or failed. Nothing may be scheduled
    /// on the strength of the shared label, but the key has to stay so those installs'
    /// own uninstalls can still find the node.
    Demote,
    /// Ours was the last mark: the key goes.
    Remove,
}

/// Read the marks other installs left on a node.
///
/// The value matters, not just the key: a `false` mark is a claim on a node whose
/// install has not finished, and reading it as "Kata is served here" would leave the
/// node advertised with nothing behind it.
fn shared_label_after(labels: &BTreeMap<String, String>, ours: &str) -> SharedLabel {
    let mut any = false;
    for (key, value) in labels {
        if key == ours || !key.starts_with(INSTANCE_LABEL_PREFIX) {
            continue;
        }
        any = true;
        if value != KATA_RUNTIME_PENDING {
            return SharedLabel::Keep;
        }
    }

    if any {
        SharedLabel::Demote
    } else {
        SharedLabel::Remove
    }
}

/// Take this install's mark off the node, and the label it shares with every other
/// install with it, unless another install is still holding the node. Returns
/// whether one is.
async fn release_node(config: &config::Config) -> Result<bool> {
    let ours = instance_label(config.multi_install_suffix.as_deref());

    // A legacy install leaves no marker behind, so when no other marker is left,
    // ask whether another kata-deploy still wants a pod here: otherwise this
    // uninstall takes the shared label out from under that install.
    //
    // Answered here rather than inside the rewrite below, and so a snapshot: a
    // DaemonSet lives outside this node, and nothing can make reading it atomic
    // with a write guarded by this node's resourceVersion. Should that DaemonSet
    // go in between, the shared label is left for the next uninstall to take.
    let labels = k8s::get_node_labels(config).await?;
    let legacy_others = shared_label_after(&labels, &ours) == SharedLabel::Remove
        && k8s::other_kata_deploy_daemonset_selects_node(config).await?;

    let others = k8s::rewrite_node_labels(config, |labels| {
        let mut updates: Vec<(String, Option<String>)> = Vec::new();
        if labels.contains_key(&ours) {
            updates.push((ours.clone(), None));
        }

        let verdict = if legacy_others {
            SharedLabel::Keep
        } else {
            shared_label_after(labels, &ours)
        };

        match verdict {
            SharedLabel::Keep => (),
            SharedLabel::Demote => {
                if labels.get(KATA_RUNTIME_LABEL).map(String::as_str) != Some(KATA_RUNTIME_PENDING)
                {
                    updates.push((
                        KATA_RUNTIME_LABEL.to_string(),
                        Some(KATA_RUNTIME_PENDING.to_string()),
                    ));
                }
            }
            SharedLabel::Remove => {
                if labels.contains_key(KATA_RUNTIME_LABEL) {
                    updates.push((KATA_RUNTIME_LABEL.to_string(), None));
                }
            }
        }

        (updates, verdict)
    })
    .await?;

    match others {
        SharedLabel::Keep => info!(
            "another kata-deploy install is still serving Kata from this node, leaving {} in place",
            KATA_RUNTIME_LABEL
        ),
        SharedLabel::Demote => info!(
            "one or more kata-deploy installs have claimed this node but none has finished \
             installing on it, so {} stays as {}",
            KATA_RUNTIME_LABEL, KATA_RUNTIME_PENDING
        ),
        SharedLabel::Remove => info!("removed {} from this node", KATA_RUNTIME_LABEL),
    }

    Ok(others != SharedLabel::Remove)
}

/// Mark the node as one kata-deploy has started installing on, before anything is
/// written to it.
///
/// `helm uninstall` cleans every node carrying the kata-runtime label, whatever its
/// value, so a node labelled only once the install *finishes* is out of its reach
/// for as long as it is half-installed. Not `true`, because RuntimeClasses select
/// that exact value; an existing label is left alone, so reinstalling over a working
/// node cannot withdraw it from scheduling.
///
/// Best-effort: failing an install over one label write would trade a rare orphan
/// for a common outage. Staged runs skip this, because their dispatcher marks the
/// node before creating the Job.
async fn claim_node(config: &config::Config) {
    let ours = instance_label(config.multi_install_suffix.as_deref());
    for key in [KATA_RUNTIME_LABEL, ours.as_str()] {
        if let Err(e) = k8s::label_node(config, key, Some(KATA_RUNTIME_PENDING), false).await {
            log::warn!(
                "install: could not mark node {} as being installed on ({e}). Should this install \
                 fail before it labels the node, `helm uninstall` will not clean this node up.",
                config.node_name
            );
        }
    }
}

/// Install stage 1 (artifacts): place kata artifacts/config on the host and set
/// up any configured snapshotters. This does not touch CRI configuration.
async fn install_stage_artifacts(
    config: &config::Config,
    runtime: &str,
    staged: bool,
) -> Result<()> {
    info!("install (artifacts): installing kata artifacts on host");

    if !staged {
        claim_node(config).await;
    }

    let _node_lock = acquire_node_mutation_lock()?;

    // Refuse before touching the host: whole-file containerd configuration keeps a
    // single backup, which the first uninstall would restore over every other
    // installation's handlers. Read under the lock, or another release's edit could
    // be half-written while this one decides.
    if runtime != "crio"
        && config
            .multi_install_suffix
            .as_deref()
            .is_some_and(|suffix| !suffix.is_empty())
    {
        let paths = config.get_containerd_paths(runtime).await?;
        anyhow::ensure!(
            paths.use_drop_in,
            "multi-install requires containerd drop-in support: whole-file configuration and its \
             single backup cannot preserve another installation during uninstall"
        );
    }

    artifacts::install_artifacts(config, runtime).await?;

    if runtime != "crio" {
        if let Some(snapshotters) = config.experimental_setup_snapshotter.as_ref() {
            for snapshotter in snapshotters {
                artifacts::snapshotters::install_snapshotter(snapshotter, config, runtime).await?;
            }
        }
    }

    info!("install (artifacts): artifacts installed");
    Ok(())
}

/// Install stage 2 (cri): write CRI drop-ins, configure snapshotters, restart
/// the runtime, and wait for the node to become ready. This node-disrupting
/// stage is kept short-lived.
///
/// `staged` distinguishes the two deployment models, because they survive the
/// runtime restart very differently:
///
///   - DaemonSet (`staged == false`): this runs inside a long-lived, regular
///     container. When containerd is restarted the kubelet re-attaches to the
///     still-running container, so the process survives the bounce, reaches the
///     readiness wait, and goes on to label the node in a single shot. It
///     therefore always restarts.
///
///   - Job (`staged == true`): this runs as a short-lived *init* container in a
///     `restartPolicy: Never` per-node Job. On some platforms (notably AKS)
///     restarting the very containerd that manages this pod tears the init
///     container down (exit 255, no logs) before it can finish - so the Job
///     retries with a fresh pod. To let the retry converge we skip the restart
///     once three things hold: the config we just (idempotently) re-applied is
///     byte-for-byte what was already on disk, systemd says the runtime has been
///     up since it was written, and the node reports the runtime serving the
///     handlers that config defines. No one of them is enough: an unchanged
///     config also describes an attempt that died before restarting anything, a
///     recent restart says nothing about an upgrade that changes the config, and
///     a node that cannot report handlers cannot rule either way. A genuine
///     change still restarts; if that restart kills the init container again,
///     the next retry finds all three satisfied and takes the skip path.
///
/// Either way the stage ends by confirming the runtime is serving kata handlers,
/// so a runtime that ignored what we wrote fails here rather than at the first
/// kata pod scheduled onto the node.
async fn install_stage_cri(config: &config::Config, runtime: &str, staged: bool) -> Result<()> {
    info!("install (cri): configuring CRI runtime");
    let _node_lock = acquire_node_mutation_lock()?;

    if runtime != "crio"
        && config
            .multi_install_suffix
            .as_deref()
            .is_some_and(|suffix| !suffix.is_empty())
    {
        let paths = config.get_containerd_paths(runtime).await?;
        anyhow::ensure!(
            paths.use_drop_in,
            "multi-install requires containerd drop-in support: whole-file configuration and its \
             single backup cannot preserve another installation during uninstall"
        );
    }

    let config_before = if staged {
        runtime::cri_config_snapshot(config, runtime).await
    } else {
        None
    };

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

    let handlers = config.shim_handlers();

    if staged {
        // Every path out of here that keeps the restart says why: a retry loop that
        // restarts forever is otherwise indistinguishable from one that never tried.
        match config_before {
            None => info!(
                "install (cri): no readable CRI config predates this attempt; a restart is needed"
            ),
            Some(before) => {
                let unchanged = runtime::cri_config_snapshot(config, runtime)
                    .await
                    .is_some_and(|after| after.same_config_as(&before));
                if !unchanged {
                    info!(
                        "install (cri): configuring {runtime} changed its CRI config; a restart is \
                         needed"
                    );
                } else if runtime::lifecycle::cri_serving_config_from(runtime, before.written_at())
                    .await
                {
                    info!(
                        "install (cri): CRI config for {runtime} is unchanged from a previous \
                         attempt, and {runtime} has been up since it was written. Skipping the \
                         (self-terminating) restart and checking the runtime is up instead."
                    );
                    runtime::lifecycle::wait_till_cri_unit_active(runtime, 300).await?;
                    info!("install (cri): runtime is up; CRI stage complete without restart");
                    return Ok(());
                }
            }
        }
    }

    info!("About to restart runtime: {}", runtime);
    runtime::lifecycle::restart_runtime(config, runtime, staged).await?;
    info!("Runtime restart completed successfully");

    if staged {
        // Only the node can say whether the runtime is serving what was written,
        // and asking needs the apiserver. The dispatcher asks before it labels.
        if !handlers.is_empty() {
            info!(
                "install (cri): leaving the check that {runtime} is serving {handlers:?} to the \
                 dispatcher, which can ask the node"
            );
        }
        return Ok(());
    }

    confirm_handlers_are_served(config, runtime, &handlers).await
}

/// Generous: the kubelet republishes this on every node status sync.
const HANDLER_WAIT_SECS: u64 = 120;

/// Fail the cri stage if the runtime came back without a single kata handler.
///
/// Restarting a unit says the unit restarted, not that it liked what we wrote.
/// A runtime that ignored our drop-in would otherwise reach the label stage and
/// leave a node advertised as kata-capable that cannot run a single kata pod.
///
/// A partial list is only reported: a node serving some of our handlers is
/// running a CRI that read our configuration, which is what this stage is
/// answerable for.
async fn confirm_handlers_are_served(
    config: &config::Config,
    runtime: &str,
    handlers: &[String],
) -> Result<()> {
    use runtime::lifecycle::HandlerReport;

    match runtime::lifecycle::kata_handlers_loaded(config, handlers, HANDLER_WAIT_SECS).await {
        HandlerReport::AllLoaded => {
            info!("install (cri): {runtime} is serving {handlers:?}");
            Ok(())
        }
        HandlerReport::Missing(missing) if missing.len() == handlers.len() => {
            anyhow::bail!(
                "{runtime} restarted but node {} reports none of {handlers:?} among its runtime \
                 handlers, so it is not serving the kata configuration this stage wrote. Check \
                 {runtime}'s logs for a rejected or unread configuration file.",
                config.node_name
            )
        }
        HandlerReport::Missing(missing) => {
            log::warn!(
                "install (cri): {runtime} is serving kata handlers, but not {missing:?}. The \
                 RuntimeClasses naming them will not be usable on node {}.",
                config.node_name
            );
            Ok(())
        }
        HandlerReport::Unknown => Ok(()),
    }
}

/// The last step of the DaemonSet install. Job mode has no such stage: there the
/// dispatcher labels the node, which is what lets the per-node Jobs run without a
/// token.
///
/// Startup taints are lifted here and nowhere else. A node can be provisioned with
/// one to keep Kata workloads away until the runtime exists, so it may only be
/// lifted after the artifacts are installed, the runtime is configured and
/// restarted, and the node is labelled.
async fn install_stage_label(config: &config::Config) -> Result<()> {
    info!("install (label): applying node label");

    // The shared label admits Kata workloads; our own mark is what this install's
    // RuntimeClasses select on top of it, and what tells another install's uninstall
    // that the shared label is not its to remove. A kubelet that clobbers either
    // leaves the node unusable, so both have to be seen to hold.
    let ours = instance_label(config.multi_install_suffix.as_deref());
    let wanted = [(KATA_RUNTIME_LABEL, "true"), (ours.as_str(), "true")];

    match k8s::get_node_labels(config).await {
        Ok(labels)
            if wanted
                .iter()
                .all(|(key, value)| labels.get(*key).map(String::as_str) == Some(*value)) =>
        {
            info!("install (label): node is already labelled, skipping");
        }
        // Any other state (absent, a different value, or a transient read error)
        // falls through to label_node_with_retry, which applies and verifies.
        _ => label_node_with_retry(config, &wanted).await?,
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
/// To outlive that race we require every label to remain at its value for
/// `STABILITY_CHECKS` consecutive observations spaced `CHECK_INTERVAL` apart
/// (≈ 15 s by default — comfortably more than the kubelet's status-update period).
/// If any of them drifts inside that window we re-apply and restart the stability
/// counter. The whole thing is bounded by `MAX_APPLY_ATTEMPTS`.
async fn label_node_with_retry(config: &config::Config, labels: &[(&str, &str)]) -> Result<()> {
    const MAX_APPLY_ATTEMPTS: u32 = 12;
    const STABILITY_CHECKS: u32 = 6;
    const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
    const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

    let described = labels
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ");

    for attempt in 1..=MAX_APPLY_ATTEMPTS {
        for (key, value) in labels {
            k8s::label_node(config, key, Some(value), true).await?;
        }
        info!(
            "Applied label(s) {} (attempt {}/{}); verifying stability ({} checks @ {}s)",
            described,
            attempt,
            MAX_APPLY_ATTEMPTS,
            STABILITY_CHECKS,
            CHECK_INTERVAL.as_secs(),
        );

        let mut stable_count: u32 = 0;
        let mut needs_reapply = false;
        while stable_count < STABILITY_CHECKS {
            tokio::time::sleep(CHECK_INTERVAL).await;

            match k8s::get_node_labels(config).await {
                Ok(observed) => {
                    let drifted = labels
                        .iter()
                        .filter(|(key, value)| {
                            observed.get(*key).map(String::as_str) != Some(value)
                        })
                        .map(|(key, _)| format!("{key}={:?}", observed.get(*key)))
                        .collect::<Vec<_>>();

                    if drifted.is_empty() {
                        stable_count += 1;
                        info!(
                            "Label(s) {} stable {}/{}",
                            described, stable_count, STABILITY_CHECKS
                        );
                        continue;
                    }

                    log::warn!(
                        "Label(s) {} drifted after {}/{} stable observation(s); re-applying \
                         (attempt {}/{})",
                        drifted.join(", "),
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
                        "Failed to verify label(s) {} during stability check \
                         (attempt {}/{}): {}; will re-apply",
                        described,
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
                "Label(s) {} confirmed stable on node after {} apply attempt(s)",
                described, attempt
            );
            return Ok(());
        }

        if attempt < MAX_APPLY_ATTEMPTS {
            tokio::time::sleep(RETRY_DELAY).await;
        }
    }

    anyhow::bail!(
        "Label(s) {} did not remain stable after {} apply attempts",
        described,
        MAX_APPLY_ATTEMPTS
    );
}

async fn cleanup(config: &config::Config, runtime: &str) -> Result<()> {
    info!("Cleaning up Kata Containers");

    info!(
        "Checking whether DaemonSet '{}' still wants a pod on this node",
        config.daemonset_name
    );
    if k8s::own_daemonset_selects_node(config).await? {
        info!(
            "DaemonSet '{}' still selects this node, skipping all cleanup to avoid disrupting a \
             rolling restart",
            config.daemonset_name
        );
        return Ok(());
    }

    info!(
        "DaemonSet '{}' is gone or no longer selects this node, proceeding with instance cleanup",
        config.daemonset_name
    );

    // Unmark before mutating the host, so nothing lands on files about to go.
    // Only the shared label waits for the last install; the restart below does
    // not - this install's configuration is leaving the disk the runtime reads.
    info!("Removing this install's node labels");
    release_node(config).await?;
    let _node_lock = acquire_node_mutation_lock()?;

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

    // Restart the CRI runtime last. On k3s/rke2 this restarts the entire
    // server process, which kills this (terminating) pod. By doing it after
    // all other cleanup, we ensure config and artifacts are already gone.
    info!("Restarting CRI runtime");
    runtime::restart_and_wait_for_ready(config, runtime, false).await?;
    info!("CRI runtime restarted successfully");

    info!("Kata Containers cleanup completed successfully");
    Ok(())
}

/// Cleanup stage 2 (revert-cri): remove CRI configuration (and any snapshotter
/// config), then restart the runtime and wait for readiness. This is the
/// privileged, node-disrupting cleanup stage and is kept short-lived. Snapshotter
/// cleanup is independent because a partial install may fail before writing CRI
/// configuration; only the restart is skipped when configuration is absent.
async fn cleanup_stage_revert_cri(
    config: &config::Config,
    runtime: &str,
    staged: bool,
) -> Result<()> {
    info!("cleanup (revert-cri): reverting CRI configuration");
    let _node_lock = acquire_node_mutation_lock()?;

    if runtime != "crio" {
        if let Some(snapshotters) = config.experimental_setup_snapshotter.as_ref() {
            for snapshotter in snapshotters {
                info!("cleanup (revert-cri): uninstalling snapshotter {snapshotter}");
                artifacts::snapshotters::uninstall_snapshotter(snapshotter, config).await?;
            }
        }
    }

    if !cri_configuration_present(config, runtime).await {
        info!("cleanup (revert-cri): CRI configuration already absent, skipping restart");
        return Ok(());
    }

    runtime::cleanup_cri_runtime_config(config, runtime).await?;

    info!("cleanup (revert-cri): restarting runtime");
    runtime::restart_and_wait_for_ready(config, runtime, staged).await?;
    info!("cleanup (revert-cri): runtime restarted");

    Ok(())
}

/// Cleanup stage 3 (remove-artifacts): delete kata artifacts/config/symlinks
/// from the host. Skips when the install directory is already gone or empty.
async fn cleanup_stage_remove_artifacts(config: &config::Config) -> Result<()> {
    info!("cleanup (remove-artifacts): removing kata artifacts from host");
    let _node_lock = acquire_node_mutation_lock()?;
    // A partial install may have loaded modules but extracted nothing.
    remove_modules_load_config(config)?;

    // The install dir is bind mounted into this pod, so it always exists and
    // outlives the artifacts it holds: an empty one means there is nothing
    // left to remove. Anything other than an absent directory is a reason to
    // stop, since we cannot tell an empty install from one we failed to read.
    let empty_or_absent = match std::fs::read_dir(&config.host_install_dir) {
        Ok(mut entries) => entries.next().is_none(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to read install dir {}", config.host_install_dir))
        }
    };
    if empty_or_absent {
        info!(
            "cleanup (remove-artifacts): install dir {} already empty, skipping",
            config.host_install_dir
        );
        return Ok(());
    }

    artifacts::remove_artifacts(config).await?;
    info!("cleanup (remove-artifacts): artifacts removed");
    Ok(())
}

/// Best-effort check for whether kata's CRI configuration is present on
/// the host for this runtime. Used by the staged cleanup to skip a disruptive
/// runtime restart when there is nothing to revert. On any uncertainty (e.g.
/// the containerd paths cannot be resolved) this returns `true` so the caller
/// errs on the side of running the revert rather than incorrectly skipping it.
async fn cri_configuration_present(config: &config::Config, runtime: &str) -> bool {
    if runtime == "crio" {
        return std::path::Path::new(&config.crio_drop_in_conf_file).exists();
    }

    match config.get_containerd_paths(runtime).await {
        Ok(paths) if paths.use_drop_in => std::path::Path::new(&paths.drop_in_file).exists(),
        // Whole-file mode leaves no drop-in to look for, so ask the question the
        // revert itself asks: is any of this configuration ours to undo?
        Ok(paths) => !matches!(
            runtime::containerd::whole_file_disposition(&paths.config_file, &paths.backup_file),
            runtime::containerd::WholeFileConfig::Keep
        ),
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

    // Nothing has been removed from the host here, so there is nothing the live
    // runtime must converge to: the shared runtime, and the kubelet with it, are
    // only bounced when this is the last install left on the node.
    if release_node(config).await? {
        info!("Skipping the CRI restart, another install is still using it");
        return Ok(());
    }

    let _node_lock = acquire_node_mutation_lock()?;
    runtime::lifecycle::restart_cri_runtime(config, runtime).await?;
    if matches!(runtime, "crio" | "containerd") {
        utils::host_systemctl(&["restart", "kubelet"]).await?;
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
    #[case("install-stage-selinux-policy", Action::InstallStageSelinuxPolicy)]
    #[case(
        "install-stage-load-kernel-modules",
        Action::InstallStageLoadKernelModules
    )]
    #[case("install-stage-host-check", Action::InstallStageHostCheck)]
    #[case("install-stage-artifacts", Action::InstallStageArtifacts)]
    #[case("install-stage-cri", Action::InstallStageCri)]
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

    #[rstest]
    #[case("install-stage")]
    #[case("cleanup-stage")]
    #[case("install-stage-foo")]
    // These two were real actions once, so an older chart can still ask for them.
    #[case("install-stage-label")]
    #[case("cleanup-stage-unlabel")]
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
    fn an_install_is_named_after_its_suffix() {
        assert_eq!(
            instance_label(None),
            "kata-deploy.katacontainers.io/default"
        );
        assert_eq!(
            instance_label(Some("dev")),
            "kata-deploy.katacontainers.io/dev"
        );
    }

    /// The shared label may only be taken away when it is nobody else's.
    #[test]
    fn only_another_installs_mark_counts() {
        let ours = instance_label(Some("dev"));
        let mark = |keys: &[&str]| -> BTreeMap<String, String> {
            keys.iter()
                .map(|key| (key.to_string(), "true".to_string()))
                .collect()
        };

        assert_eq!(
            shared_label_after(&mark(&[&ours]), &ours),
            SharedLabel::Remove
        );
        assert_eq!(
            shared_label_after(
                &mark(&[&ours, KATA_RUNTIME_LABEL, "kubernetes.io/hostname"]),
                &ours
            ),
            SharedLabel::Remove,
            "neither the shared label nor an unrelated one is another install's mark"
        );
        assert_eq!(
            shared_label_after(&mark(&[&ours, &instance_label(None)]), &ours),
            SharedLabel::Keep
        );
        assert_eq!(
            shared_label_after(&mark(&[&instance_label(Some("prod"))]), &ours),
            SharedLabel::Keep
        );
    }

    /// Installs that have claimed the node but not finished on it must not keep it
    /// advertised as able to run Kata - and must still be able to find it.
    #[test]
    fn unfinished_installs_hold_the_key_without_the_promise() {
        let ours = instance_label(Some("dev"));
        let labels = BTreeMap::from([
            (ours.clone(), "true".to_string()),
            (instance_label(None), KATA_RUNTIME_PENDING.to_string()),
        ]);

        assert_eq!(shared_label_after(&labels, &ours), SharedLabel::Demote);

        // However many of them there are.
        let labels = BTreeMap::from([
            (instance_label(None), KATA_RUNTIME_PENDING.to_string()),
            (
                instance_label(Some("prod")),
                KATA_RUNTIME_PENDING.to_string(),
            ),
        ]);

        assert_eq!(shared_label_after(&labels, &ours), SharedLabel::Demote);
    }

    /// One install serving Kata is enough to keep the shared label, however many
    /// unfinished ones are read before it - and labels are read in key order, so
    /// "default" here is read before "prod".
    #[test]
    fn a_serving_install_outweighs_unfinished_ones() {
        let ours = instance_label(Some("dev"));
        let labels = BTreeMap::from([
            (instance_label(None), KATA_RUNTIME_PENDING.to_string()),
            (instance_label(Some("prod")), "true".to_string()),
        ]);

        assert_eq!(shared_label_after(&labels, &ours), SharedLabel::Keep);
    }

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

    /// The usage text spellings are the ones erofs-utils 1.9.3 ships on both
    /// amd64 and arm64; `rfc3484_sort` is what an arm64 build folds its own
    /// `sort` into.
    #[rstest]
    #[case(b"    --mkfs-time         the t", "--mkfs-time", true)]
    #[case(b"ta\n --sort=<path,none>  ", "--sort", true)]
    #[case(b"\0rfc3484_sort\0", "--sort", false)]
    #[case(b"\0qsort\0", "--sort", false)]
    #[case(b"\0--mkfs-timestamp\0", "--mkfs-time", false)]
    #[case(b"\0--sort", "--sort", true)]
    #[case(b"1.7.1\0", "--sort", false)]
    // A word ending in the option is no more a mention of it than `qsort` is.
    #[case(b"\0x--sort\0", "--sort", false)]
    #[case(b"\0no_--mkfs-time\0", "--mkfs-time", false)]
    // Nothing before it to look at.
    #[case(b"--sort=<path,none>", "--sort", true)]
    fn test_documents_option(#[case] binary: &[u8], #[case] option: &str, #[case] expected: bool) {
        assert_eq!(documents_option(binary, option), expected);
    }

    /// All non-internal staged actions remain visible in `--help` so operators
    /// can discover and run individual stages.
    #[rstest]
    #[case(Action::InstallStageHostCheck)]
    #[case(Action::InstallStageLoadKernelModules)]
    #[case(Action::InstallStageArtifacts)]
    #[case(Action::InstallStageCri)]
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

    fn module_names(modules: &[HostModule]) -> Vec<&str> {
        modules.iter().map(|module| module.name).collect()
    }

    fn test_module_plan(
        arch: &str,
        cpuinfo: &str,
        shims: &[&str],
        custom_bases: &[&str],
        erofs_enabled: bool,
        erofs_dmverity: bool,
    ) -> Result<HostModulePlan> {
        host_modules_for_install(
            arch,
            cpuinfo,
            shims,
            custom_bases,
            erofs_enabled,
            erofs_dmverity,
        )
    }

    fn test_host_modules(
        arch: &str,
        cpuinfo: &str,
        shims: &[&str],
        custom_bases: &[&str],
        erofs_enabled: bool,
        erofs_dmverity: bool,
    ) -> Result<Vec<HostModule>> {
        test_module_plan(
            arch,
            cpuinfo,
            shims,
            custom_bases,
            erofs_enabled,
            erofs_dmverity,
        )
        .map(|plan| plan.modules)
    }

    #[rstest]
    #[case("GenuineIntel", "kvm_intel")]
    #[case("AuthenticAMD", "kvm_amd")]
    fn x86_module_selection_follows_cpu_vendor(#[case] vendor: &str, #[case] vendor_module: &str) {
        let modules = test_host_modules("x86_64", vendor, &["qemu"], &[], false, false).unwrap();
        assert_eq!(
            module_names(&modules),
            vec!["kvm", vendor_module, "vhost", "vhost_net", "vhost_vsock"]
        );
    }

    #[test]
    fn x86_asks_for_a_backend_and_treats_the_kvm_modules_as_optional() {
        let plan =
            test_module_plan("x86_64", "GenuineIntel", &["qemu"], &[], false, false).unwrap();
        assert!(plan.needs_x86_virtualization);
        for name in ["kvm", "kvm_intel"] {
            let module = plan
                .modules
                .iter()
                .find(|module| module.name == name)
                .expect("the KVM modules are still attempted");
            assert!(!module.required);
        }
    }

    /// An unnameable vendor may just mean a Hyper-V root partition, where no KVM
    /// module would have loaded anyway.
    #[test]
    fn unknown_x86_vendor_leaves_the_backend_check_to_decide() {
        let plan =
            test_module_plan("x86_64", "UnknownVendor", &["qemu"], &[], false, false).unwrap();
        assert!(plan.needs_x86_virtualization);
        assert_eq!(
            module_names(&plan.modules),
            vec!["kvm", "vhost", "vhost_net", "vhost_vsock"]
        );
    }

    #[rstest]
    #[case(
        "aarch64",
        vec!["kvm", "vhost", "vhost_net", "vhost_vsock"]
    )]
    #[case("riscv64", vec!["kvm", "vhost", "vhost_net", "vhost_vsock"])]
    #[case("powerpc64", vec!["kvm", "kvm_hv", "vhost_vsock"])]
    #[case("s390x", vec!["kvm", "vhost_vsock"])]
    fn non_x86_module_selection_matches_kata_check(
        #[case] arch: &str,
        #[case] expected: Vec<&str>,
    ) {
        let modules = test_host_modules(arch, "", &["qemu"], &[], false, false).unwrap();
        assert_eq!(module_names(&modules), expected);
    }

    #[test]
    fn remote_only_install_needs_no_virtualization_at_all() {
        for plan in [
            test_module_plan("x86_64", "", &["remote"], &[], false, false).unwrap(),
            test_module_plan("x86_64", "", &[], &["remote"], false, false).unwrap(),
        ] {
            assert!(plan.modules.is_empty());
            assert!(!plan.needs_x86_virtualization);
        }
    }

    #[test]
    fn local_custom_runtime_requests_virtualization_modules() {
        let modules = test_host_modules(
            "x86_64",
            "GenuineIntel",
            &["remote"],
            &["qemu"],
            false,
            false,
        )
        .unwrap();
        assert!(module_names(&modules).contains(&"kvm_intel"));
    }

    #[rstest]
    #[case(&["qemu-runtime-rs"], true)]
    #[case(&["clh-azure-runtime-rs"], false)]
    #[case(&["clh", "qemu"], true)]
    fn vhost_vsock_is_only_required_where_the_host_device_is_used(
        #[case] shims: &[&str],
        #[case] expected_required: bool,
    ) {
        let modules =
            test_host_modules("x86_64", "GenuineIntel", shims, &[], false, false).unwrap();
        let vsock = modules
            .iter()
            .find(|module| module.name == "vhost_vsock")
            .expect("vhost_vsock is always considered");
        assert_eq!(vsock.required, expected_required);
    }

    #[test]
    fn erofs_features_are_selected_and_deduplicated() {
        let modules = test_host_modules("aarch64", "", &["qemu"], &[], true, true).unwrap();
        let names = module_names(&modules);
        assert!(names.ends_with(&["erofs", "loop", "dm_mod", "dm_verity"]));
        assert_eq!(
            names.iter().copied().collect::<HashSet<_>>().len(),
            names.len()
        );
        assert!(modules.iter().all(|module| module.required));
    }

    #[test]
    fn modules_load_config_is_per_install() {
        let base = std::path::Path::new("/etc/modules-load.d");
        assert_eq!(
            modules_load_config_path(base, None).unwrap(),
            base.join("kata-containers-default.conf")
        );
        assert_eq!(
            modules_load_config_path(base, Some("dev")).unwrap(),
            base.join("kata-containers-dev.conf")
        );
        assert!(modules_load_config_path(base, Some("../escape")).is_err());

        let content = modules_load_config_content(&["kvm", "vhost_vsock"]);
        assert!(content.starts_with("# Managed by kata-deploy"));
        assert!(content.contains("\nkvm\nvhost_vsock\n"));
    }

    #[test]
    fn required_module_failures_abort_but_optional_failures_do_not() {
        assert!(handle_module_load_failure(
            HostModule::required("vhost_vsock"),
            anyhow::anyhow!("no vhost_vsock")
        )
        .is_err());
        assert!(handle_module_load_failure(
            HostModule::optional("fsverity"),
            anyhow::anyhow!("no fsverity")
        )
        .is_ok());
    }

    /// /bin/sh exists here but not on the fake host, so only a resolution that
    /// escapes the root answers true for it.
    #[rstest]
    #[case::absolute_symlink("/bin/kmod", true)]
    #[case::relative_symlink("../bin/kmod", true)]
    #[case::escapes_the_host_root("/bin/sh", false)]
    #[case::dangling("/bin/nowhere", false)]
    fn host_symlinks_resolve_inside_the_host_root(#[case] target: &str, #[case] expected: bool) {
        let host = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(host.path().join("usr/sbin")).expect("usr/sbin");
        std::fs::create_dir_all(host.path().join("usr/bin")).expect("usr/bin");
        std::fs::write(host.path().join("usr/bin/kmod"), b"#!/bin/sh\n").expect("kmod");
        // usrmerge, so /bin/kmod lands on usr/bin/kmod.
        std::os::unix::fs::symlink("usr/bin", host.path().join("bin")).expect("bin symlink");
        std::os::unix::fs::symlink(target, host.path().join("usr/sbin/modprobe"))
            .expect("modprobe symlink");

        assert_eq!(
            host_path_is_file(host.path(), std::path::Path::new("/usr/sbin/modprobe")),
            expected,
            "modprobe -> {target}"
        );
    }

    #[test]
    fn a_missing_host_path_is_not_a_file() {
        let host = tempfile::tempdir().expect("tempdir");
        assert!(!host_path_is_file(
            host.path(),
            std::path::Path::new("/usr/sbin/modprobe")
        ));
    }

    #[test]
    fn a_symlink_loop_under_the_host_root_terminates() {
        let host = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(host.path().join("usr/sbin")).expect("usr/sbin");
        std::os::unix::fs::symlink("/usr/sbin/b", host.path().join("usr/sbin/a")).expect("a");
        std::os::unix::fs::symlink("/usr/sbin/a", host.path().join("usr/sbin/b")).expect("b");

        assert!(!host_path_is_file(
            host.path(),
            std::path::Path::new("/usr/sbin/a")
        ));
    }
}
