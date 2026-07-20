// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate lazy_static;
extern crate capctl;
extern crate prometheus;
extern crate protocols;
extern crate regex;
extern crate scan_fmt;
extern crate serde_json;

#[macro_use]
extern crate scopeguard;

#[macro_use]
extern crate slog;

use anyhow::{anyhow, bail, Context, Result};
use cfg_if::cfg_if;
use clap::Parser;
use const_format::concatcp;
use initdata::{InitdataReturnValue, AA_CONFIG_PATH, CDH_CONFIG_PATH};
use nix::fcntl::OFlag;
use nix::sys::reboot::{reboot, RebootMode};
use nix::sys::socket::{self, AddressFamily, SockFlag, SockType, VsockAddr};
use nix::unistd::{self, dup, sync, Pid};
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::ErrorKind;
use std::os::unix::fs::{self as unixfs, FileTypeExt};
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
use std::path::Path;
use std::process::exit;
use std::sync::Arc;
use tracing::{instrument, span};

mod confidential_data_hub;
mod config;
mod console;
mod device;
mod features;
mod guest_extension_image;
mod initdata;
mod linux_abi;
mod metrics;
mod mediation;
mod mount;
mod namespace;
mod netlink;
mod network;
mod passfd_io;
mod pci;
pub mod random;
mod sandbox;
mod signal;
mod storage;
mod uevent;
mod util;
mod version;
mod watcher;

use config::GuestComponentsProcs;
use mount::{cgroups_mount, general_mount};
use sandbox::Sandbox;
use signal::setup_signal_handler;
use slog::{debug, error, info, o, warn, Logger};
use uevent::watch_uevents;

use futures::future::join_all;
use rustjail::pipestream::PipeStream;
use tokio::{
    io::AsyncWrite,
    sync::{
        watch::{channel, Receiver},
        Mutex,
    },
    task::JoinHandle,
};

mod rpc;
mod tracer;

#[cfg(feature = "agent-policy")]
mod policy;

cfg_if! {
    if #[cfg(target_arch = "s390x")] {
        mod ap;
        mod ccw;
    }
}

const NAME: &str = "kata-agent";

const UNIX_SOCKET_PREFIX: &str = "unix://";

// Legacy (non-extension) rootfs locations for the CoCo guest components. They are
// used to build the built-in launch plan when no CoCo extension image is mounted,
// keeping monolithic / non-confidential images working unchanged.
const AA_PATH: &str = "/usr/local/bin/attestation-agent";
const AA_ATTESTATION_SOCKET: &str =
    "/run/confidential-containers/attestation-agent/attestation-agent.sock";
const AA_ATTESTATION_URI: &str = concatcp!(UNIX_SOCKET_PREFIX, AA_ATTESTATION_SOCKET);

const CDH_PATH: &str = "/usr/local/bin/confidential-data-hub";
const CDH_SOCKET: &str = "/run/confidential-containers/cdh.sock";
const CDH_SOCKET_URI: &str = concatcp!(UNIX_SOCKET_PREFIX, CDH_SOCKET);

const API_SERVER_PATH: &str = "/usr/local/bin/api-server-rest";

const OCICRYPT_CONFIG_PATH: &str = "/etc/ocicrypt_config.json";

lazy_static! {
    static ref AGENT_CONFIG: AgentConfig =
        // Note: We can't do AgentOpts.parse() here to send through the processed arguments to AgentConfig
        // clap::Parser::parse() greedily process all command line input including cargo test parameters,
        // so should only be used inside main.
        AgentConfig::from_cmdline("/proc/cmdline", env::args().collect()).unwrap();
}

#[cfg(feature = "agent-policy")]
lazy_static! {
    static ref AGENT_POLICY: Mutex<AgentPolicy> = Mutex::new(AgentPolicy::new());
}

// FR-6: the Security Reference Monitor tracks each security-relevant, state-mutating
// operation as a two-phase transaction (prepare/execute/commit/abort) so policy and
// runtime state commit together or are rolled back/quarantined. Present only in strict
// builds; it is agent-internal and introduces no new shim<->agent API.
#[cfg(feature = "strict-policy")]
lazy_static! {
    static ref SRM: Mutex<kata_security_reference_monitor::ReferenceMonitor> =
        Mutex::new(kata_security_reference_monitor::ReferenceMonitor::new());
}

