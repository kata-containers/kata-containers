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
    let bundle = "$bundle_dir";
    let cid = 3;
    let container_id = "$container_id";
    let config_file_uri = "file:///tmp/config.json";
    let port = 1024;
    let sandbox_id = "$sandbox_id";

    format!(
        r#"EXAMPLES:

- Check if the agent is running:

  $ {program} connect --vsock-cid {cid} --vsock-port {port} --cmd Check

- Query the agent environment:

  $ {program} connect --vsock-cid {cid} --vsock-port {port} --cmd GuestDetails

- List all available (built-in and Kata Agent API) commands:

  $ {program} connect --vsock-cid {cid} --vsock-port {port} --cmd list

- Generate a random container ID:

  $ {program} generate-cid

- Generate a random sandbox ID:

  $ {program} generate-sid

- Attempt to create 7 sandboxes, ignoring any errors:

  $ {program} connect --vsock-cid {cid} --vsock-port {port} --repeat 7 --cmd CreateSandbox

- Query guest details forever:

  $ {program} connect --vsock-cid {cid} --vsock-port {port} --repeat -1 --cmd GuestDetails

- Send a 'SIGUSR1' signal to a container process:

  $ {program} connect --vsock-cid {cid} --vsock-port {port} --cmd 'SignalProcess signal=usr1 sid={sandbox_id} cid={container_id}'

- Create a sandbox with a single container, and then destroy everything:

  $ {program} connect --vsock-cid {cid} --vsock-port {port} --cmd CreateSandbox
  $ {program} connect --vsock-cid {cid} --vsock-port {port} --bundle-dir {bundle:?} --cmd CreateContainer
  $ {program} connect --vsock-cid {cid} --vsock-port {port} --cmd DestroySandbox

- Create a Container using a custom configuration file:

  $ {program} connect --vsock-cid {cid} --vsock-port {port} --bundle-dir {bundle:?} --cmd 'CreateContainer spec={config_file_uri}'
	"#,
        bundle = bundle,
        cid = cid,
        config_file_uri = config_file_uri,
        container_id = container_id,
        port = port,
        program = program_name,
        sandbox_id = sandbox_id,
    )
}

fn connect(name: &str, global_args: clap::ArgMatches) -> Result<()> {
    let args = global_args
        .subcommand_matches("connect")
        .ok_or("BUG: missing sub-command arguments".to_string())
        .map_err(|e| anyhow!(e))?;

    let interactive = args.is_present("interactive");
    let ignore_errors = args.is_present("ignore-errors");

    let cid_str = args
        .value_of("vsock-cid")
        .ok_or("need VSOCK cid".to_string())
        .map_err(|e| anyhow!(e))?;

    let port_str = args
        .value_of("vsock-port")
        .ok_or("need VSOCK port number".to_string())
        .map_err(|e| anyhow!(e))?;

    let cid: u32 = cid_str
        .parse::<u32>()
        .map_err(|e| anyhow!(format!("invalid VSOCK CID number: {}", e.to_string())))?;

    let port: u32 = port_str
        .parse::<u32>()
        .map_err(|e| anyhow!(format!("invalid VSOCK port number: {}", e)))?;

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
    let logger = logging::create_logger(name, crate_name!(), log_level, writer);

    let timeout_nano: i64 = match args.value_of("timeout") {
        Some(t) => utils::human_time_to_ns(t).map_err(|e| e)?,
        None => 0,
    };

    let bundle_dir = args.value_of("bundle-dir").unwrap_or("");

    let result = rpc::run(
        &logger,
        cid,
        port,
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
                    Arg::with_name("vsock-cid")
                    .long("vsock-cid")
                    .help("VSOCK Context ID")
                    .takes_value(true)
                    .value_name("CID"),
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
                    Arg::with_name("vsock-port")
                    .long("vsock-port")
                    .help("VSOCK Port number")
                    .takes_value(true)
                    .value_name("port-number"),
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
