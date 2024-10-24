// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate lazy_static;

use crate::types::Config;
use anyhow::{anyhow, Result};
use clap::{crate_name, crate_version, App, Arg, SubCommand};
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

    let interactive = args.is_present("interactive");
    let ignore_errors = args.is_present("ignore-errors");

    let server_address = args
        .value_of("server-address")
        .ok_or_else(|| anyhow!("need server adddress"))?
        .to_string();

    let mut commands: Vec<&str> = Vec::new();

    if !interactive {
        commands = args
            .values_of("cmd")
            .ok_or_else(|| anyhow!("need commands to send to the server"))?
            .collect();
    }

    let log_level_name = global_args
        .value_of("log-level")
        .ok_or_else(|| anyhow!("cannot get log level"))?;

    let log_level = logging::level_name_to_slog_level(log_level_name).map_err(|e| anyhow!(e))?;

    let writer = io::stdout();
    let (logger, _guard) = logging::create_logger(name, crate_name!(), log_level, writer);

    let timeout_nano: i64 = match args.value_of("timeout") {
        Some(t) => utils::human_time_to_ns(t)?,
        None => 0,
    };

    let hybrid_vsock_port = args
        .value_of("hybrid-vsock-port")
        .ok_or_else(|| anyhow!("Need Hybrid VSOCK port number"))?
        .parse::<u64>()
        .map_err(|e| anyhow!("VSOCK port number must be an integer: {:?}", e))?;

    let bundle_dir = args.value_of("bundle-dir").unwrap_or("").to_string();

    let hybrid_vsock = args.is_present("hybrid-vsock");
    let no_auto_values = args.is_present("no-auto-values");

    let cfg = Config {
        server_address,
        bundle_dir,
        timeout_nano,
        hybrid_vsock_port,
        interactive,
        hybrid_vsock,
        ignore_errors,
        no_auto_values,
    };

    let result = rpc::run(&logger, &cfg, commands);

    result.map_err(|e| anyhow!(e))
}

fn real_main() -> Result<()> {
    let name = crate_name!();

    let hybrid_vsock_port_help = format!(
        "Kata agent VSOCK port number (only useful with --hybrid-vsock) [default: {}]",
        DEFAULT_KATA_AGENT_API_VSOCK_PORT
    );

    let app = App::new(name)
        .version(crate_version!())
        .about(ABOUT_TEXT)
        .long_about(DESCRIPTION_TEXT)
        .after_help(WARNING_TEXT)
        .arg(
            Arg::with_name("log-level")
                .long("log-level")
                .short("l")
                .help("specific log level")
                .default_value(logging::slog_level_to_level_name(DEFAULT_LOG_LEVEL).map_err(|e| anyhow!(e))?)
                .possible_values(&logging::get_log_levels())
                .takes_value(true)
                .required(false),
        )
        .subcommand(
            SubCommand::with_name("connect")
                .about("Connect to agent")
                .after_help(WARNING_TEXT)
                .arg(
                    Arg::with_name("bundle-dir")
                    .long("bundle-dir")
                    .help("OCI bundle directory")
                    .takes_value(true)
                    .value_name("directory"),
                    )
                .arg(
                    Arg::with_name("cmd")
                    .long("cmd")
                    .short("c")
                    .takes_value(true)
                    .multiple(true)
                    .help("API command (with optional arguments) to send to the server"),
                    )
                .arg(
                    Arg::with_name("ignore-errors")
                    .long("ignore-errors")
                    .help("Don't exit on first error"),
                    )
                .arg(
                    Arg::with_name("hybrid-vsock")
                    .long("hybrid-vsock")
                    .help("Treat a unix:// server address as a Hybrid VSOCK one"),
                    )
                .arg(
                    Arg::with_name("hybrid-vsock-port")
                    .long("hybrid-vsock-port")
                    .help(&hybrid_vsock_port_help)
                    .default_value(DEFAULT_KATA_AGENT_API_VSOCK_PORT)
                    .takes_value(true)
                    .value_name("PORT")
                    )
                .arg(
                    Arg::with_name("interactive")
                    .short("i")
                    .long("interactive")
                    .help("Allow interactive client"),
                    )
                .arg(
                    Arg::with_name("no-auto-values")
                    .short("n")
                    .long("no-auto-values")
                    .help("Disable automatic generation of values for sandbox ID, container ID, etc"),
                    )
                .arg(
                    Arg::with_name("server-address")
                    .long("server-address")
                    .help("server URI (vsock:// or unix://)")
                    .takes_value(true)
                    .value_name("URI"),
                    )
                .arg(
                    Arg::with_name("timeout")
                    .long("timeout")
                    .help("timeout value as nanoseconds or using human-readable suffixes (0 [forever], 99ns, 30us, 2ms, 5s, 7m, etc)")
                    .takes_value(true)
                    .value_name("human-time"),
                    )
                )
                .subcommand(
                    SubCommand::with_name("generate-cid")
                    .about("Create a random container ID")
                )
                .subcommand(
                    SubCommand::with_name("generate-sid")
                    .about("Create a random sandbox ID")
                )
                .subcommand(
                    SubCommand::with_name("examples")
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