// FR-9: registry of container occurrences and their lifecycle states. The host
// container_id is an untrusted alias; the enforcer mints its own occurrence handle and
// gates every lifecycle-mutating RPC on the occurrence state. Strict builds only;
// agent-internal, no new shim<->agent API.
#[cfg(feature = "strict-policy")]
lazy_static! {
    static ref OCCURRENCES: Mutex<kata_security_reference_monitor::OccurrenceRegistry> =
        Mutex::new(kata_security_reference_monitor::OccurrenceRegistry::new());
}

// FR-1: verifier/accumulator for signed, add-only policy fragments. Receipts are enforced
// in strict builds. Authorized issuers and root constraints are configured from measured
// state; absent configuration, no issuer is trusted (fail-closed). Strict builds only.
#[cfg(feature = "strict-policy")]
lazy_static! {
    static ref FRAGMENTS: Mutex<kata_security_reference_monitor::FragmentStore> =
        Mutex::new(kata_security_reference_monitor::FragmentStore::new(true));
}

#[derive(Parser)]
// The default clap version info doesn't match our form, so we need to override it
#[clap(disable_version_flag = true)]
struct AgentOpts {
    /// Print the version information
    #[clap(short, long)]
    version: bool,
    #[clap(subcommand)]
    subcmd: Option<SubCommand>,
    /// Specify a custom agent config file
    #[clap(short, long)]
    config: Option<String>,
}

#[derive(Parser)]
enum SubCommand {
    Init {},
}

#[instrument]
fn announce(logger: &Logger, config: &AgentConfig) {
    let extra_features = features::get_build_features();

    info!(logger, "announce";
    "agent-commit" => version::VERSION_COMMIT,
    "agent-version" =>  version::AGENT_VERSION,
    "api-version" => version::API_VERSION,
    "config" => format!("{:?}", config),
    "extra-features" => format!("{extra_features:?}"),
    );
}

// Create a thread to handle reading from the logger pipe. The thread will
// output to the vsock port specified, or stdout.
async fn create_logger_task(rfd: RawFd, vsock_port: u32, shutdown: Receiver<bool>) -> Result<()> {
    let mut reader = PipeStream::from_fd(rfd);
    let mut writer: Box<dyn AsyncWrite + Unpin + Send> = if vsock_port > 0 {
        let listenfd = socket::socket(
            AddressFamily::Vsock,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )?;

        let addr = VsockAddr::new(libc::VMADDR_CID_ANY, vsock_port);
        socket::bind(listenfd.as_raw_fd(), &addr)?;
        socket::listen(&listenfd, nix::sys::socket::Backlog::new(1).unwrap())?;

        Box::new(util::get_vsock_stream(listenfd.into_raw_fd()).await?)
    } else {
        Box::new(tokio::io::stdout())
    };

    let _ = util::interruptable_io_copier(&mut reader, &mut writer, shutdown).await;

    Ok(())
}

