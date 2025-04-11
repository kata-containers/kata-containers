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

use anyhow::{anyhow, Context, Result};
use cfg_if::cfg_if;
use clap::{AppSettings, Parser};
use const_format::{concatcp, formatcp};
use nix::fcntl::OFlag;
use nix::sys::reboot::{reboot, RebootMode};
use nix::sys::socket::{self, AddressFamily, SockFlag, SockType, VsockAddr};
use nix::unistd::{self, dup, sync, Pid};
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::os::unix::fs::{self as unixfs, FileTypeExt};
use std::os::unix::io::AsRawFd;
use std::path::Path;
use std::process::exit;
use std::process::Command;
use std::sync::Arc;
use tracing::{instrument, span};

mod cdh;
mod config;
mod console;
mod device;
mod features;
mod linux_abi;
mod metrics;
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

#[cfg(feature = "guest-pull")]
mod image;

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

const AA_PATH: &str = "/usr/local/bin/attestation-agent";
const AA_ATTESTATION_SOCKET: &str =
    "/run/confidential-containers/attestation-agent/attestation-agent.sock";
const AA_ATTESTATION_URI: &str = concatcp!(UNIX_SOCKET_PREFIX, AA_ATTESTATION_SOCKET);

const CDH_PATH: &str = "/usr/local/bin/confidential-data-hub";
const CDH_SOCKET: &str = "/run/confidential-containers/cdh.sock";
const CDH_SOCKET_URI: &str = concatcp!(UNIX_SOCKET_PREFIX, CDH_SOCKET);

const API_SERVER_PATH: &str = "/usr/local/bin/api-server-rest";

/// Path of ocicrypt config file. This is used by image-rs when decrypting image.
const OCICRYPT_CONFIG_PATH: &str = "/run/confidential-containers/ocicrypt_config.json";

const OCICRYPT_CONFIG: &str = formatcp!(
    r#"{{
    "key-providers": {{
        "attestation-agent": {{
            "ttrpc": "{}"
        }}
    }}
}}"#,
    CDH_SOCKET_URI
);

const DEFAULT_LAUNCH_PROCESS_TIMEOUT: i32 = 6;

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

