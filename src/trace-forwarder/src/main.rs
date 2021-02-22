// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#![warn(unused_extern_crates)]
use anyhow::{anyhow, Result};
use clap::{crate_name, crate_version, App, Arg};
use slog::{error, info, Logger};
use std::env;
use std::io;
use std::process::exit;

// Traces will be created using this program name
const DEFAULT_TRACE_NAME: &str = "kata-agent";

const VSOCK_CID_ANY: &str = "any";
const ABOUT_TEXT: &str = "Kata Containers Trace Forwarder";

const DESCRIPTION_TEXT: &str = r#"
DESCRIPTION:
    Kata Containers component that runs on the host and forwards
    trace data from the container to a trace collector on the host."#;

const DEFAULT_LOG_LEVEL: slog::Level = slog::Level::Info;

// VSOCK port this program listens to for trace data, sent by the agent.
//
// Must match the number used by the agent
const DEFAULT_KATA_VSOCK_TRACING_PORT: &str = "10240";

const DEFAULT_JAEGER_HOST: &str = "127.0.0.1";
const DEFAULT_JAEGER_PORT: &str = "6831";

mod handler;
mod server;
mod tracer;

fn announce(logger: &Logger, version: &str) {
    let commit = env::var("VERSION_COMMIT").map_or(String::new(), |s| s);

    info!(logger, "announce";
    "commit-version" => commit.as_str(),
    "version" =>  version);
}

fn make_examples_text(program_name: &str) -> String {
    format!(
        r#"EXAMPLES:

- Normally run on host specifying VSOCK port number
  for Kata Containers agent to connect to:

    $ {program} --trace-name {trace_name:?} -p 12345

  "#,
        program = program_name,
        trace_name = DEFAULT_TRACE_NAME,
    )
}

fn real_main() -> Result<()> {
    let version = crate_version!();
    let name = crate_name!();

    let args = App::new(name)
        .version(version)
        .version_short("v")
        .about(ABOUT_TEXT)
        .long_about(DESCRIPTION_TEXT)
        .after_help(&*make_examples_text(name))
        .arg(
            Arg::with_name("trace-name")
                .long("trace-name")
                .help("Specify name for traces")
                .required(false)
                .takes_value(true)
                .default_value(DEFAULT_TRACE_NAME),
        )
        .arg(
            Arg::with_name("jaeger-host")
                .long("jaeger-host")
                .help("Jaeger host address")
                .takes_value(true)
                .default_value(DEFAULT_JAEGER_HOST),
        )
        .arg(
            Arg::with_name("jaeger-port")
                .long("jaeger-port")
                .help("Jaeger port number")
                .takes_value(true)
                .default_value(DEFAULT_JAEGER_PORT),
        )
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
        .arg(
            Arg::with_name("vsock-cid")
                .long("vsock-cid")
                .help(&format!("VSOCK CID number (or {:?})", VSOCK_CID_ANY))
                .takes_value(true)
                .required(false)
                .default_value(VSOCK_CID_ANY),
        )
        .arg(
            Arg::with_name("vsock-port")
                .long("vsock-port")
                .help("VSOCK port number")
                .takes_value(true)
                .default_value(DEFAULT_KATA_VSOCK_TRACING_PORT),
        )
        .get_matches();

    let vsock_port: u32 = args
        .value_of("vsock-port")
        .ok_or(anyhow!("Need VSOCK port number"))
        .map_or_else(
            |e| Err(anyhow!(e)),
            |p| {
                p.parse::<u32>()
                    .map_err(|e| anyhow!(format!("VSOCK port number must be an integer: {:?}", e)))
            },
        )?;

    if vsock_port == 0 {
        return Err(anyhow!("VSOCK port number cannot be zero"));
    }

    let vsock_cid: u32 = args
        .value_of("vsock-cid")
        .ok_or(libc::VMADDR_CID_ANY as u32)
        .map_or_else(
            |e| Err(anyhow!(e)),
            |c| {
                if c == VSOCK_CID_ANY {
                    // Explicit request for "any CID"
                    Ok(libc::VMADDR_CID_ANY as u32)
                } else {
                    c.parse::<u32>()
                        .map_err(|e| anyhow!(format!("CID number must be an integer: {:?}", e)))
                }
            },
        )
        .map_err(|e| anyhow!(e))?;

    if vsock_cid == 0 {
        return Err(anyhow!("VSOCK CID cannot be zero"));
    }

    let jaeger_port: u32 = args
        .value_of("jaeger-port")
        .ok_or("Need Jaeger port number")
        .map(|p| p.parse::<u32>().unwrap())
        .map_err(|e| anyhow!("Jaeger port number must be an integer: {:?}", e))?;

    if jaeger_port == 0 {
        return Err(anyhow!("Jaeger port number cannot be zero"));
    }

    let jaeger_host = args
        .value_of("jaeger-host")
        .ok_or("Need Jaeger host")
        .map_err(|e| anyhow!(e))?;

    if jaeger_host == "" {
        return Err(anyhow!("Jaeger host cannot be blank"));
    }

    // Cannot fail as a default has been specified
    let log_level_name = args.value_of("log-level").unwrap();

    let log_level = logging::level_name_to_slog_level(log_level_name).map_err(|e| anyhow!(e))?;

    // Setup logger
    let writer = io::stdout();
    let (logger, _logger_guard) = logging::create_logger(name, name, log_level, writer);

    announce(&logger, version);

    let trace_name: &str = args
        .value_of("trace-name")
        .ok_or(anyhow!("BUG: trace name not set"))
        .map_or_else(
            |e| Err(anyhow!(e)),
            |n| {
                if n == "" {
                    Err(anyhow!("Need non-blank trace name"))
                } else {
                    Ok(n)
                }
            },
        )?;

    let mut server = server::VsockTraceServer::new(
        &logger,
        vsock_port,
        vsock_cid,
        jaeger_host,
        jaeger_port,
        trace_name,
    );

    let result = server.start();

    if result.is_err() {
        error!(logger, "failed"; "error" => format!("{:?}", result.err()));
    } else {
        info!(logger, "success");
    }

    Ok(())
}

fn main() {
    match real_main() {
        Err(e) => {
            eprintln!("ERROR: {}", e);
            exit(1);
        }
        _ => (),
    };

    exit(0);
}