async fn real_main(init_mode: bool) -> std::result::Result<(), Box<dyn std::error::Error>> {
    env::set_var("RUST_BACKTRACE", "full");

    // List of tasks that need to be stopped for a clean shutdown
    let mut tasks: Vec<JoinHandle<Result<()>>> = vec![];

    console::initialize();

    // support vsock log
    let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC)?;

    let (shutdown_tx, shutdown_rx) = channel(true);

    if init_mode {
        // dup a new file descriptor for this temporary logger writer,
        // since this logger would be dropped and it's writer would
        // be closed out of this code block.
        let newwfd = dup(&wfd)?;
        let writer = unsafe { File::from_raw_fd(newwfd.into_raw_fd()) };

        // Init a temporary logger used by init agent as init process
        // since before do the base mount, it wouldn't access "/proc/cmdline"
        // to get the customzied debug level.
        let (logger, logger_async_guard) =
            logging::create_logger(NAME, "agent", slog::Level::Debug, writer);

        // Must mount proc fs before parsing kernel command line
        general_mount(&logger).map_err(|e| {
            error!(logger, "fail general mount: {}", e);
            e
        })?;

        lazy_static::initialize(&AGENT_CONFIG);
        let cgroup_v2 = AGENT_CONFIG.unified_cgroup_hierarchy || AGENT_CONFIG.cgroup_no_v1 == "all";

        init_agent_as_init(&logger, cgroup_v2)?;
        drop(logger_async_guard);
    } else {
        lazy_static::initialize(&AGENT_CONFIG);
    }

    let config = &AGENT_CONFIG;
    let log_vport = config.log_vport as u32;

    let log_handle = tokio::spawn(create_logger_task(
        rfd.into_raw_fd(),
        log_vport,
        shutdown_rx.clone(),
    ));

    tasks.push(log_handle);

    let writer = unsafe { File::from_raw_fd(wfd.into_raw_fd()) };

    // Recreate a logger with the log level get from "/proc/cmdline".
    let (logger, logger_async_guard) =
        logging::create_logger(NAME, "agent", config.log_level, writer);

    announce(&logger, config);

    // This variable is required as it enables the global (and crucially static) logger,
    // which is required to satisfy the the lifetime constraints of the auto-generated gRPC code.
    let global_logger = slog_scope::set_global_logger(logger.new(o!("subsystem" => "rpc")));

    // Allow the global logger to be modified later (for shutdown)
    global_logger.cancel_reset();

    let mut ttrpc_log_guard: Result<(), log::SetLoggerError> = Ok(());

    if config.log_level == slog::Level::Trace {
        // Redirect ttrpc log calls to slog iff full debug requested
        ttrpc_log_guard = Ok(slog_stdlog::init()?);
    }

    if config.tracing {
        tracer::setup_tracing(NAME, &logger)?;
    }

    let root_span = span!(tracing::Level::TRACE, "root-span");

    // XXX: Start the root trace transaction.
    //
    // XXX: Note that *ALL* spans needs to start after this point!!
    let span_guard = root_span.enter();

    // Start the fd passthrough io listener
    let passfd_listener_port = config.passfd_listener_port as u32;
    if passfd_listener_port != 0 {
        passfd_io::start_listen(passfd_listener_port).await?;
    }

    // Start the sandbox and wait for its ttRPC server to end
    start_sandbox(&logger, config, init_mode, &mut tasks, shutdown_rx.clone()).await?;

    // Install a NOP logger for the remainder of the shutdown sequence
    // to ensure any log calls made by local crates using the scope logger
    // don't fail.
    let global_logger_guard2 =
        slog_scope::set_global_logger(slog::Logger::root(slog::Discard, o!()));
    global_logger_guard2.cancel_reset();

    drop(logger_async_guard);

    drop(ttrpc_log_guard);

    // Trigger a controlled shutdown
    shutdown_tx
        .send(true)
        .map_err(|e| anyhow!(e).context("failed to request shutdown"))?;

    // Wait for all threads to finish
    let results = join_all(tasks).await;

    // force flushing spans
    drop(span_guard);
    drop(root_span);

    if config.tracing {
        tracer::end_tracing();
    }

    eprintln!("{NAME} shutdown complete");

    let mut wait_errors: Vec<tokio::task::JoinError> = vec![];
    for result in results {
        if let Err(e) = result {
            eprintln!("wait task error: {e:#?}");
            wait_errors.push(e);
        }
    }

    if wait_errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow!("wait all tasks failed: {:#?}", wait_errors).into())
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args = AgentOpts::parse();

    if args.version {
        let extra_features = features::get_build_features();

        println!(
            "{} version {} (api version: {}, commit version: {}, type: rust, extra-features: {extra_features:?})",
            NAME,
            version::AGENT_VERSION,
            version::API_VERSION,
            version::VERSION_COMMIT,
        );
        exit(0);
    }

    if let Some(SubCommand::Init {}) = args.subcmd {
        reset_sigpipe();
        rustjail::container::init_child();
        exit(0);
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let init_mode = unistd::getpid() == Pid::from_raw(1);
    let result = rt.block_on(real_main(init_mode));

    if init_mode {
        sync();
        let _ = reboot(RebootMode::RB_POWER_OFF);
    }

    result
}

