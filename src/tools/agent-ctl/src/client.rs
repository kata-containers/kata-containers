// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Description: Client side of ttRPC comms

use crate::types::*;
use crate::utils;
use anyhow::{anyhow, Result};
use byteorder::ByteOrder;
use nix::sys::socket::{connect, socket, AddressFamily, SockAddr, SockFlag, SockType, UnixAddr};
use protocols::agent::*;
use protocols::agent_ttrpc::*;
use protocols::health::*;
use protocols::health_ttrpc::*;
use slog::{debug, info};
use std::convert::TryFrom;
use std::fs;
use std::io::Write; // XXX: for flush()
use std::io::{self, Read, Seek, SeekFrom};
use std::io::{BufRead, BufReader};
use std::os::unix::io::{IntoRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::thread::sleep;
use std::time::Duration;
use ttrpc::context::Context;

// Run the specified closure to set an automatic value if the ttRPC Context
// does not contain the special values requesting automatic values be
// suppressed.
macro_rules! run_if_auto_values {
    ($ctx:expr, $closure:expr) => {{
        let cfg = $ctx.metadata.get(METADATA_CFG_NS);

        if let Some(v) = cfg {
            if v.contains(&AUTO_VALUES_CFG_NAME.to_string()) {
                debug!(sl!(), "Running closure to generate values");

                $closure()?;
            }
        }
    }};
}

// Hack until the actual Context type supports this.
fn clone_context(ctx: &Context) -> Context {
    Context {
        metadata: ctx.metadata.clone(),
        timeout_nano: ctx.timeout_nano,
    }
}

// Agent command handler type
//
// Notes:
//
// - 'cmdline' is the command line (command name and optional space separate
//   arguments).
// - 'options' can be read and written to, allowing commands to pass state to
//   each other via well-known option names.
type AgentCmdFp = fn(
    ctx: &Context,
    client: &AgentServiceClient,
    health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()>;

// Builtin command handler type
type BuiltinCmdFp = fn(args: &str) -> (Result<()>, bool);

enum ServiceType {
    Agent,
    Health,
}

// XXX: Agent command names *MUST* start with an upper-case letter.
struct AgentCmd {
    name: &'static str,
    st: ServiceType,
    fp: AgentCmdFp,
}

// XXX: Builtin command names *MUST* start with a lower-case letter.
struct BuiltinCmd {
    name: &'static str,
    descr: &'static str,
    fp: BuiltinCmdFp,
}

// Command that causes the agent to exit (iff tracing is enabled)
const SHUTDOWN_CMD: &str = "DestroySandbox";

// Command that requests this program ends
const CMD_QUIT: &str = "quit";
const CMD_REPEAT: &str = "repeat";

const DEFAULT_PROC_SIGNAL: &str = "SIGKILL";

const ERR_API_FAILED: &str = "API failed";

// Value used as a "namespace" in the ttRPC Context's metadata.
const METADATA_CFG_NS: &str = "agent-ctl-cfg";

// Special value which if found means generate any values
// automatically.
const AUTO_VALUES_CFG_NAME: &str = "auto-values";

static AGENT_CMDS: &[AgentCmd] = &[
    AgentCmd {
        name: "AddARPNeighbors",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_add_arp_neighbors,
    },
    AgentCmd {
        name: "AddSwap",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_add_swap,
    },
    AgentCmd {
        name: "Check",
        st: ServiceType::Health,
        fp: agent_cmd_health_check,
    },
    AgentCmd {
        name: "Version",
        st: ServiceType::Health,
        fp: agent_cmd_health_version,
    },
    AgentCmd {
        name: "CloseStdin",
        st: ServiceType::Agent,
        fp: agent_cmd_container_close_stdin,
    },
    AgentCmd {
        name: "CopyFile",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_copy_file,
    },
    AgentCmd {
        name: "CreateContainer",
        st: ServiceType::Agent,
        fp: agent_cmd_container_create,
    },
    AgentCmd {
        name: "CreateSandbox",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_create,
    },
    AgentCmd {
        name: "DestroySandbox",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_destroy,
    },
    AgentCmd {
        name: "ExecProcess",
        st: ServiceType::Agent,
        fp: agent_cmd_container_exec,
    },
    AgentCmd {
        name: "GetGuestDetails",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_get_guest_details,
    },
    AgentCmd {
        name: "GetIptables",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_get_ip_tables,
    },
    AgentCmd {
        name: "GetMetrics",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_get_metrics,
    },
    AgentCmd {
        name: "GetOOMEvent",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_get_oom_event,
    },
    AgentCmd {
        name: "GetVolumeStats",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_get_volume_stats,
    },
    AgentCmd {
        name: "ListInterfaces",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_list_interfaces,
    },
    AgentCmd {
        name: "ListRoutes",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_list_routes,
    },
    AgentCmd {
        name: "MemHotplugByProbe",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_mem_hotplug_by_probe,
    },
    AgentCmd {
        name: "OnlineCPUMem",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_online_cpu_mem,
    },
    AgentCmd {
        name: "PauseContainer",
        st: ServiceType::Agent,
        fp: agent_cmd_container_pause,
    },
    AgentCmd {
        name: "ReadStderr",
        st: ServiceType::Agent,
        fp: agent_cmd_container_read_stderr,
    },
    AgentCmd {
        name: "ReadStdout",
        st: ServiceType::Agent,
        fp: agent_cmd_container_read_stdout,
    },
    AgentCmd {
        name: "ReseedRandomDev",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_reseed_random_dev,
    },
    AgentCmd {
        name: "RemoveContainer",
        st: ServiceType::Agent,
        fp: agent_cmd_container_remove,
    },
    AgentCmd {
        name: "ResumeContainer",
        st: ServiceType::Agent,
        fp: agent_cmd_container_resume,
    },
    AgentCmd {
        name: "SetGuestDateTime",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_set_guest_date_time,
    },
    AgentCmd {
        name: "SetIptables",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_set_ip_tables,
    },
    AgentCmd {
        name: "SignalProcess",
        st: ServiceType::Agent,
        fp: agent_cmd_container_signal_process,
    },
    AgentCmd {
        name: "StartContainer",
        st: ServiceType::Agent,
        fp: agent_cmd_container_start,
    },
    AgentCmd {
        name: "StatsContainer",
        st: ServiceType::Agent,
        fp: agent_cmd_container_stats,
    },
    AgentCmd {
        name: "TtyWinResize",
        st: ServiceType::Agent,
        fp: agent_cmd_container_tty_win_resize,
    },
    AgentCmd {
        name: "UpdateContainer",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_update_container,
    },
    AgentCmd {
        name: "UpdateInterface",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_update_interface,
    },
    AgentCmd {
        name: "UpdateRoutes",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_update_routes,
    },
    AgentCmd {
        name: "WaitProcess",
        st: ServiceType::Agent,
        fp: agent_cmd_container_wait_process,
    },
    AgentCmd {
        name: "WriteStdin",
        st: ServiceType::Agent,
        fp: agent_cmd_container_write_stdin,
    },
    AgentCmd {
        name: "SetPolicy",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_set_policy,
    },
    AgentCmd {
        name: "MemAgentMemcgSet",
        st: ServiceType::Agent,
        fp: agent_cmd_mem_agent_memcg_set,
    },
    AgentCmd {
        name: "MemAgentCompactSet",
        st: ServiceType::Agent,
        fp: agent_cmd_mem_agent_compact_set,
    },
];

static BUILTIN_CMDS: & [BuiltinCmd] = &[
    BuiltinCmd {
        name: "echo",
        descr: "Display the arguments",
        fp: builtin_cmd_echo,
    },
    BuiltinCmd {
        name: "help",
        descr: "Alias for 'list'",
        fp: builtin_cmd_list,
    },
    BuiltinCmd {
        name: "list",
        descr: "List all available commands",
        fp: builtin_cmd_list,
    },
    BuiltinCmd {
        name: "repeat",
        descr: "Repeat the next command 'n' times [-1 for forever]",
        fp: builtin_cmd_repeat,
    },
    BuiltinCmd {
        name: "sleep",
        descr:
            "Pause for specified period number of nanoseconds (supports human-readable suffixes [no floating points numbers])",
        fp: builtin_cmd_sleep,
    },
    BuiltinCmd {
        name: CMD_QUIT,
        descr: "Exit this program",
        fp: builtin_cmd_quit,
    },
];

fn get_agent_cmd_names() -> Vec<String> {
    let mut names = Vec::new();

    for cmd in AGENT_CMDS {
        names.push(cmd.name.to_string());
    }

    names
}

fn get_agent_cmd_details() -> Vec<String> {
    let mut cmds = Vec::new();

    for cmd in AGENT_CMDS {
        let service = match cmd.st {
            ServiceType::Agent => "agent",
            ServiceType::Health => "health",
        };

        cmds.push(format!("{} ({} service)", cmd.name, service));
    }

    cmds
}

fn get_agent_cmd_func(name: &str) -> Result<AgentCmdFp> {
    for cmd in AGENT_CMDS {
        if cmd.name.eq(name) {
            return Ok(cmd.fp);
        }
    }

    Err(anyhow!("Invalid command: {:?}", name))
}

fn get_builtin_cmd_details() -> Vec<String> {
    let mut cmds = Vec::new();

    for cmd in BUILTIN_CMDS {
        cmds.push(format!("{} ({})", cmd.name, cmd.descr));
    }

    cmds
}

fn get_all_cmd_details() -> Vec<String> {
    let mut cmds = get_builtin_cmd_details();

    cmds.append(&mut get_agent_cmd_names());

    cmds
}

fn get_builtin_cmd_func(name: &str) -> Result<BuiltinCmdFp> {
    for cmd in BUILTIN_CMDS {
        if cmd.name.eq(name) {
            return Ok(cmd.fp);
        }
    }

    Err(anyhow!("Invalid command: {:?}", name))
}

fn client_create_vsock_fd(cid: libc::c_uint, port: u32) -> Result<RawFd> {
    let fd = socket(
        AddressFamily::Vsock,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC,
        None,
    )
    .map_err(|e| anyhow!(e))?;

    let sock_addr = SockAddr::new_vsock(cid, port);

    connect(fd, &sock_addr).map_err(|e| anyhow!(e))?;

    Ok(fd)
}

// Setup the existing stream by making a Hybrid VSOCK host-initiated
// connection request to the Hybrid VSOCK-capable hypervisor (CLH or FC),
// asking it to route the connection to the Kata Agent running inside the VM.
fn setup_hybrid_vsock(mut stream: &UnixStream, hybrid_vsock_port: u64) -> Result<()> {
    // Challenge message sent to the Hybrid VSOCK capable hypervisor asking
    // for a connection to a real VSOCK server running in the VM on the
    // port specified as part of this message.
    const CONNECT_CMD: &str = "CONNECT";

    // Expected response message returned by the Hybrid VSOCK capable
    // hypervisor informing the client that the CONNECT_CMD was successful.
    const OK_CMD: &str = "OK";

    // Contact the agent by dialing it's port number and
    // waiting for the hybrid vsock hypervisor to route the call for us ;)
    //
    // See: https://github.com/firecracker-microvm/firecracker/blob/main/docs/vsock.md#host-initiated-connections
    let msg = format!("{} {}\n", CONNECT_CMD, hybrid_vsock_port);

    stream.write_all(msg.as_bytes())?;

    // Now, see if we get the expected response
    let stream_reader = stream.try_clone()?;
    let mut reader = BufReader::new(&stream_reader);

    let mut msg = String::new();
    reader.read_line(&mut msg)?;

    if msg.starts_with(OK_CMD) {
        let response = msg
            .strip_prefix(OK_CMD)
            .ok_or(format!("invalid response: {:?}", msg))
            .map_err(|e| anyhow!(e))?
            .trim();

        debug!(sl!(), "Hybrid VSOCK host-side port: {:?}", response);
    } else {
        return Err(anyhow!(
            "failed to setup Hybrid VSOCK connection: response was: {:?}",
            msg
        ));
    }

    // The Unix stream is now connected directly to the VSOCK socket
    // the Kata agent is listening to in the VM.
    Ok(())
}

fn create_ttrpc_client(
    server_address: String,
    hybrid_vsock_port: u64,
    hybrid_vsock: bool,
) -> Result<ttrpc::Client> {
    if server_address.is_empty() {
        return Err(anyhow!("server address cannot be blank"));
    }

    let fields: Vec<&str> = server_address.split("://").collect();

    if fields.len() != 2 {
        return Err(anyhow!("invalid server address URI"));
    }

    let scheme = fields[0].to_lowercase();

    let fd: RawFd = match scheme.as_str() {
        // Formats:
        //
        // - "unix://absolute-path" (domain socket, or hybrid vsock!)
        //   (example: "unix:///tmp/domain.socket")
        //
        // - "unix://@absolute-path" (abstract socket)
        //   (example: "unix://@/tmp/abstract.socket")
        //
        "unix" => {
            let mut abstract_socket = false;

            let mut path = fields[1].to_string();

            if path.starts_with('@') {
                abstract_socket = true;

                // Remove the magic abstract-socket request character ('@').
                path = path[1..].to_string();
            }

            if abstract_socket {
                let socket_fd = match socket(
                    AddressFamily::Unix,
                    SockType::Stream,
                    SockFlag::empty(),
                    None,
                ) {
                    Ok(s) => s,
                    Err(e) => return Err(anyhow!(e).context("Failed to create Unix Domain socket")),
                };

                let unix_addr = match UnixAddr::new_abstract(path.as_bytes()) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(
                            anyhow!(e).context("Failed to create Unix Domain abstract socket")
                        )
                    }
                };

                let sock_addr = SockAddr::Unix(unix_addr);

                connect(socket_fd, &sock_addr).map_err(|e| {
                    anyhow!(e).context("Failed to connect to Unix Domain abstract socket")
                })?;

                socket_fd
            } else {
                let stream = match UnixStream::connect(path) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(
                            anyhow!(e).context("failed to create named UNIX Domain stream socket")
                        )
                    }
                };

                if hybrid_vsock {
                    setup_hybrid_vsock(&stream, hybrid_vsock_port)?
                }

                stream.into_raw_fd()
            }
        }
        // Format: "vsock://cid:port"
        "vsock" => {
            let addr: Vec<&str> = fields[1].split(':').collect();

            if addr.len() != 2 {
                return Err(anyhow!("invalid VSOCK server address URI"));
            }

            let cid: u32 = match addr[0] {
                "-1" | "" => libc::VMADDR_CID_ANY,
                _ => match addr[0].parse::<u32>() {
                    Ok(c) => c,
                    Err(e) => return Err(anyhow!(e).context("VSOCK CID is not numeric")),
                },
            };

            let port: u32 = match addr[1].parse::<u32>() {
                Ok(r) => r,
                Err(e) => return Err(anyhow!(e).context("VSOCK port is not numeric")),
            };

            client_create_vsock_fd(cid, port).map_err(|e| {
                anyhow!(e).context("failed to create VSOCK connection (check agent is running)")
            })?
        }
        _ => {
            return Err(anyhow!("invalid server address URI scheme: {:?}", scheme));
        }
    };

    ttrpc::Client::new(fd).map_err(|err| anyhow!("failed to new a ttrpc client: {:?}", err))
}

