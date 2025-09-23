// Copyright (c) 2020 Intel Corporation
// Copyright (c) 2025 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate lazy_static;
use crate::types::Config;
use anyhow::{anyhow, Result};
use clap::{crate_name, crate_version, Arg, Command};
use std::io;
use std::process::exit;

// Convenience macro to obtain the scope logger
#[macro_export]
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

mod client;
mod image;
mod rpc;
mod types;
mod utils;
mod vm;

const DEFAULT_LOG_LEVEL: slog::Level = slog::Level::Info;

const DESCRIPTION_TEXT: &str = r#"DESCRIPTION:
    Low-level test tool that allows basic interaction with
    the Kata Containers agent using agent API calls."#;

const ABOUT_TEXT: &str = "Kata Containers agent tool";

const WARNING_TEXT: &str = r#"WARNING:
    This tool is for *advanced* users familiar with the low-level agent API calls.
    Further, it is designed to be run on test and development systems **only**:
    since the tool can make arbitrary API calls, it is possible to easily confuse
    irrevocably other parts of the system or even kill a running container or
    sandbox."#;

// The VSOCK port number the Kata agent uses to listen to API requests on.
const DEFAULT_KATA_AGENT_API_VSOCK_PORT: &str = "1024";

fn make_examples_text(program_name: &str) -> String {
    let abstract_server_address = "unix://@/foo/bar/abstract.socket";
    let bundle = "$bundle_dir";
    let config_file_uri = "file:///tmp/config.json";
    let container_id = "$container_id";
    let local_server_address = "unix:///tmp/local.socket";
    let sandbox_id = "$sandbox_id";
    let vsock_server_address = "vsock://3:1024";
    let hybrid_vsock_server_address = "unix:///run/vc/vm/foo/clh.sock";

    format!(
        r#"EXAMPLES:

- Check if the agent is running:

  $ {program} connect --server-address "{vsock_server_address}" --cmd Check

- Connect to the agent using a Hybrid VSOCK hypervisor (here Cloud Hypervisor):

  $ {program} connect --server-address "{hybrid_vsock_server_address}" --hybrid-vsock --cmd Check

- Connect to the agent using local sockets (when running in same environment as the agent):

  # Local socket
  $ {program} connect --server-address "{local_server_address}" --cmd Check

  # Abstract socket
  $ {program} connect --server-address "{abstract_server_address}" --cmd Check

- Boot up a test VM and connect to the agent (socket address determined by the tool):

  $ {program} connect --vm qemu --cmd Check

- Query the agent environment:

  $ {program} connect --server-address "{vsock_server_address}" --cmd GetGuestDetails

- List all available (built-in and Kata Agent API) commands:

  $ {program} connect --server-address "{vsock_server_address}" --cmd list

- Generate a random container ID:

  $ {program} generate-cid

- Generate a random sandbox ID:

  $ {program} generate-sid

- Attempt to create 7 sandboxes, ignoring any errors:

  $ {program} connect --server-address "{vsock_server_address}" --repeat 7 --cmd CreateSandbox

- Query guest details forever:

  $ {program} connect --server-address "{vsock_server_address}" --repeat -1 --cmd GetGuestDetails

- Query guest details, asking for full details by specifying the API request object in JSON format:

  $ {program} connect --server-address "{vsock_server_address}" -c 'GetGuestDetails json://{{"mem_block_size": true, "mem_hotplug_probe": true}}'

- Query guest details, asking for extra detail by partially specifying the API request object in JSON format from a file:

  $ echo '{{"mem_block_size": true}}' > /tmp/api.json
  $ {program} connect --server-address "{vsock_server_address}" -c 'GetGuestDetails file:///tmp/api.json'

- Send a 'SIGUSR1' signal to a container process:

  $ {program} connect --server-address "{vsock_server_address}" --cmd 'SignalProcess signal=usr1 sid={sandbox_id} cid={container_id}'

- Create a sandbox with a single container, and then destroy everything:

  $ {program} connect --server-address "{vsock_server_address}" --cmd CreateSandbox
  $ {program} connect --server-address "{vsock_server_address}" --bundle-dir {bundle:?} --cmd CreateContainer
  $ {program} connect --server-address "{vsock_server_address}" --cmd DestroySandbox

- Create a Container using a custom configuration file:

  $ {program} connect --server-address "{vsock_server_address}" --bundle-dir {bundle:?} --cmd 'CreateContainer spec={config_file_uri}'
	"#,
        abstract_server_address = abstract_server_address,
        bundle = bundle,
        config_file_uri = config_file_uri,
        container_id = container_id,
        local_server_address = local_server_address,
        program = program_name,
        sandbox_id = sandbox_id,
        vsock_server_address = vsock_server_address,
        hybrid_vsock_server_address = hybrid_vsock_server_address,
    )
}