#[instrument]
async fn start_sandbox(
    logger: &Logger,
    config: &AgentConfig,
    init_mode: bool,
    tasks: &mut Vec<JoinHandle<Result<()>>>,
    shutdown: Receiver<bool>,
) -> Result<()> {
    let debug_console_vport = config.debug_console_vport as u32;

    if config.debug_console {
        let debug_console_task = tokio::task::spawn(console::debug_console_handler(
            logger.clone(),
            debug_console_vport,
            shutdown.clone(),
        ));

        tasks.push(debug_console_task);
    }

    // Initialize unique sandbox structure.
    let s = Sandbox::new(logger).context("Failed to create sandbox")?;
    if init_mode {
        s.rtnl.handle_localhost().await?;
    }

    #[cfg(feature = "agent-policy")]
    if let Err(e) = initialize_policy().await {
        error!(logger, "Failed to initialize agent policy: {:?}", e);
        // Continuing execution without a security policy could be dangerous.
        // Give a brief moment for the logs to flush, then abort the process to stop the VM.
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        std::process::abort();
    }

    let sandbox = Arc::new(Mutex::new(s));

    let signal_handler_task = tokio::spawn(setup_signal_handler(
        logger.clone(),
        sandbox.clone(),
        shutdown.clone(),
    ));

    tasks.push(signal_handler_task);

    let uevents_handler_task = tokio::spawn(watch_uevents(sandbox.clone(), shutdown.clone()));

    tasks.push(uevents_handler_task);

    let (tx, rx) = tokio::sync::oneshot::channel();
    sandbox.lock().await.sender = Some(tx);

    let initdata_return_value = initdata::initialize_initdata(logger).await?;

    let gc_procs = config.guest_components_procs;
    let launch_plan = build_coco_launch_plan(config, &initdata_return_value, gc_procs)?;
    if !attestation_components_available(logger, &launch_plan) {
        warn!(
            logger,
            "attestation binaries requested for launch not available"
        );
    } else {
        init_attestation_components(logger, &launch_plan).await?;
    }

    // if policy is given via initdata, use it
    #[cfg(feature = "agent-policy")]
    if let Some(initdata_return_value) = initdata_return_value {
        if let Some(policy) = &initdata_return_value._policy {
            info!(logger, "using policy from initdata");
            AGENT_POLICY
                .lock()
                .await
                .set_policy(policy)
                .await
                .context("Failed to set policy from initdata")?;
        }
    }

    let mut oma = None;
    let mut _ort = None;
    if let Some(c) = &config.mem_agent {
        let (ma, rt) =
            mem_agent::agent::MemAgent::new(c.memcg_config.clone(), c.compact_config.clone())
                .map_err(|e| {
                    error!(logger, "MemAgent::new fail: {}", e);
                    e
                })
                .context("start mem-agent")?;
        oma = Some(ma);
        _ort = Some(rt);
    }

    // vsock:///dev/vsock, port
    let mut server =
        rpc::start(sandbox.clone(), config.server_addr.as_str(), init_mode, oma).await?;

    server.start().await?;

    rx.await?;
    server.shutdown().await?;

    Ok(())
}

// Map the requested guest-components level to the numeric gating level used by
// extension manifests. A process is launched only when its declared `level` is
// <= this value. The ordering mirrors the implications documented on
// `GuestComponentsProcs` (ApiServerRest implies CDH implies AttestationAgent).
fn guest_components_max_level(procs: GuestComponentsProcs) -> u32 {
    match procs {
        GuestComponentsProcs::None => 0,
        GuestComponentsProcs::AttestationAgent => 1,
        GuestComponentsProcs::ConfidentialDataHub => 2,
        GuestComponentsProcs::ApiServerRest => 3,
    }
}