fn kata_service_agent(
    server_address: String,
    hybrid_vsock_port: u64,
    hybrid_vsock: bool,
) -> Result<AgentServiceClient> {
    let ttrpc_client = create_ttrpc_client(server_address, hybrid_vsock_port, hybrid_vsock)?;

    Ok(AgentServiceClient::new(ttrpc_client))
}

fn kata_service_health(
    server_address: String,
    hybrid_vsock_port: u64,
    hybrid_vsock: bool,
) -> Result<HealthClient> {
    let ttrpc_client = create_ttrpc_client(server_address, hybrid_vsock_port, hybrid_vsock)?;

    Ok(HealthClient::new(ttrpc_client))
}

fn announce(cfg: &Config) {
    info!(sl!(), "announce"; "config" => format!("{:?}", cfg));
}

pub fn client(cfg: &Config, commands: Vec<&str>) -> Result<()> {
    if commands.len() == 1 && commands[0].eq("list") {
        println!("Built-in commands:\n");

        let mut builtin_cmds = get_builtin_cmd_details();
        builtin_cmds.sort();
        builtin_cmds.iter().for_each(|n| println!("  {}", n));

        println!();

        println!("Agent API commands:\n");

        let mut agent_cmds = get_agent_cmd_details();
        agent_cmds.sort();
        agent_cmds.iter().for_each(|n| println!("  {}", n));

        println!();

        return Ok(());
    }

    announce(cfg);

    // Create separate connections for each of the services provided
    // by the agent.
    let client = kata_service_agent(
        cfg.server_address.clone(),
        cfg.hybrid_vsock_port,
        cfg.hybrid_vsock,
    )?;

    let health = kata_service_health(
        cfg.server_address.clone(),
        cfg.hybrid_vsock_port,
        cfg.hybrid_vsock,
    )?;

    let mut options = Options::new();

    let mut ttrpc_ctx = ttrpc::context::with_timeout(cfg.timeout_nano);

    // Allow the commands to change their behaviour based on the value
    // of this option.

    if !cfg.no_auto_values {
        ttrpc_ctx.add(METADATA_CFG_NS.into(), AUTO_VALUES_CFG_NAME.to_string());

        debug!(sl!(), "Automatic value generation disabled");
    }

    // Special-case loading the OCI config file so it is accessible
    // to all commands.
    let oci_spec_json = utils::get_oci_spec_json(cfg)?;
    options.insert("spec".to_string(), oci_spec_json);

    // Convenience option
    options.insert("bundle-dir".to_string(), cfg.bundle_dir.clone());

    info!(sl!(), "client setup complete";
        "server-address" => cfg.server_address.to_string());

    if cfg.interactive {
        return interactive_client_loop(cfg, &mut options, &client, &health, &ttrpc_ctx);
    }

    let mut repeat_count = 1;

    for cmd in commands {
        if cmd.starts_with(CMD_REPEAT) {
            repeat_count = get_repeat_count(cmd);
            continue;
        }

        let (result, shutdown) = handle_cmd(
            cfg,
            &client,
            &health,
            &ttrpc_ctx,
            repeat_count,
            &mut options,
            cmd,
        );

        result.map_err(|e| anyhow!(e))?;

        if shutdown {
            break;
        }

        // Reset
        repeat_count = 1;
    }

    Ok(())
}

