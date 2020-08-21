// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Description: Client side of ttRPC comms

use crate::types::{Config, Options};
use crate::utils;
use anyhow::{anyhow, Result};
use nix::sys::socket::{connect, socket, AddressFamily, SockAddr, SockFlag, SockType};
use protocols::agent::*;
use protocols::agent_ttrpc::*;
use protocols::health::*;
use protocols::health_ttrpc::*;
use slog::{debug, info};
use std::io;
use std::io::Write; // XXX: for flush()
use std::os::unix::io::RawFd;
use std::thread::sleep;
use std::time::Duration;
use ttrpc;

// Agent command handler type
//
// Notes:
//
// - 'cmdline' is the command line (command name and optional space separate
//   arguments).
// - 'options' can be read and written to, allowing commands to pass state to
//   each other via well-known option names.
type AgentCmdFp = fn(
    cfg: &Config,
    client: &AgentServiceClient,
    health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()>;

// Builtin command handler type
type BuiltinCmdFp = fn(cfg: &Config, options: &mut Options, args: &str) -> (Result<()>, bool);

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
const SHUTDOWN_CMD: &'static str = "DestroySandbox";

// Command that requests this program ends
const CMD_QUIT: &'static str = "quit";
const CMD_REPEAT: &'static str = "repeat";

const DEFAULT_PROC_SIGNAL: &'static str = "SIGKILL";

// Format is either "json" or "table".
const DEFAULT_PS_FORMAT: &str = "json";

const ERR_API_FAILED: &str = "API failed";

static AGENT_CMDS: &'static [AgentCmd] = &[
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
        name: "GuestDetails",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_guest_details,
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
        name: "ListProcesses",
        st: ServiceType::Agent,
        fp: agent_cmd_container_list_processes,
    },
    AgentCmd {
        name: "PauseContainer",
        st: ServiceType::Agent,
        fp: agent_cmd_container_pause,
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
        name: "StartTracing",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_tracing_start,
    },
    AgentCmd {
        name: "StatsContainer",
        st: ServiceType::Agent,
        fp: agent_cmd_container_stats,
    },
    AgentCmd {
        name: "StopTracing",
        st: ServiceType::Agent,
        fp: agent_cmd_sandbox_tracing_stop,
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
];