// Build the substitution context exposed to extension manifests. New extension bundles
// can rely on these variables without requiring agent code changes; introducing
// a brand new variable is the only case that needs touching the agent.
fn build_substitution_ctx(
    config: &AgentConfig,
    initdata_return_value: &Option<InitdataReturnValue>,
) -> Result<std::collections::HashMap<String, String>> {
    let ocicrypt_config_path = guest_extension_image::resolve_component_path(
        guest_extension_image::COCO_EXTENSION_NAME,
        guest_extension_image::COCO_COMPONENT_OCICRYPT_CONFIG,
        OCICRYPT_CONFIG_PATH,
    )?;

    let initdata_toml_path = if initdata_return_value.is_some() {
        initdata::INITDATA_TOML_PATH.to_string()
    } else {
        String::new()
    };

    let extension_root =
        guest_extension_image::extension_mount_root(guest_extension_image::COCO_EXTENSION_NAME)?;

    let mut ctx = std::collections::HashMap::new();
    ctx.insert(
        "aa_attestation_uri".to_string(),
        AA_ATTESTATION_URI.to_string(),
    );
    ctx.insert(
        "aa_attestation_socket".to_string(),
        AA_ATTESTATION_SOCKET.to_string(),
    );
    ctx.insert("aa_config_path".to_string(), AA_CONFIG_PATH.to_string());
    ctx.insert("cdh_config_path".to_string(), CDH_CONFIG_PATH.to_string());
    ctx.insert("cdh_socket".to_string(), CDH_SOCKET.to_string());
    ctx.insert(
        "ocicrypt_config_path".to_string(),
        ocicrypt_config_path.to_string_lossy().into_owned(),
    );
    ctx.insert(
        "rest_api_features".to_string(),
        config.guest_components_rest_api.to_string(),
    );
    ctx.insert(
        "launch_process_timeout".to_string(),
        config.launch_process_timeout.as_secs().to_string(),
    );
    ctx.insert("initdata_toml_path".to_string(), initdata_toml_path);
    ctx.insert(
        "extension_root".to_string(),
        extension_root.to_string_lossy().into_owned(),
    );
    // The CoCo extension ships several attestation-agent flavours and selects one
    // via the manifest's "attester_variant". The guest init (NVRC) owns that
    // decision: with a GPU present it sets KATA_ATTESTER_VARIANT=nvidia so the
    // NVIDIA-attester build launches (it emits the GPU evidence a KBS GPU
    // policy requires). Absent that signal we fall back to the stock attester.
    // Cross-component contract: the env var name and "nvidia" value are set by
    // NVRC (src/kata_agent.rs, src/gpu.rs); keep them in sync.
    let attester_variant = env::var("KATA_ATTESTER_VARIANT")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "default".to_string());
    ctx.insert("attester_variant".to_string(), attester_variant);

    Ok(ctx)
}

// Built-in launch plan used when no CoCo extension image is mounted. It reproduces
// the legacy behaviour of launching the guest components from the rootfs
// (`/usr/local/bin/...`), so monolithic and non-confidential images are
// unaffected by the extension machinery.
fn builtin_coco_plan(
    config: &AgentConfig,
    initdata_return_value: &Option<InitdataReturnValue>,
    max_level: u32,
) -> Vec<guest_extension_image::LaunchSpec> {
    let mut plan = Vec::new();

    if max_level >= 1 {
        let mut args = vec![
            "--attestation_sock".to_string(),
            AA_ATTESTATION_URI.to_string(),
        ];
        if initdata_return_value.is_some() {
            args.push("--initdata-toml".to_string());
            args.push(initdata::INITDATA_TOML_PATH.to_string());
        }
        plan.push(guest_extension_image::LaunchSpec {
            id: "attestation-agent".to_string(),
            path: Path::new(AA_PATH).to_path_buf(),
            args,
            config: Some(AA_CONFIG_PATH.to_string()),
            env: vec![],
            wait_socket: Some(AA_ATTESTATION_SOCKET.to_string()),
            timeout_secs: config.launch_process_timeout.as_secs(),
        });
    }

    if max_level >= 2 {
        plan.push(guest_extension_image::LaunchSpec {
            id: "confidential-data-hub".to_string(),
            path: Path::new(CDH_PATH).to_path_buf(),
            args: vec![],
            config: Some(CDH_CONFIG_PATH.to_string()),
            env: vec![(
                "OCICRYPT_KEYPROVIDER_CONFIG".to_string(),
                OCICRYPT_CONFIG_PATH.to_string(),
            )],
            wait_socket: Some(CDH_SOCKET.to_string()),
            timeout_secs: config.launch_process_timeout.as_secs(),
        });
    }

    if max_level >= 3 {
        plan.push(guest_extension_image::LaunchSpec {
            id: "api-server-rest".to_string(),
            path: Path::new(API_SERVER_PATH).to_path_buf(),
            args: vec![
                "--features".to_string(),
                config.guest_components_rest_api.to_string(),
            ],
            config: None,
            env: vec![],
            wait_socket: None,
            timeout_secs: 0,
        });
    }

    plan
}

