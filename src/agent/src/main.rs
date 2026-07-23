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

// FR-14: network phase state machine. Network-mutating RPCs are permitted only during
// sandbox setup; once a workload container starts the network surface is frozen. Strict
// builds only; agent-internal.
#[cfg(feature = "strict-policy")]
lazy_static! {
    static ref NET_PHASE: Mutex<kata_security_reference_monitor::NetworkPhaseMachine> =
        Mutex::new(kata_security_reference_monitor::NetworkPhaseMachine::new());
}

// FR-4C: measured allowlist of authorized read-only layer (dm-verity) root digests. The
// storage handler authorizes each dm-verity layer's (algorithm, root_hash) against this
// store before creating the verity device — the Kata analogue of runhcs
// EnforceDeviceMountPolicy(target, RootDigest). Configured from measured state; when
// verification is required but no layer is authorized, every layer is rejected (fail-closed).
// Strict builds only.
#[cfg(feature = "strict-policy")]
lazy_static! {
    pub(crate) static ref VERIFIED_LAYERS: Mutex<kata_security_reference_monitor::VerifiedLayerStore> =
        Mutex::new(kata_security_reference_monitor::VerifiedLayerStore::new(false));
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

    // FR-7: the interactive debug console is an un-mediated shell into the guest and is
    // never available in a strict confidential build, regardless of host configuration.
    #[cfg(feature = "strict-policy")]
    let debug_console_enabled = false;
    #[cfg(not(feature = "strict-policy"))]
    let debug_console_enabled = config.debug_console;

    if debug_console_enabled {
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

    // FR-1b: seed the policy-fragment trust root (authorized issuers, per-issuer SVN floor,
    // receipt requirement) from measured guest state. Absent config ⇒ no authorized issuer
    // ⇒ every fragment is rejected (fail-closed).
    #[cfg(feature = "strict-policy")]
    if let Err(e) = seed_fragment_trust_root(logger).await {
        warn!(logger, "FR-1: fragment trust root not seeded: {:?}", e);
    }

    // FR-4C: seed the verified-layer allowlist (authorized dm-verity root digests) from
    // measured guest state. When require_verified_layers is set but no layer is authorized,
    // every read-only layer is rejected (fail-closed).
    #[cfg(feature = "strict-policy")]
    if let Err(e) = seed_verified_layers(logger).await {
        warn!(logger, "FR-4C: verified layers not seeded: {:?}", e);
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

// FR-1b: measured guest path listing the authorized policy-fragment issuers. It lives in
// the measured rootfs; overridable via KATA_FRAGMENT_ISSUERS for tests. Format (TOML):
//   require_receipt = true
//   [[issuer]]
//   id = "issuerA"
//   ed25519_pubkey_hex = "<64 hex chars>"
//   min_svn = 5
#[cfg(feature = "strict-policy")]
const FRAGMENT_ISSUERS_PATH: &str = "/etc/kata/fragment-issuers.toml";

// FR-1i: runtime SVN high-water state, persisted so an agent restart cannot reopen a
// rollback window. Must live on sealed/encrypted-scratch storage (in a confidential guest
// the writable scratch is memory-/disk-encrypted). Overridable via KATA_FRAGMENT_SVN_STATE.
#[cfg(feature = "strict-policy")]
const FRAGMENT_SVN_STATE_PATH: &str = "/run/kata/fragment-svn.state";

#[cfg(feature = "strict-policy")]
fn fragment_svn_state_path() -> String {
    std::env::var("KATA_FRAGMENT_SVN_STATE").unwrap_or_else(|_| FRAGMENT_SVN_STATE_PATH.to_string())
}

// FR-1i: write the exported SVN snapshot to the persistence path (best-effort).
#[cfg(feature = "strict-policy")]
pub(crate) fn persist_fragment_svn_state(snapshot: &str) {
    let path = fragment_svn_state_path();
    if let Some(dir) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&path, snapshot);
}

#[cfg(feature = "strict-policy")]
#[derive(serde::Deserialize, Default)]
struct FragmentTrustConfig {
    #[serde(default)]
    require_receipt: Option<bool>,
    /// FR-1f: legacy single transparency anchor public key (hex); mapped to the default
    /// ledger. Prefer `[[ledger]]` for multi-ledger / rotation.
    #[serde(default)]
    transparency_anchor_hex: Option<String>,
    /// FR-1f (trust list): the Transparency Trust List — named ledgers with rotatable keys.
    #[serde(default)]
    ledger: Vec<FragmentLedgerConfig>,
    /// FR-1d: require every fragment to carry a valid did:x509 chain (no raw-key path).
    #[serde(default)]
    require_x509: Option<bool>,
    /// FR-1d: measured certificate revocation list (SHA-256 fingerprints, hex).
    #[serde(default)]
    revoked: Vec<String>,
    /// FR-1d: authorized did:x509 CA anchors.
    #[serde(default, rename = "ca_anchor")]
    ca_anchor: Vec<FragmentCaAnchorConfig>,
    /// FR-1j: enable append-only application ordering (the log-head gate). Opt-in.
    #[serde(default)]
    ordered: Option<bool>,
    /// FR-1j: the measured ordering-log genesis (hex). Defaults to a fixed constant when
    /// `ordered` is true and this is unset.
    #[serde(default)]
    log_genesis_hex: Option<String>,
    #[serde(default)]
    issuer: Vec<FragmentIssuerConfig>,
}

#[cfg(feature = "strict-policy")]
#[derive(serde::Deserialize)]
struct FragmentCaAnchorConfig {
    /// The did:x509 issuer id this anchor authorizes (must equal a fragment's issuer).
    did: String,
    /// SHA-256 fingerprint (hex) of the trusted CA certificate DER. One of this or
    /// `ca_cert_pem` must be set.
    #[serde(default)]
    ca_fingerprint_hex: Option<String>,
    /// PEM of the trusted CA certificate (its fingerprint is derived). Alternative to
    /// `ca_fingerprint_hex`.
    #[serde(default)]
    ca_cert_pem: Option<String>,
    /// did:x509 policy over the leaf: required subject Common Name.
    #[serde(default)]
    require_subject_cn: Option<String>,
    /// did:x509 policy: required leaf Extended Key Usage OIDs (dotted).
    #[serde(default)]
    require_eku: Vec<String>,
    /// did:x509 policy: required leaf DNS SubjectAltName entries.
    #[serde(default)]
    require_san_dns: Vec<String>,
}

#[cfg(feature = "strict-policy")]
#[derive(serde::Deserialize)]
struct FragmentLedgerConfig {
    id: String,
    /// One or more current Ed25519 verification keys for this ledger (multiple ⇒ rotation).
    #[serde(default)]
    pubkey_hex: Vec<String>,
    /// BL-2: additional non-Ed25519 keys (ES256/ES384/PS256/RS256), each a SubjectPublicKeyInfo
    /// DER in hex plus its COSE algorithm name.
    #[serde(default)]
    key: Vec<FragmentLedgerKeyConfig>,
}

#[cfg(feature = "strict-policy")]
#[derive(serde::Deserialize)]
struct FragmentLedgerKeyConfig {
    /// COSE algorithm: "eddsa" | "es256" | "es384" | "ps256" | "rs256".
    alg: String,
    /// SubjectPublicKeyInfo DER (hex) for the ledger key.
    spki_hex: String,
}

#[cfg(feature = "strict-policy")]
#[derive(serde::Deserialize)]
struct FragmentIssuerConfig {
    id: String,
    ed25519_pubkey_hex: String,
    #[serde(default)]
    min_svn: u64,
    /// FR-1f (trust list): ledgers a receipt for this issuer's default feed must come from
    /// (policy-driven required_receipts). Non-empty ⇒ a receipt is mandatory.
    #[serde(default)]
    required_receipt_from: Vec<String>,
    /// FR-1f (trust list): ledgers allowed to back receipts for this issuer's default feed.
    #[serde(default)]
    allowed_ledgers: Vec<String>,
    /// FR-1e: named feeds this issuer may publish, with their SVN floor.
    #[serde(default)]
    feed: Vec<FragmentFeedConfig>,
}

#[cfg(feature = "strict-policy")]
#[derive(serde::Deserialize)]
struct FragmentFeedConfig {
    name: String,
    #[serde(default)]
    min_svn: u64,
    /// FR-1f (trust list): ledgers a receipt for this feed must come from.
    #[serde(default)]
    required_receipt_from: Vec<String>,
    /// FR-1f (trust list): ledgers allowed to back receipts for this feed.
    #[serde(default)]
    allowed_ledgers: Vec<String>,
}

#[cfg(feature = "strict-policy")]
fn decode_hex32(s: &str) -> Result<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 {
        anyhow::bail!("ed25519 pubkey must be 64 hex chars, got {}", s.len());
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
            .map_err(|e| anyhow::anyhow!("invalid hex in pubkey: {e}"))?;
    }
    Ok(out)
}

// FR-1j: decode an arbitrary-length hex string (e.g. the ordering-log genesis).
#[cfg(feature = "strict-policy")]
fn decode_hex_vec(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        anyhow::bail!("hex string has odd length: {}", s.len());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| anyhow::anyhow!("invalid hex: {e}"))
        })
        .collect()
}