// Handle internal and agent API commands.
fn handle_cmd(
    cfg: &Config,
    client: &AgentServiceClient,
    health: &HealthClient,
    ctx: &Context,
    repeat_count: i64,
    options: &mut Options,
    cmdline: &str,
) -> (Result<()>, bool) {
    let fields: Vec<&str> = cmdline.split_whitespace().collect();

    let cmd = fields[0];

    if cmd.is_empty() {
        // Ignore empty commands
        return (Ok(()), false);
    }

    let first = match cmd.chars().next() {
        Some(c) => c,
        None => return (Err(anyhow!("failed to check command name")), false),
    };

    let args = if fields.len() > 1 {
        fields[1..].join(" ")
    } else {
        String::new()
    };

    let mut count = 0;

    let mut count_msg = String::new();

    if repeat_count < 0 {
        count_msg = "forever".to_string();
    }

    let mut error_count: u64 = 0;
    let mut result: (Result<()>, bool);

    loop {
        if repeat_count > 0 {
            count_msg = format!("{} of {}", count + 1, repeat_count);
        }

        info!(sl!(), "Run command {:} ({})", cmd, count_msg);

        if first.is_lowercase() {
            result = handle_builtin_cmd(cmd, &args);
        } else {
            result = handle_agent_cmd(ctx, client, health, options, cmd, &args);
        }

        if result.0.is_err() {
            if cfg.ignore_errors {
                error_count += 1;
                debug!(sl!(), "ignoring error for command {:}: {:?}", cmd, result.0);
            } else {
                return result;
            }
        }

        info!(
            sl!(),
            "Command {:} ({}) returned {:?}", cmd, count_msg, result
        );

        if repeat_count > 0 {
            count += 1;

            if count == repeat_count {
                break;
            }
        }
    }

    if cfg.ignore_errors {
        debug!(sl!(), "Error count for command {}: {}", cmd, error_count);
        (Ok(()), result.1)
    } else {
        result
    }
}