// Build the ordered launch plan for the guest components. When a CoCo extension
// image is mounted its manifest drives the plan (so new bundles need no agent
// changes); otherwise the built-in legacy plan is used.
fn build_coco_launch_plan(
    config: &AgentConfig,
    initdata_return_value: &Option<InitdataReturnValue>,
    procs: GuestComponentsProcs,
) -> Result<Vec<guest_extension_image::LaunchSpec>> {
    let max_level = guest_components_max_level(procs);
    let ctx = build_substitution_ctx(config, initdata_return_value)?;
    match guest_extension_image::launch_plan(
        guest_extension_image::COCO_EXTENSION_NAME,
        max_level,
        &ctx,
    )? {
        Some(plan) => Ok(plan),
        None => Ok(builtin_coco_plan(config, initdata_return_value, max_level)),
    }
}

// Check that every process in the launch plan is present on disk. A missing
// binary means the components were not provisioned (e.g. a non-confidential
// rootfs), in which case launching is skipped.
fn attestation_components_available(
    logger: &Logger,
    plan: &[guest_extension_image::LaunchSpec],
) -> bool {
    for spec in plan {
        let exists = spec
            .path
            .try_exists()
            .unwrap_or_else(|error| match error.kind() {
                ErrorKind::NotFound => false,
                _ => panic!(
                    "Path existence check failed for '{}': {}",
                    spec.path.display(),
                    error
                ),
            });

        if !exists {
            warn!(logger, "{} not found", spec.path.display());
            return false;
        }
    }
    true
}

async fn launch_guest_component_procs(
    logger: &Logger,
    plan: &[guest_extension_image::LaunchSpec],
) -> Result<()> {
    for spec in plan {
        let path = spec
            .path
            .to_str()
            .ok_or_else(|| anyhow!("non-utf8 component path {}", spec.path.display()))?;
        debug!(logger, "spawning extension component process {}", spec.id);

        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        let envs: Vec<(&str, &str)> = spec
            .env
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        launch_process(
            logger,
            path,
            args,
            spec.config.as_deref(),
            spec.wait_socket.as_deref().unwrap_or(""),
            spec.timeout_secs,
            &envs,
        )
        .await
        .map_err(|e| anyhow!("launch_process {} failed: {:?}", path, e))?;
    }

    Ok(())
}

// Start-up attestation-agent, CDH and api-server-rest if they are packaged in the rootfs
// and the corresponding procs are enabled in the agent configuration. the process will be
// launched in the background and the function will return immediately.
// If the CDH is started, a CDH client will be instantiated and returned.
async fn init_attestation_components(
    logger: &Logger,
    plan: &[guest_extension_image::LaunchSpec],
) -> Result<()> {
    launch_guest_component_procs(logger, plan).await?;

    // If a CDH socket exists, initialize the CDH client and enable ocicrypt
    match tokio::fs::metadata(CDH_SOCKET).await {
        Ok(md) => {
            if md.file_type().is_socket() {
                confidential_data_hub::init_cdh_client(CDH_SOCKET_URI).await?;
            } else {
                debug!(logger, "File {} is not a socket", CDH_SOCKET);
            }
        }
        Err(err) => warn!(
            logger,
            "Failed to probe CDH socket file {}: {:?}", CDH_SOCKET, err
        ),
    }

    Ok(())
}

async fn wait_for_path_to_exist(logger: &Logger, path: &str, timeout_secs: u64) -> Result<()> {
    let p = Path::new(path);
    let mut attempts = 0;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if p.exists() {
            return Ok(());
        }
        if attempts >= timeout_secs {
            break;
        }
        attempts += 1;
        info!(
            logger,
            "waiting for {} to exist (attempts={})", path, attempts
        );
    }

    Err(anyhow!("wait for {} to exist timeout.", path))
}

async fn launch_process(
    logger: &Logger,
    path: &str,
    mut args: Vec<&str>,
    config: Option<&str>,
    unix_socket_path: &str,
    timeout_secs: u64,
    envs: &[(&str, &str)],
) -> Result<()> {
    if !Path::new(path).exists() {
        bail!("path {} does not exist.", path);
    }

    if let Some(config_path) = config {
        if Path::new(config_path).exists() {
            args.push("-c");
            args.push(config_path);
        }
    }

    if !unix_socket_path.is_empty() && Path::new(unix_socket_path).exists() {
        tokio::fs::remove_file(unix_socket_path).await?;
    }

    let mut process = tokio::process::Command::new(path);
    process.args(args);
    for (k, v) in envs {
        process.env(k, v);
    }
    process.spawn()?;
    if !unix_socket_path.is_empty() && timeout_secs > 0 {
        wait_for_path_to_exist(logger, unix_socket_path, timeout_secs).await?;
    }

    Ok(())
}