fn connect(name: &str, global_args: clap::ArgMatches) -> Result<()> {
    let args = global_args
        .subcommand_matches("connect")
        .ok_or_else(|| anyhow!("BUG: missing sub-command arguments"))?;

    let interactive = args.contains_id("interactive");
    let ignore_errors = args.contains_id("ignore-errors");

    // boot-up a test vm for testing commands
    let hypervisor_name = args
        .get_one::<String>("vm")
        .map(|s| s.as_str())
        .unwrap_or_default()
        .to_string();

    let server_address = args
        .get_one::<String>("server-address")
        .map(|s| s.as_str())
        .unwrap_or_default()
        .to_string();

    // if vm is requested, we retrieve the server
    // address after the boot-up is completed
    if hypervisor_name.is_empty() && server_address.is_empty() {
        return Err(anyhow!("need server address"));
    }

    let mut commands: Vec<&str> = Vec::new();

    if !interactive {
        commands = args
            .get_many::<String>("cmd")
            .ok_or_else(|| anyhow!("need commands to send to the server"))?
            .map(|s| s.as_str())
            .collect();
    }

    let log_level_name = global_args
        .get_one::<String>("log-level")
        .map(|s| s.as_str())
        .ok_or_else(|| anyhow!("cannot get log level"))?;

    let log_level = logging::level_name_to_slog_level(log_level_name).map_err(|e| anyhow!(e))?;

    let writer = io::stdout();
    let (logger, _guard) = logging::create_logger(name, crate_name!(), log_level, writer);

    let timeout_nano: i64 = match args.get_one::<String>("timeout").map(|s| s.as_str()) {
        Some(t) => utils::human_time_to_ns(t)?,
        None => 0,
    };

    let hybrid_vsock_port = args
        .get_one::<String>("hybrid-vsock-port")
        .map(|s| s.as_str())
        .ok_or_else(|| anyhow!("Need Hybrid VSOCK port number"))?
        .parse::<u64>()
        .map_err(|e| anyhow!("VSOCK port number must be an integer: {:?}", e))?;

    let bundle_dir = args
        .get_one::<String>("bundle-dir")
        .map(|s| s.as_str())
        .unwrap_or("")
        .to_string();

    let hybrid_vsock = args.contains_id("hybrid-vsock");
    let no_auto_values = args.contains_id("no-auto-values");

    let mut cfg = Config {
        server_address,
        bundle_dir,
        timeout_nano,
        hybrid_vsock_port,
        interactive,
        hybrid_vsock,
        ignore_errors,
        no_auto_values,
        hypervisor_name,
        shared_fs_host_path: String::new(),
    };

    let result = rpc::run(&logger, &mut cfg, commands);

    result.map_err(|e| anyhow!(e))
}

fn real_main() -> Result<()> {
    let name = crate_name!();

    let hybrid_vsock_port_help = format!(
        "Kata agent VSOCK port number (only useful with --hybrid-vsock) [default: {}]",
        DEFAULT_KATA_AGENT_API_VSOCK_PORT
    );

    let app = Command::new(name)
        .version(crate_version!())
        .about(ABOUT_TEXT)
        .long_about(DESCRIPTION_TEXT)
        .after_help(WARNING_TEXT)
        .arg(
            Arg::new("log-level")
                .long("log-level")
                .short('l')
                .help("specific log level")
                .default_value(logging::slog_level_to_level_name(DEFAULT_LOG_LEVEL).map_err(|e| anyhow!(e))?)
                .value_parser(logging::get_log_levels())
                .required(false),
        )
        .subcommand(
            Command::new("connect")
                .about("Connect to agent")
                .after_help(WARNING_TEXT)
                .arg(
                    Arg::new("bundle-dir")
                    .long("bundle-dir")
                    .help("OCI bundle directory")
                    .value_name("directory"),
                    )
                .arg(
                    Arg::new("cmd")
                    .long("cmd")
                    .short('c')
                    .num_args(0..)
                    .help("API command (with optional arguments) to send to the server"),
                    )
                .arg(
                    Arg::new("ignore-errors")
                    .long("ignore-errors")
                    .help("Don't exit on first error"),
                    )
                .arg(
                    Arg::new("hybrid-vsock")
                    .long("hybrid-vsock")
                    .help("Treat a unix:// server address as a Hybrid VSOCK one"),
                    )
                .arg(
                    Arg::new("hybrid-vsock-port")
                    .long("hybrid-vsock-port")
                    .help(&hybrid_vsock_port_help)
                    .default_value(DEFAULT_KATA_AGENT_API_VSOCK_PORT)
                    .value_name("PORT")
                    )
                .arg(
                    Arg::new("interactive")
                    .short('i')
                    .long("interactive")
                    .help("Allow interactive client"),
                    )
                .arg(
                    Arg::new("no-auto-values")
                    .short('n')
                    .long("no-auto-values")
                    .help("Disable automatic generation of values for sandbox ID, container ID, etc"),
                    )
                .arg(
                    Arg::new("server-address")
                    .long("server-address")
                    .help("server URI (vsock:// or unix://)")
                    .value_name("URI"),
                    )
                .arg(
                    Arg::new("timeout")
                    .long("timeout")
                    .help("timeout value as nanoseconds or using human-readable suffixes (0 [forever], 99ns, 30us, 2ms, 5s, 7m, etc)")
                    .value_name("human-time"),
                    )
                .arg(
                    Arg::new("vm")
                    .long("vm")
                    .help("boot a pod vm for testing")
                    .value_name("HYPERVISOR"),
                    )
                )
                .subcommand(
                    Command::new("generate-cid")
                    .about("Create a random container ID")
                )
                .subcommand(
                    Command::new("generate-sid")
                    .about("Create a random sandbox ID")
                )
                .subcommand(
                    Command::new("examples")
                    .about("Show usage examples")
                );

    let args = app.get_matches();

    let subcmd = args
        .subcommand_name()
        .ok_or_else(|| anyhow!("need sub-command"))?;

    match subcmd {
        "generate-cid" => {
            println!("{}", utils::random_container_id());
            Ok(())
        }
        "generate-sid" => {
            println!("{}", utils::random_sandbox_id());
            Ok(())
        }
        "examples" => {
            println!("{}", make_examples_text(name));
            Ok(())
        }
        "connect" => connect(name, args),
        _ => Err(anyhow!(format!("invalid sub-command: {:?}", subcmd))),
    }
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("ERROR: {:#?}", e);
        exit(1);
    }
}