#[derive(Parser)]
// The default clap version info doesn't match our form, so we need to override it
#[clap(global_setting(AppSettings::DisableVersionFlag))]
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
        socket::bind(listenfd, &addr)?;
        socket::listen(listenfd, 1)?;

        Box::new(util::get_vsock_stream(listenfd).await?)
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
        let newwfd = dup(wfd)?;
        let writer = unsafe { File::from_raw_fd(newwfd) };

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

    let log_handle = tokio::spawn(create_logger_task(rfd, log_vport, shutdown_rx.clone()));

    tasks.push(log_handle);

    let writer = unsafe { File::from_raw_fd(wfd) };

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

    eprintln!("{} shutdown complete", NAME);

    let mut wait_errors: Vec<tokio::task::JoinError> = vec![];
    for result in results {
        if let Err(e) = result {
            eprintln!("wait task error: {:#?}", e);
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

    #[cfg(feature = "guest-pull")]
    image::set_proxy_env_vars().await;

    #[cfg(feature = "agent-policy")]
    if let Err(e) = initialize_policy().await {
        error!(logger, "Failed to initialize agent policy: {:?}", e);
        // Continuing execution without a security policy could be dangerous.
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

    let gc_procs = config.guest_components_procs;
    if !attestation_binaries_available(logger, &gc_procs) {
        warn!(
            logger,
            "attestation binaries requested for launch not available"
        );
    } else {
        init_attestation_components(logger, config).await?;
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

// Check if required attestation binaries are available on the rootfs.
fn attestation_binaries_available(logger: &Logger, procs: &GuestComponentsProcs) -> bool {
    let binaries = match procs {
        GuestComponentsProcs::AttestationAgent => vec![AA_PATH],
        GuestComponentsProcs::ConfidentialDataHub => vec![AA_PATH, CDH_PATH],
        GuestComponentsProcs::ApiServerRest => vec![AA_PATH, CDH_PATH, API_SERVER_PATH],
        _ => vec![],
    };
    for binary in binaries.iter() {
        if !Path::new(binary).exists() {
            warn!(logger, "{} not found", binary);
            return false;
        }
    }
    true
}

async fn launch_guest_component_procs(logger: &Logger, config: &AgentConfig) -> Result<()> {
    if config.guest_components_procs == GuestComponentsProcs::None {
        return Ok(());
    }

    debug!(logger, "spawning attestation-agent process {}", AA_PATH);
    launch_process(
        logger,
        AA_PATH,
        &vec!["--attestation_sock", AA_ATTESTATION_URI],
        AA_ATTESTATION_SOCKET,
        DEFAULT_LAUNCH_PROCESS_TIMEOUT,
    )
    .map_err(|e| anyhow!("launch_process {} failed: {:?}", AA_PATH, e))?;

    // skip launch of confidential-data-hub and api-server-rest
    if config.guest_components_procs == GuestComponentsProcs::AttestationAgent {
        return Ok(());
    }

    debug!(
        logger,
        "spawning confidential-data-hub process {}", CDH_PATH
    );

    launch_process(
        logger,
        CDH_PATH,
        &vec![],
        CDH_SOCKET,
        DEFAULT_LAUNCH_PROCESS_TIMEOUT,
    )
    .map_err(|e| anyhow!("launch_process {} failed: {:?}", CDH_PATH, e))?;

    // skip launch of api-server-rest
    if config.guest_components_procs == GuestComponentsProcs::ConfidentialDataHub {
        return Ok(());
    }

    let features = config.guest_components_rest_api;
    debug!(
        logger,
        "spawning api-server-rest process {} --features {}", API_SERVER_PATH, features
    );
    launch_process(
        logger,
        API_SERVER_PATH,
        &vec!["--features", &features.to_string()],
        "",
        0,
    )
    .map_err(|e| anyhow!("launch_process {} failed: {:?}", API_SERVER_PATH, e))?;

    Ok(())
}

// Start-up attestation-agent, CDH and api-server-rest if they are packaged in the rootfs
// and the corresponding procs are enabled in the agent configuration. the process will be
// launched in the background and the function will return immediately.
// If the CDH is started, a CDH client will be instantiated and returned.
async fn init_attestation_components(logger: &Logger, config: &AgentConfig) -> Result<()> {
    launch_guest_component_procs(logger, config).await?;

    // If a CDH socket exists, initialize the CDH client and enable ocicrypt
    match tokio::fs::metadata(CDH_SOCKET).await {
        Ok(md) => {
            if md.file_type().is_socket() {
                cdh::init_cdh_client(CDH_SOCKET_URI).await?;
                fs::write(OCICRYPT_CONFIG_PATH, OCICRYPT_CONFIG.as_bytes())?;
                env::set_var("OCICRYPT_KEYPROVIDER_CONFIG", OCICRYPT_CONFIG_PATH);
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

fn wait_for_path_to_exist(logger: &Logger, path: &str, timeout_secs: i32) -> Result<()> {
    let p = Path::new(path);
    let mut attempts = 0;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
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

fn launch_process(
    logger: &Logger,
    path: &str,
    args: &Vec<&str>,
    unix_socket_path: &str,
    timeout_secs: i32,
) -> Result<()> {
    if !Path::new(path).exists() {
        return Err(anyhow!("path {} does not exist.", path));
    }
    if !unix_socket_path.is_empty() && Path::new(unix_socket_path).exists() {
        fs::remove_file(unix_socket_path)?;
    }
    Command::new(path).args(args).spawn()?;
    if !unix_socket_path.is_empty() && timeout_secs > 0 {
        wait_for_path_to_exist(logger, unix_socket_path, timeout_secs)?;
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
use std::os::unix::io::{FromRawFd, RawFd};

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
                result: Err(anyhow!(nix::errno::Errno::from_i32(libc::EACCES))),
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

            let msg = format!("test[{}]: {:?}", i, d);
            let (rfd, wfd) = unistd::pipe2(OFlag::O_CLOEXEC).unwrap();
            defer!({
                // XXX: Never try to close rfd, because it will be closed by PipeStream in
                // create_logger_task() and it's not safe to close the same fd twice time.
                unistd::close(wfd).unwrap();
            });

            let (shutdown_tx, shutdown_rx) = channel(true);

            shutdown_tx.send(true).unwrap();
            let result = create_logger_task(rfd, d.vsock_port, shutdown_rx).await;

            let msg = format!("{}, result: {:?}", msg, result);
            assert_result!(d.result, result, msg);
        }
    }
}
