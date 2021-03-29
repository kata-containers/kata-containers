// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate lazy_static;

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

fn make_examples_text(program_name: &str) -> String {
    let abstract_server_address = "unix://@/foo/bar/abstract.socket";
    let bundle = "$bundle_dir";
    let config_file_uri = "file:///tmp/config.json";
    let container_id = "$container_id";
    let local_server_address = "unix:///tmp/local.socket";
    let sandbox_id = "$sandbox_id";
    let vsock_server_address = "vsock://3:1024";

    format!(
        r#"EXAMPLES:

- Check if the agent is running:

  $ {program} connect --server-address "{vsock_server_address}" --cmd Check

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
    )
}

fn connect(name: &str, global_args: clap::ArgMatches) -> Result<()> {
    let args = global_args
        .subcommand_matches("connect")
        .ok_or("BUG: missing sub-command arguments".to_string())
        .map_err(|e| anyhow!(e))?;

    let interactive = args.is_present("interactive");
    let ignore_errors = args.is_present("ignore-errors");

    let server_address = args
        .value_of("server-address")
        .ok_or("need server adddress".to_string())
        .map_err(|e| anyhow!(e))?;

    let mut commands: Vec<&str> = Vec::new();

    if !interactive {
        commands = args
            .values_of("cmd")
            .ok_or("need commands to send to the server".to_string())
            .map_err(|e| anyhow!(e))?
            .collect();
    }

    // Cannot fail as a default has been specified
    let log_level_name = global_args.value_of("log-level").unwrap();

    let log_level = logging::level_name_to_slog_level(log_level_name).map_err(|e| anyhow!(e))?;

    let writer = io::stdout();
    let (logger, _guard) = logging::create_logger(name, crate_name!(), log_level, writer);

    let timeout_nano: i64 = match args.value_of("timeout") {
        Some(t) => utils::human_time_to_ns(t).map_err(|e| e)?,
        None => 0,
    };

    let bundle_dir = args.value_of("bundle-dir").unwrap_or("");

    let result = rpc::run(
        &logger,
        server_address,
        bundle_dir,
        interactive,
        ignore_errors,
        timeout_nano,
        commands,
    );
    if result.is_err() {
        return result;
    }

    Ok(())
}

fn real_main() -> Result<()> {
    let name = crate_name!();

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
                .default_value(logging::slog_level_to_level_name(DEFAULT_LOG_LEVEL).unwrap())
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
                    Arg::with_name("interactive")
                    .short("i")
                    .long("interactive")
                    .help("Allow interactive client"),
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
        .ok_or("need sub-command".to_string())
        .map_err(|e| anyhow!(e))?;

    match subcmd {
        "generate-cid" => {
            println!("{}", utils::random_container_id());
            return Ok(());
        }
        "generate-sid" => {
            println!("{}", utils::random_sandbox_id());
            return Ok(());
        }
        "examples" => {
            println!("{}", make_examples_text(name));
            return Ok(());
        }
        "connect" => {
            return connect(name, args);
        }
        _ => return Err(anyhow!(format!("invalid sub-command: {:?}", subcmd))),
    }
}

fn main() {
    match real_main() {
        Err(e) => {
            eprintln!("ERROR: {}", e);
            exit(1);
        }
        _ => (),
    };
}