fn handle_builtin_cmd(cmd: &str, args: &str) -> (Result<()>, bool) {
    let f = match get_builtin_cmd_func(cmd) {
        Ok(fp) => fp,
        Err(e) => return (Err(e), false),
    };

    f(args)
}

// Execute the ttRPC specified by the first field of "line". Return a result
// along with a bool which if set means the client should shutdown.
fn handle_agent_cmd(
    ctx: &Context,
    client: &AgentServiceClient,
    health: &HealthClient,
    options: &mut Options,
    cmd: &str,
    args: &str,
) -> (Result<()>, bool) {
    let f = match get_agent_cmd_func(cmd) {
        Ok(fp) => fp,
        Err(e) => return (Err(e), false),
    };

    let result = f(ctx, client, health, options, args);
    if result.is_err() {
        return (result, false);
    }

    let shutdown = cmd.eq(SHUTDOWN_CMD);

    (Ok(()), shutdown)
}

fn interactive_client_loop(
    cfg: &Config,
    options: &mut Options,
    client: &AgentServiceClient,
    health: &HealthClient,
    ctx: &Context,
) -> Result<()> {
    let result = builtin_cmd_list("");
    if result.0.is_err() {
        return result.0;
    }

    let mut repeat_count: i64 = 1;

    loop {
        let cmdline =
            readline("Enter command").map_err(|e| anyhow!(e).context("failed to read line"))?;

        if cmdline.is_empty() {
            continue;
        }

        if cmdline.starts_with(CMD_REPEAT) {
            repeat_count = get_repeat_count(&cmdline);
            continue;
        }

        let (result, shutdown) =
            handle_cmd(cfg, client, health, ctx, repeat_count, options, &cmdline);

        result.map_err(|e| anyhow!(e))?;

        if shutdown {
            break;
        }

        // Reset
        repeat_count = 1;
    }

    Ok(())
}