static BUILTIN_CMDS: &'static [BuiltinCmd] = &[
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
        if cmd.name == name {
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
        if cmd.name == name {
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

fn create_ttrpc_client(cid: libc::c_uint, port: u32) -> Result<ttrpc::Client> {
    let fd = client_create_vsock_fd(cid, port).map_err(|e| {
        anyhow!(format!(
            "failed to create VSOCK connection (check agent is running): {:?}",
            e
        ))
    })?;

    Ok(ttrpc::client::Client::new(fd))
}

fn kata_service_agent(cid: libc::c_uint, port: u32) -> Result<AgentServiceClient> {
    let ttrpc_client = create_ttrpc_client(cid, port)?;

    Ok(AgentServiceClient::new(ttrpc_client))
}

fn kata_service_health(cid: libc::c_uint, port: u32) -> Result<HealthClient> {
    let ttrpc_client = create_ttrpc_client(cid, port)?;

    Ok(HealthClient::new(ttrpc_client))
}

fn announce(cfg: &Config) {
    info!(sl!(), "announce"; "config" => format!("{:?}", cfg));
}

pub fn client(cfg: &Config, commands: Vec<&str>) -> Result<()> {
    if commands.len() == 1 && commands[0] == "list" {
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

    let cid = cfg.cid;
    let port = cfg.port;

    let addr = format!("vsock://{}:{}", cid, port);

    // Create separate connections for each of the services provided
    // by the agent.
    let client = kata_service_agent(cid, port as u32)?;
    let health = kata_service_health(cid, port as u32)?;

    let mut options = Options::new();

    // Special-case loading the OCI config file so it is accessible
    // to all commands.
    let oci_spec_json = utils::get_oci_spec_json(cfg)?;
    options.insert("spec".to_string(), oci_spec_json);

    // Convenience option
    options.insert("bundle-dir".to_string(), cfg.bundle_dir.clone());

    info!(sl!(), "client setup complete";
        "server-address" => addr);

    if cfg.interactive {
        return interactive_client_loop(&cfg, &mut options, &client, &health);
    }

    let mut repeat_count = 1;

    for cmd in commands {
        if cmd.starts_with(CMD_REPEAT) {
            repeat_count = get_repeat_count(cmd);
            continue;
        }

        let (result, shutdown) =
            handle_cmd(&cfg, &client, &health, repeat_count, &mut options, &cmd);
        if result.is_err() {
            return result;
        }

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
    repeat_count: i64,
    options: &mut Options,
    cmdline: &str,
) -> (Result<()>, bool) {
    let fields: Vec<&str> = cmdline.split_whitespace().collect();

    let cmd = fields[0];

    if cmd == "" {
        // Ignore empty commands
        return (Ok(()), false);
    }

    let first = match cmd.chars().nth(0) {
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
            result = handle_builtin_cmd(cfg, options, cmd, &args);
        } else {
            result = handle_agent_cmd(cfg, client, health, options, cmd, &args);
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

fn handle_builtin_cmd(
    cfg: &Config,
    options: &mut Options,
    cmd: &str,
    args: &str,
) -> (Result<()>, bool) {
    let f = match get_builtin_cmd_func(&cmd) {
        Ok(fp) => fp,
        Err(e) => return (Err(e), false),
    };

    f(cfg, options, &args)
}

// Execute the ttRPC specified by the first field of "line". Return a result
// along with a bool which if set means the client should shutdown.
fn handle_agent_cmd(
    cfg: &Config,
    client: &AgentServiceClient,
    health: &HealthClient,
    options: &mut Options,
    cmd: &str,
    args: &str,
) -> (Result<()>, bool) {
    let f = match get_agent_cmd_func(&cmd) {
        Ok(fp) => fp,
        Err(e) => return (Err(e), false),
    };

    let result = f(cfg, client, health, options, &args);
    if result.is_err() {
        return (result, false);
    }

    let shutdown = cmd == SHUTDOWN_CMD;

    (Ok(()), shutdown)
}

fn interactive_client_loop(
    cfg: &Config,
    options: &mut Options,
    client: &AgentServiceClient,
    health: &HealthClient,
) -> Result<()> {
    let result = builtin_cmd_list(cfg, options, "");
    if result.0.is_err() {
        return result.0;
    }

    let mut repeat_count: i64 = 1;

    loop {
        let cmdline = readline("Enter command")
            .map_err(|e| anyhow!(format!("failed to read line: {}", e)))?;

        if cmdline == "" {
            continue;
        }

        if cmdline.starts_with(CMD_REPEAT) {
            repeat_count = get_repeat_count(&cmdline);
            continue;
        }

        let (result, shutdown) = handle_cmd(cfg, client, health, repeat_count, options, &cmdline);
        if result.is_err() {
            return result;
        }

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
    cfg: &Config,
    _client: &AgentServiceClient,
    health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    let mut req = CheckRequest::default();

    // value unused
    req.set_service("".to_string());

    let reply = health
        .check(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_health_version(
    cfg: &Config,
    _client: &AgentServiceClient,
    health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    // XXX: Yes, the API is actually broken!
    let mut req = CheckRequest::default();

    // value unused
    req.set_service("".to_string());

    let reply = health
        .version(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_create(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = CreateSandboxRequest::default();

    let sid = utils::get_option("sid", options, args);
    req.set_sandbox_id(sid);

    let reply = client
        .create_sandbox(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_destroy(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    let req = DestroySandboxRequest::default();

    let reply = client
        .destroy_sandbox(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_create(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = CreateContainerRequest::default();

    let cid = utils::get_option("cid", options, args);
    let exec_id = utils::get_option("exec_id", options, args);

    // FIXME: container create: add back "spec=file:///" support

    let grpc_spec = utils::get_grpc_spec(options, &cid).map_err(|e| anyhow!(e))?;

    req.set_container_id(cid);
    req.set_exec_id(exec_id);
    req.set_OCI(grpc_spec);

    let reply = client
        .create_container(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_remove(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = RemoveContainerRequest::default();

    let cid = utils::get_option("cid", options, args);

    req.set_container_id(cid);

    let reply = client
        .remove_container(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_exec(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = ExecProcessRequest::default();

    let cid = utils::get_option("cid", options, args);
    let exec_id = utils::get_option("exec_id", options, args);

    let grpc_spec = utils::get_grpc_spec(options, &cid).map_err(|e| anyhow!(e))?;

    let process = grpc_spec
        .Process
        .into_option()
        .ok_or(format!(
            "failed to get process from OCI spec: {}",
            cfg.bundle_dir
        ))
        .map_err(|e| anyhow!(e))?;

    req.set_container_id(cid);
    req.set_exec_id(exec_id);
    req.set_process(process);

    let reply = client
        .exec_process(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_stats(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = StatsContainerRequest::default();

    let cid = utils::get_option("cid", options, args);

    req.set_container_id(cid);

    let reply = client
        .stats_container(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_pause(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = PauseContainerRequest::default();

    let cid = utils::get_option("cid", options, args);

    req.set_container_id(cid);

    let reply = client
        .pause_container(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_resume(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = ResumeContainerRequest::default();

    let cid = utils::get_option("cid", options, args);

    req.set_container_id(cid);

    let reply = client
        .resume_container(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_start(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = StartContainerRequest::default();

    let cid = utils::get_option("cid", options, args);

    req.set_container_id(cid);

    let reply = client
        .start_container(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_guest_details(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    let mut req = GuestDetailsRequest::default();

    req.set_mem_block_size(true);

    let reply = client
        .get_guest_details(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_list_processes(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = ListProcessesRequest::default();

    let cid = utils::get_option("cid", options, args);

    let mut list_format = utils::get_option("format", options, args);

    if list_format == "" {
        list_format = DEFAULT_PS_FORMAT.to_string();
    }

    req.set_container_id(cid);
    req.set_format(list_format);

    let reply = client
        .list_processes(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_wait_process(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = WaitProcessRequest::default();

    let cid = utils::get_option("cid", options, args);
    let exec_id = utils::get_option("exec_id", options, args);

    req.set_container_id(cid);
    req.set_exec_id(exec_id);

    let reply = client
        .wait_process(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_container_signal_process(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    options: &mut Options,
    args: &str,
) -> Result<()> {
    let mut req = SignalProcessRequest::default();

    let cid = utils::get_option("cid", options, args);
    let exec_id = utils::get_option("exec_id", options, args);

    let mut sigstr = utils::get_option("signal", options, args);

    // Convert to a numeric
    if sigstr == "" {
        sigstr = DEFAULT_PROC_SIGNAL.to_string();
    }

    let signum = utils::signame_to_signum(&sigstr).map_err(|e| anyhow!(e))?;

    req.set_container_id(cid);
    req.set_exec_id(exec_id);
    req.set_signal(signum as u32);

    let reply = client
        .signal_process(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_tracing_start(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    let req = StartTracingRequest::default();

    let reply = client
        .start_tracing(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_tracing_stop(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    let req = StopTracingRequest::default();

    let reply = client
        .stop_tracing(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_update_interface(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    let req = UpdateInterfaceRequest::default();

    let reply = client
        .update_interface(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    // FIXME: Implement 'UpdateInterface' fully.
    eprintln!("FIXME: 'UpdateInterface' not fully implemented");

    // let if = ...;
    // req.set_interface(if);

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_update_routes(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    let req = UpdateRoutesRequest::default();

    let reply = client
        .update_routes(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    // FIXME: Implement 'UpdateRoutes' fully.
    eprintln!("FIXME: 'UpdateRoutes' not fully implemented");

    // let routes = ...;
    // req.set_routes(routes);

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_list_interfaces(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    let req = ListInterfacesRequest::default();

    let reply = client
        .list_interfaces(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

fn agent_cmd_sandbox_list_routes(
    cfg: &Config,
    client: &AgentServiceClient,
    _health: &HealthClient,
    _options: &mut Options,
    _args: &str,
) -> Result<()> {
    let req = ListRoutesRequest::default();

    let reply = client
        .list_routes(&req, cfg.timeout_nano)
        .map_err(|e| anyhow!(format!("{}: {:?}", ERR_API_FAILED, e)))?;

    info!(sl!(), "response received";
        "response" => format!("{:?}", reply));

    Ok(())
}

#[inline]
fn builtin_cmd_repeat(_cfg: &Config, _options: &mut Options, _args: &str) -> (Result<()>, bool) {
    // XXX: NOP implementation. Due to the way repeat has to work, providing
    // handler like this is "too late" to be useful. However, a handler
    // is required as "repeat" is a valid command.
    //
    // A cleaner approach would be to make `AgentCmd.fp` an `Option` which for
    // this command would be specified as `None`, but this is the only command
    // which doesn't need an implementation, so this approach is simpler :)

    (Ok(()), false)
}

fn builtin_cmd_sleep(_cfg: &Config, _options: &mut Options, args: &str) -> (Result<()>, bool) {
    let ns = match utils::human_time_to_ns(args) {
        Ok(t) => t,
        Err(e) => return (Err(e), false),
    };

    sleep(Duration::from_nanos(ns as u64));

    (Ok(()), false)
}

fn builtin_cmd_echo(_cfg: &Config, _options: &mut Options, args: &str) -> (Result<()>, bool) {
    println!("{}", args);

    (Ok(()), false)
}

fn builtin_cmd_quit(_cfg: &Config, _options: &mut Options, _args: &str) -> (Result<()>, bool) {
    (Ok(()), true)
}

fn builtin_cmd_list(_cfg: &Config, _options: &mut Options, _args: &str) -> (Result<()>, bool) {
    let cmds = get_all_cmd_details();

    cmds.iter().for_each(|n| println!(" - {}", n));

    println!("");

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
        Ok(n) => return n,
        Err(_) => return default_repeat_count,
    }
}