// FR-1b: configure the global fragment store from measured state. Absent/empty config
// leaves the store with no authorized issuers (fail-closed).
#[cfg(feature = "strict-policy")]
async fn seed_fragment_trust_root(logger: &Logger) -> Result<()> {
    let path = std::env::var("KATA_FRAGMENT_ISSUERS")
        .unwrap_or_else(|_| FRAGMENT_ISSUERS_PATH.to_string());
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => {
            info!(logger, "FR-1: no fragment-issuer config; fragments fail-closed");
            return Ok(());
        }
    };
    let cfg: FragmentTrustConfig = toml::from_str(&text).context("parse fragment-issuers.toml")?;

    let mut store = FRAGMENTS.lock().await;
    if let Some(rr) = cfg.require_receipt {
        // Rebuild with the configured receipt requirement, preserving fail-closed default.
        *store = kata_security_reference_monitor::FragmentStore::new(rr);
    }
    // FR-1f: configure the transparency anchor (receipts cryptographically verified).
    if let Some(anchor_hex) = &cfg.transparency_anchor_hex {
        let key = decode_hex32(anchor_hex).context("transparency anchor key")?;
        store
            .set_transparency_anchor(&key)
            .map_err(|e| anyhow::anyhow!("set transparency anchor: {}", e))?;
        info!(logger, "FR-1: transparency anchor configured (default ledger)");
    }
    // FR-1f (trust list): load named ledgers with rotatable keys.
    if !cfg.ledger.is_empty() {
        let mut entries: Vec<(String, Vec<[u8; 32]>)> = Vec::with_capacity(cfg.ledger.len());
        for l in &cfg.ledger {
            let mut keys = Vec::with_capacity(l.pubkey_hex.len());
            for k in &l.pubkey_hex {
                keys.push(decode_hex32(k).with_context(|| format!("ledger {} key", l.id))?);
            }
            entries.push((l.id.clone(), keys));
        }
        store
            .load_transparency_trust_list(&entries)
            .map_err(|e| anyhow::anyhow!("load transparency trust list: {}", e))?;
        // BL-2: additional non-Ed25519 ledger keys (ES256/ES384/PS256/RS256).
        for l in &cfg.ledger {
            for k in &l.key {
                let alg = match k.alg.trim().to_ascii_lowercase().as_str() {
                    "eddsa" | "ed25519" => kata_security_reference_monitor::cose_keys::CoseAlg::EdDsa,
                    "es256" => kata_security_reference_monitor::cose_keys::CoseAlg::Es256,
                    "es384" => kata_security_reference_monitor::cose_keys::CoseAlg::Es384,
                    "ps256" => kata_security_reference_monitor::cose_keys::CoseAlg::Ps256,
                    "rs256" => kata_security_reference_monitor::cose_keys::CoseAlg::Rs256,
                    other => anyhow::bail!("ledger {} unsupported key alg {}", l.id, other),
                };
                let der = decode_hex_vec(&k.spki_hex)
                    .with_context(|| format!("ledger {} spki_hex", l.id))?;
                let pk = kata_security_reference_monitor::cose_keys::PublicKey::from_spki_der(&der)
                    .ok_or_else(|| anyhow::anyhow!("ledger {} invalid SPKI key", l.id))?;
                store.add_ledger_key(l.id.clone(), pk, alg);
            }
        }
        info!(logger, "FR-1: transparency trust list loaded"; "ledgers" => cfg.ledger.len());
    }
    // FR-1d: did:x509 issuer identity — require_x509, revocation list, CA anchors.
    if let Some(rx) = cfg.require_x509 {
        store.set_require_x509(rx);
    }
    if !cfg.revoked.is_empty() {
        let mut fps = Vec::with_capacity(cfg.revoked.len());
        for hexfp in &cfg.revoked {
            fps.push(decode_hex32(hexfp).context("revoked cert fingerprint")?);
        }
        store.set_revoked_certs(fps);
        info!(logger, "FR-1: revocation list loaded"; "revoked" => cfg.revoked.len());
    }
    for ca in &cfg.ca_anchor {
        let ca_fingerprint = if let Some(hexfp) = &ca.ca_fingerprint_hex {
            decode_hex32(hexfp).with_context(|| format!("ca_anchor {} fingerprint", ca.did))?
        } else if let Some(pem) = &ca.ca_cert_pem {
            kata_security_reference_monitor::did_x509::ca_fingerprint_from_pem(pem)
                .map_err(|e| anyhow::anyhow!("ca_anchor {} pem: {}", ca.did, e))?
        } else {
            anyhow::bail!("ca_anchor {} needs ca_fingerprint_hex or ca_cert_pem", ca.did);
        };
        store.authorize_did_x509(kata_security_reference_monitor::DidX509Anchor {
            did: ca.did.clone(),
            ca_fingerprint,
            policy: kata_security_reference_monitor::DidX509Policy {
                require_subject_cn: ca.require_subject_cn.clone(),
                require_eku: ca.require_eku.clone(),
                require_san_dns: ca.require_san_dns.clone(),
            },
        });
        info!(logger, "FR-1: authorized did:x509 anchor"; "did" => &ca.did);
    }
    for issuer in &cfg.issuer {
        let key = decode_hex32(&issuer.ed25519_pubkey_hex)
            .with_context(|| format!("issuer {}", issuer.id))?;
        store
            .authorize_issuer(issuer.id.clone(), &key)
            .map_err(|e| anyhow::anyhow!("authorize issuer {}: {}", issuer.id, e))?;
        store.set_min_svn(issuer.id.clone(), issuer.min_svn);
        // FR-1f (trust list): default-feed receipt scoping for this issuer.
        if !issuer.allowed_ledgers.is_empty() {
            store.set_allowed_ledgers(issuer.id.clone(), "", &issuer.allowed_ledgers);
        }
        if !issuer.required_receipt_from.is_empty() {
            store.require_receipt_for(issuer.id.clone(), "", &issuer.required_receipt_from);
        }
        // FR-1e: declare named feeds for this issuer.
        for feed in &issuer.feed {
            store.declare_feed(issuer.id.clone(), feed.name.clone(), feed.min_svn);
            // FR-1f (trust list): per-feed receipt scoping.
            if !feed.allowed_ledgers.is_empty() {
                store.set_allowed_ledgers(issuer.id.clone(), feed.name.clone(), &feed.allowed_ledgers);
            }
            if !feed.required_receipt_from.is_empty() {
                store.require_receipt_for(
                    issuer.id.clone(),
                    feed.name.clone(),
                    &feed.required_receipt_from,
                );
            }
        }
        info!(logger, "FR-1: authorized fragment issuer";
            "issuer" => &issuer.id, "min-svn" => issuer.min_svn, "feeds" => issuer.feed.len());
    }

    // FR-1j: enable append-only application ordering (before importing persisted state so
    // the restored head is not overwritten by the genesis).
    if cfg.ordered.unwrap_or(false) {
        let genesis = if let Some(hex) = &cfg.log_genesis_hex {
            decode_hex_vec(hex).context("log_genesis_hex")?
        } else {
            b"kata-fragment-log/v1".to_vec()
        };
        store.set_log_genesis(&genesis);
        info!(logger, "FR-1: append-only fragment ordering enabled (FR-1j)");
    }

    // FR-1i: re-import any persisted SVN high-water marks so a restart keeps rollback
    // protection (import can only raise the floor, never lower it). FR-1j: this also
    // restores the ordering log head (raise-only) across restart.
    if let Ok(snapshot) = std::fs::read_to_string(fragment_svn_state_path()) {
        store.import_svn_state(&snapshot);
        info!(logger, "FR-1: imported persisted fragment SVN state");
    }
    Ok(())
}