fn readline(prompt: &str) -> std::result::Result<String, String> {
    print!("{}: ", prompt);

    io::stdout()
        .flush()
        .map_err(|e| format!("failed to flush: {:?}", e))?;

    let mut line = String::new();

    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("failed to read line: {:?}", e))?;

    // Remove NL
    Ok(line.trim_end().to_string())
}

fn agent_cmd_health_check(
    ctx: &Context,
    _client: &AgentServiceClient,
    health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: CheckRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = health
        .check(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_health_version(
    ctx: &Context,
    _client: &AgentServiceClient,
    health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    // XXX: Yes, the API is actually broken!
    let req: CheckRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = health
        .version(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_create(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: CreateSandboxRequest = utils::make_request(args)?;

    // Generate sandbox_id if it is empty
    if req.sandbox_id.is_empty() {
        req.set_sandbox_id(utils::random_sandbox_id());
    }

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .create_sandbox(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_destroy(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: DestroySandboxRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .destroy_sandbox(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_create(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let input: CreateContainerInput = utils::make_request(args)?;

    if input.image.is_empty() {
        info!(sl!(), "create container: error image is empty");
        return Err(anyhow!("CreateContainer needs image reference"));
    }

    let ctx = clone_context(ctx);

    let req = utils::make_create_container_request(input)?;

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .create_container(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_remove(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: RemoveContainerRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .remove_container(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    // Un-mount the rootfs mount point.
    utils::remove_container_image_mount(req.container_id())?;

    Ok(())
}

fn agent_cmd_container_exec(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: ExecProcessRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;
        let exec_id = utils::get_option("exec_id", options, args)?;

        let ttrpc_spec = utils::get_ttrpc_spec(options, &cid).map_err(|e| anyhow!(e))?;

        let bundle_dir = options
            .get("bundle-dir")
            .ok_or("BUG: bundle-dir missing")
            .map_err(|e| anyhow!(e))?;

        let process = ttrpc_spec
            .Process
            .into_option()
            .ok_or(format!(
                "failed to get process from OCI spec: {}",
                bundle_dir,
            ))
            .map_err(|e| anyhow!(e))?;

        req.set_container_id(cid);
        req.set_exec_id(exec_id);
        req.set_process(process);

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .exec_process(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_stats(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: StatsContainerRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;

        req.set_container_id(cid);
        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .stats_container(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_pause(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: PauseContainerRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;

        req.set_container_id(cid);
        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .pause_container(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_resume(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: ResumeContainerRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;

        req.set_container_id(cid);
        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .resume_container(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_start(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: StartContainerRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .start_container(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

#[allow(clippy::redundant_closure_call)]
fn agent_cmd_sandbox_get_guest_details(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: GuestDetailsRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        req.set_mem_block_size(true);
        req.set_mem_hotplug_probe(true);

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .get_guest_details(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_get_ip_tables(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: GetIPTablesRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .get_ip_tables(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_wait_process(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: WaitProcessRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;
        let exec_id = utils::get_option("exec_id", options, args)?;

        req.set_container_id(cid);
        req.set_exec_id(exec_id);

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .wait_process(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_signal_process(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: SignalProcessRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;
        let exec_id = utils::get_option("exec_id", options, args)?;

        let mut sigstr = utils::get_option("signal", options, args)?;

        // Convert to a numeric
        if sigstr.is_empty() {
            sigstr = DEFAULT_PROC_SIGNAL.to_string();
        }

        let signum = utils::signame_to_signum(&sigstr).map_err(|e| anyhow!(e))?;

        req.set_container_id(cid);
        req.set_exec_id(exec_id);
        req.set_signal(signum as u32);

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .signal_process(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_update_interface(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: UpdateInterfaceRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));
    let reply = client
        .update_interface(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    // FIXME: Implement 'UpdateInterface' fully.
    eprintln!("FIXME: 'UpdateInterface' not fully implemented");

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_update_routes(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: UpdateRoutesRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .update_routes(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    // FIXME: Implement 'UpdateRoutes' fully.
    eprintln!("FIXME: 'UpdateRoutes' not fully implemented");

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_list_interfaces(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: ListInterfacesRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .list_interfaces(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_list_routes(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: ListRoutesRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .list_routes(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_tty_win_resize(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: TtyWinResizeRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;
        let exec_id = utils::get_option("exec_id", options, args)?;

        req.set_container_id(cid);
        req.set_exec_id(exec_id);

        let rows_str = utils::get_option("row", options, args)?;

        if !rows_str.is_empty() {
            let rows = rows_str
                .parse::<u32>()
                .map_err(|e| anyhow!(e).context("invalid row size"))?;
            req.set_row(rows);
        }

        let cols_str = utils::get_option("column", options, args)?;

        if !cols_str.is_empty() {
            let cols = cols_str
                .parse::<u32>()
                .map_err(|e| anyhow!(e).context("invalid column size"))?;

            req.set_column(cols);
        }

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .tty_win_resize(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_close_stdin(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: CloseStdinRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;
        let exec_id = utils::get_option("exec_id", options, args)?;

        req.set_container_id(cid);
        req.set_exec_id(exec_id);

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .close_stdin(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_read_stdout(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: ReadStreamRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;
        let exec_id = utils::get_option("exec_id", options, args)?;

        req.set_container_id(cid);
        req.set_exec_id(exec_id);

        let length_str = utils::get_option("len", options, args)?;

        if !length_str.is_empty() {
            let length = length_str
                .parse::<u32>()
                .map_err(|e| anyhow!(e).context("invalid length"))?;
            req.set_len(length);
        }

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .read_stdout(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_read_stderr(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: ReadStreamRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;
        let exec_id = utils::get_option("exec_id", options, args)?;

        req.set_container_id(cid);
        req.set_exec_id(exec_id);

        let length_str = utils::get_option("len", options, args)?;

        if !length_str.is_empty() {
            let length = length_str
                .parse::<u32>()
                .map_err(|e| anyhow!(e).context("invalid length"))?;
            req.set_len(length);
        }

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .read_stderr(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_write_stdin(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: WriteStreamRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;
        let exec_id = utils::get_option("exec_id", options, args)?;

        let str_data = utils::get_option("data", options, args)?;
        let data = utils::str_to_bytes(&str_data)?;

        req.set_container_id(cid);
        req.set_exec_id(exec_id);
        req.set_data(data.to_vec());

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .write_stdin(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_get_metrics(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: GetMetricsRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .get_metrics(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_get_oom_event(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: GetOOMEventRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .get_oom_event(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_get_volume_stats(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: VolumeStatsRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .get_volume_stats(ctx, &req)
        .map_err(|e| anyhow!(e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_copy_file(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let input: CopyFileInput = utils::make_request(args)?;

    let mut req: CopyFileRequest = utils::make_copy_file_request(&input)?;

    info!(sl!(), "sending request"; "request" => format!("{:?}", req));

    if req.file_size() == 0 {
        let reply = client
            .copy_file(clone_context(ctx), &req)
            .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

        info!(sl!(), "response received"; "response" => format!("{:?}", reply));

        return Ok(());
    }

    let chunk_size = 1024 * 1024;
    let mut remaining_bytes = req.file_size();
    let mut src_file = fs::File::open(&input.src)?;
    let mut offset = 0;
    while remaining_bytes > 0 {
        let mut copy_size = remaining_bytes;
        if copy_size > chunk_size {
            copy_size = chunk_size;
        }

        let mut buf = vec![0; usize::try_from(copy_size)?];
        src_file.read_exact(&mut buf)?;
        req.set_data(buf);
        req.set_offset(offset);

        let reply = client
            .copy_file(clone_context(ctx), &req)
            .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

        info!(sl!(), "response received"; "response" => format!("{:?}", reply));

        remaining_bytes -= copy_size;
        offset += copy_size;
        src_file.seek(SeekFrom::Start(offset as u64))?;
    }

    Ok(())
}

fn agent_cmd_sandbox_reseed_random_dev(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: ReseedRandomDevRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let str_data = utils::get_option("data", options, args)?;
        let data = utils::str_to_bytes(&str_data)?;

        req.set_data(data.to_vec());

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .reseed_random_dev(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_online_cpu_mem(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: OnlineCPUMemRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let wait_str = utils::get_option("wait", options, args)?;

        if !wait_str.is_empty() {
            let wait = wait_str
                .parse::<bool>()
                .map_err(|e| anyhow!(e).context("invalid wait bool"))?;

            req.set_wait(wait);
        }

        let nb_cpus_str = utils::get_option("nb_cpus", options, args)?;

        if !nb_cpus_str.is_empty() {
            let nb_cpus = nb_cpus_str
                .parse::<u32>()
                .map_err(|e| anyhow!(e).context("invalid nb_cpus value"))?;

            req.set_nb_cpus(nb_cpus);
        }

        let cpu_only_str = utils::get_option("cpu_only", options, args)?;

        if !cpu_only_str.is_empty() {
            let cpu_only = cpu_only_str
                .parse::<bool>()
                .map_err(|e| anyhow!(e).context("invalid cpu_only bool"))?;

            req.set_cpu_only(cpu_only);
        }

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .online_cpu_mem(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_set_guest_date_time(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: SetGuestDateTimeRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let secs_str = utils::get_option("sec", options, args)?;

        if !secs_str.is_empty() {
            let secs = secs_str
                .parse::<i64>()
                .map_err(|e| anyhow!(e).context("invalid seconds"))?;

            req.set_Sec(secs);
        }

        let usecs_str = utils::get_option("usec", options, args)?;

        if !usecs_str.is_empty() {
            let usecs = usecs_str
                .parse::<i64>()
                .map_err(|e| anyhow!(e).context("invalid useconds"))?;

            req.set_Usec(usecs);
        }

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .set_guest_date_time(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_set_ip_tables(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: SetIPTablesRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .set_ip_tables(ctx, &req)
        .map_err(|e| anyhow!(e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_add_arp_neighbors(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: AddARPNeighborsRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    // FIXME: Implement fully.
    eprintln!("FIXME: 'AddARPNeighbors' not fully implemented");

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .add_arp_neighbors(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_update_container(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: UpdateContainerRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    run_if_auto_values!(ctx, || -> Result<()> {
        let cid = utils::get_option("cid", options, args)?;

        req.set_container_id(cid);

        Ok(())
    });

    // FIXME: Implement fully
    eprintln!("FIXME: 'UpdateContainer' not fully implemented");

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .update_container(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

#[allow(clippy::redundant_closure_call)]
fn agent_cmd_sandbox_mem_hotplug_by_probe(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req: MemHotplugByProbeRequest = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    // Expected to be a comma separated list of hex addresses
    let addr_list = utils::get_option("memHotplugProbeAddr", options, args)?;

    run_if_auto_values!(ctx, || -> Result<()> {
        if !addr_list.is_empty() {
            let addrs: Vec<u64> = addr_list
                // Convert into a list of string values.
                .split(',')
                // Convert each string element into a u8 array of bytes, ignoring
                // those elements that fail the conversion.
                .filter_map(|s| hex::decode(s.trim_start_matches("0x")).ok())
                // "Stretch" the u8 byte slice into one of length 8
                // (to allow each 8 byte chunk to be converted into a u64).
                .map(|mut v| -> Vec<u8> {
                    v.resize(8, 0x0);
                    v
                })
                // Convert the slice of u8 bytes into a u64
                .map(|b| byteorder::LittleEndian::read_u64(&b))
                .collect();

            req.set_memHotplugProbeAddr(addrs);
        }

        Ok(())
    });

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .mem_hotplug_by_probe(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

#[inline]
fn builtin_cmd_repeat(_args: &str) -> (Result<()>, bool) {
    // XXX: NOP implementation. Due to the way repeat has to work, providing a
    // handler like this is "too late" to be useful. However, a handler
    // is required as "repeat" is a valid command.
    //
    // A cleaner approach would be to make `AgentCmd.fp` an `Option` which for
    // this command would be specified as `None`, but this is the only command
    // which doesn't need an implementation, so this approach is simpler :)

    (Ok(()), false)
}

fn builtin_cmd_sleep(args: &str) -> (Result<()>, bool) {
    let ns = match utils::human_time_to_ns(args) {
        Ok(t) => t,
        Err(e) => return (Err(e), false),
    };

    sleep(Duration::from_nanos(ns as u64));

    (Ok(()), false)
}

fn builtin_cmd_echo(args: &str) -> (Result<()>, bool) {
    println!("{}", args);

    (Ok(()), false)
}

fn builtin_cmd_quit(_args: &str) -> (Result<()>, bool) {
    (Ok(()), true)
}

fn builtin_cmd_list(_args: &str) -> (Result<()>, bool) {
    let cmds = get_all_cmd_details();

    cmds.iter().for_each(|n| println!(" - {}", n));

    println!();

    (Ok(()), false)
}

fn get_repeat_count(cmdline: &str) -> i64 {
    let default_repeat_count: i64 = 1;

    let fields: Vec<&str> = cmdline.split_whitespace().collect();

    if fields.len() < 2 {
        return default_repeat_count;
    }

    if fields[0] != CMD_REPEAT {
        return default_repeat_count;
    }

    let count = fields[1];

    match count.parse::<i64>() {
        Ok(n) => n,
        Err(_) => default_repeat_count,
    }
}

fn agent_cmd_sandbox_add_swap(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    let req = AddSwapRequest::default();

    let ctx = clone_context(ctx);

    debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .add_swap(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    // FIXME: Implement 'AddSwap' fully.
    eprintln!("FIXME: 'AddSwap' not fully implemented");

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_set_policy(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let input: SetPolicyInput = utils::make_request(args)?;

    let req = utils::make_set_policy_request(&input)?;

    let ctx = clone_context(ctx);

    info!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .set_policy(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_mem_agent_memcg_set(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    //let req = MemAgentMemcgConfig::default();
    let req: MemAgentMemcgConfig = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    info!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .mem_agent_memcg_set(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_mem_agent_compact_set(
    ctx: &Context,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    args: &str,
) -> Result<()> {
    let req: MemAgentCompactConfig = utils::make_request(args)?;

    let ctx = clone_context(ctx);

    info!(sl!(), "sending request"; "request" => format!("{:?}", req));

    let reply = client
        .mem_agent_compact_set(ctx, &req)
        .map_err(|e| anyhow!("{:?}", e).context(ERR_API_FAILED))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}
