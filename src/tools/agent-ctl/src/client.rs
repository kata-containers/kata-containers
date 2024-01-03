// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Description: Client side of ttRPC comms

use crate::types::{Config, Options};
use crate::utils;
use anyhow::{anyhow, Result};
use nix::sys::socket::{connect, socket, AddressFamily, SockAddr, SockFlag, SockType, UnixAddr};
use protocols::agent_ttrpc::*;
use protocols::health_ttrpc::*;
use slog::{debug, info};
use std::io;
use std::io::Write; // XXX: for flush()
use std::io::{BufRead, BufReader};
use std::os::unix::io::{IntoRawFd, RawFd};
use std::os::unix::net::UnixStream;
use ttrpc::context::Context;

#[macro_use]
mod command;

// Command that requests this program ends
const CMD_REPEAT: &str = "repeat";

const DEFAULT_PROC_SIGNAL: &str = "SIGKILL";

const ERR_API_FAILED: &str = "API failed";

// Value used as a "namespace" in the ttRPC Context's metadata.
const METADATA_CFG_NS: &str = "agent-ctl-cfg";

// Special value which if found means do not generate any values
// automatically.
const NO_AUTO_VALUES_CFG_NAME: &str = "no-auto-values";

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
        // print_cmd_list!();
        command::cmd_list();

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
        ttrpc_ctx.add(METADATA_CFG_NS.into(), NO_AUTO_VALUES_CFG_NAME.to_string());

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
            result = match command::parse_builtin_cmd(cmd) {
                Ok(builtin_cmd) => builtin_cmd.exec(&args),
                Err(e) => (Err(e), false),
            }
        } else {
            result = match command::parse_agent_cmd(cmd) {
                Ok(agent_cmd) => agent_cmd.exec(ctx, client, health, options, &args),
                Err(e) => (Err(e), false),
            }
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

fn interactive_client_loop(
    cfg: &Config,
    options: &mut Options,
    client: &AgentServiceClient,
    health: &HealthClient,
    ctx: &Context,
) -> Result<()> {
    command::cmd_list();

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