// init_agent_as_init will do the initializations such as setting up the rootfs
// when this agent has been run as the init process.
fn init_agent_as_init(logger: &Logger, unified_cgroup_hierarchy: bool) -> Result<()> {
    cgroups_mount(logger, unified_cgroup_hierarchy).map_err(|e| {
        error!(
            logger,
            "fail cgroups mount, unified_cgroup_hierarchy {}: {}", unified_cgroup_hierarchy, e
        );
        e
    })?;

    fs::remove_file(Path::new("/dev/ptmx"))?;
    unixfs::symlink(Path::new("/dev/pts/ptmx"), Path::new("/dev/ptmx"))?;

    unistd::setsid()?;

    unsafe {
        libc::ioctl(std::io::stdin().as_raw_fd(), libc::TIOCSCTTY, 1);
    }

    env::set_var("PATH", "/bin:/sbin/:/usr/bin/:/usr/sbin/");

    let contents =
        std::fs::read_to_string("/etc/hostname").unwrap_or_else(|_| String::from("localhost"));
    let contents_array: Vec<&str> = contents.split(' ').collect();
    let hostname = contents_array[0].trim();

    if unistd::sethostname(OsStr::new(hostname)).is_err() {
        warn!(logger, "failed to set hostname");
    }

    Ok(())
}

#[cfg(feature = "agent-policy")]
async fn initialize_policy() -> Result<()> {
    AGENT_POLICY
        .lock()
        .await
        .initialize(
            AGENT_CONFIG.log_level.as_usize(),
            AGENT_CONFIG.policy_file.clone(),
            None,
        )
        .await
}

// The Rust standard library had suppressed the default SIGPIPE behavior,
// see https://github.com/rust-lang/rust/pull/13158.
// Since the parent's signal handler would be inherited by it's child process,
// thus we should re-enable the standard SIGPIPE behavior as a workaround to
// fix the issue of https://github.com/kata-containers/kata-containers/issues/1887.
fn reset_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

use crate::config::AgentConfig;
use std::os::unix::io::RawFd;

#[cfg(feature = "agent-policy")]
use kata_agent_policy::policy::AgentPolicy;

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::TestUserType;
    use test_utils::{assert_result, skip_if_not_root, skip_if_root};

    #[tokio::test]
    async fn test_create_logger_task() {
        #[derive(Debug)]
        struct TestData {
            vsock_port: u32,
            test_user: TestUserType,
            result: Result<()>,
        }

        let tests = &[
            TestData {
                // non-root user cannot use privileged vsock port
                vsock_port: 1,
                test_user: TestUserType::NonRootOnly,
                result: Err(anyhow!(nix::errno::Errno::from_raw(libc::EACCES))),
            },
            TestData {
                // passing vsock_port 0 causes logger task to write to stdout
                vsock_port: 0,
                test_user: TestUserType::Any,
                result: Ok(()),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            if d.test_user == TestUserType::RootOnly {
                skip_if_not_root!();
            } else if d.test_user == TestUserType::NonRootOnly {
                skip_if_root!();
            }

            let msg = format!("test[{i}]: {d:?}");
            let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC).unwrap();
            let rfd_raw = rfd.as_raw_fd();
            let wfd_raw = wfd.as_raw_fd();
            // Prevent OwnedFd from closing the fds when dropped
            std::mem::forget(rfd);
            std::mem::forget(wfd);
            defer!({
                // XXX: Never try to close rfd, because it will be closed by PipeStream in
                // create_logger_task() and it's not safe to close the same fd twice time.
                unistd::close(wfd_raw).unwrap();
            });

            let (shutdown_tx, shutdown_rx) = channel(true);

            shutdown_tx.send(true).unwrap();
            let result = create_logger_task(rfd_raw, d.vsock_port, shutdown_rx).await;

            let msg = format!("{msg}, result: {result:?}");
            assert_result!(d.result, result, msg);
        }
    }
}