// FR-4C: measured guest path listing the authorized read-only layer (dm-verity) root
// digests. It lives in the measured rootfs; overridable via KATA_VERIFIED_LAYERS for tests.
// Format (TOML):
//   require_verified_layers = true
//   [[layer]]
//   algorithm = "sha256"
//   root_hash = "<hex>"
#[cfg(feature = "strict-policy")]
const VERIFIED_LAYERS_PATH: &str = "/etc/kata/verified-layers.toml";

#[cfg(feature = "strict-policy")]
#[derive(serde::Deserialize, Default)]
struct VerifiedLayersConfig {
    /// When true, every dm-verity read-only layer must be in the allowlist (fail-closed).
    #[serde(default)]
    require_verified_layers: Option<bool>,
    #[serde(default)]
    layer: Vec<VerifiedLayerConfig>,
}

#[cfg(feature = "strict-policy")]
#[derive(serde::Deserialize)]
struct VerifiedLayerConfig {
    /// dm-verity hash algorithm (e.g. "sha256"). Defaults to "sha256".
    #[serde(default = "default_layer_algorithm")]
    algorithm: String,
    /// dm-verity root hash (hex).
    root_hash: String,
}

#[cfg(feature = "strict-policy")]
fn default_layer_algorithm() -> String {
    "sha256".to_string()
}

// FR-4C: configure the verified-layer allowlist from measured state. Absent/empty config
// leaves verification not required (opt-in); when require_verified_layers is set but no
// layer is authorized, every read-only layer is rejected (fail-closed).
#[cfg(feature = "strict-policy")]
async fn seed_verified_layers(logger: &Logger) -> Result<()> {
    let path =
        std::env::var("KATA_VERIFIED_LAYERS").unwrap_or_else(|_| VERIFIED_LAYERS_PATH.to_string());
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => {
            info!(logger, "FR-4C: no verified-layers config; layer verification off");
            return Ok(());
        }
    };
    let cfg: VerifiedLayersConfig =
        toml::from_str(&text).context("parse verified-layers.toml")?;

    let mut store = VERIFIED_LAYERS.lock().await;
    if let Some(req) = cfg.require_verified_layers {
        store.set_require(req);
    }
    for layer in &cfg.layer {
        store.authorize_layer(&layer.algorithm, &layer.root_hash);
    }
    info!(logger, "FR-4C: verified-layer allowlist configured";
        "required" => store.is_required(), "layers" => store.len());
    Ok(())
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
