// Copyright (c) 2020-2021 Intel Corporation
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

const ABOUT_TEXT: &str = "Kata Containers Trace Forwarder";

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
mod utils;

use crate::utils::{
    make_hybrid_socket_path, str_to_vsock_cid, str_to_vsock_port, VSOCK_CID_ANY_STR,
};
use server::VsockType;

fn announce(logger: &Logger, version: &str, dump_only: bool) {
    let commit = env::var("VERSION_COMMIT").map_or(String::new(), |s| s);

    info!(logger, "announce";
    "commit-version" => commit.as_str(),
    "version" =>  version,
    "dump-only" => dump_only);
}

fn make_description_text() -> String {
    format!(
        r#" DESCRIPTION:
    Kata Containers component that runs on the host and forwards
    trace data from the container to a trace collector on the host.

    This tool requires agent tracing to be enabled in the Kata
    configuration file. It uses VSOCK to listen for trace data originating
    from the Kata agent running inside the Kata Container.

    The variety of VSOCK used depends on the configuration hypervisor:

    |------------------------|--------------------|----------------|
    | Hypervisor             | Type of VSOCK      | Run as user    |
    |------------------------|--------------------|----------------|
    | Cloud Hypervisor (CLH) | Firecracker Hybrid | privileged     |
    |------------------------|--------------------|----------------|
    | QEMU                   | Standard           | non-privileged |
    |------------------------|--------------------|----------------|
    | Firecracker (FC)       | Firecracker Hybrid | privileged     |
    |------------------------|--------------------|----------------|

        Key:

        - Firecracker Hybrid VSOCK: See the Firecracker
          VSOCK documentation.
        - Standard VSOCK: see vsock(7).

    The way this tool is run depends on the configured hypervisor.
    See EXAMPLES for further information.

    Note that Hybrid VSOCK requries root privileges initially. Due to the way the
    hybrid protocol works, the specified "master socket" itself is not used: to
    communicate with the agent, this tool must generate a socket path using
    the specified socket path as a prefix. Since the master socket will be
    created in a root-owned directory when the Kata Containers VM (sandbox) is
    created, this tool must be run as root to allow it to create the second
    agent-specific socket. However, once the forwarder has started running, it
    drops privileges and will continue running as user {user:?}.
    "#,
        user = server::NON_PRIV_USER
    )
}

fn make_examples_text(program_name: &str) -> String {
    format!(
        r#"EXAMPLES:

- Example assuming QEMU is the Kata configured hypervisor:

    $ {program} --trace-name {trace_name:?}

- Example assuming cloud-hypervisor is the Kata configured hypervisor
  and the sandbox _about_ to be created will be called {sandbox_id:?}:

    $ sandbox_id={sandbox_id:?}
    $ sudo {program} --trace-name {trace_name:?} --socket-path /run/vc/vm/{sandbox_id}/clh.sock

- Example assuming firecracker is the Kata configured hypervisor
  and the sandbox _about_ to be created will be called {sandbox_id:?}:

    $ sandbox_id={sandbox_id:?}
    $ sudo {program} --trace-name {trace_name:?} --socket-path /run/vc/firecracker/{sandbox_id}/root/kata.hvsock
  "#,
        program = program_name,
        trace_name = DEFAULT_TRACE_NAME,
        sandbox_id = "foo"
    )
}

fn handle_hybrid_vsock(socket_path: &str, port: Option<&str>) -> Result<VsockType> {
    let socket_path = make_hybrid_socket_path(socket_path, port, DEFAULT_KATA_VSOCK_TRACING_PORT)?;

    let vsock = VsockType::Hybrid { socket_path };

    Ok(vsock)
}

fn handle_standard_vsock(cid: Option<&str>, port: Option<&str>) -> Result<VsockType> {
    let cid = str_to_vsock_cid(cid)?;
    let port = str_to_vsock_port(port, DEFAULT_KATA_VSOCK_TRACING_PORT)?;

    let vsock = VsockType::Standard { port, cid };

    Ok(vsock)
}

fn real_main() -> Result<()> {
    let version = crate_version!();
    let name = crate_name!();

    let args = App::new(name)
        .version(version)
        .version_short("v")
        .about(ABOUT_TEXT)
        .long_about(&*make_description_text())
        .after_help(&*make_examples_text(name))
        .arg(
            Arg::with_name("dump-only")
                .long("dump-only")
                .help("Disable forwarding of spans and write to stdout (for testing)"),
        )
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
            Arg::with_name("socket-path")
                .long("socket-path")
                .help("Full path to hypervisor socket (needs root! cloud-hypervisor and firecracker hypervisors only)")
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("vsock-cid")
                .long("vsock-cid")
                .help(&format!(
                    "VSOCK CID number (or {:?}) (QEMU hypervisor only)",
                    VSOCK_CID_ANY_STR
                ))
                .takes_value(true)
                .required(false)
                .default_value(VSOCK_CID_ANY_STR),
        )
        .arg(
            Arg::with_name("vsock-port")
                .long("vsock-port")
                .help("VSOCK port number (QEMU hypervisor only)")
                .takes_value(true)
                .default_value(DEFAULT_KATA_VSOCK_TRACING_PORT),
        )
        .get_matches();

    // Cannot fail as a default has been specified
    let log_level_name = args.value_of("log-level").unwrap();

    let log_level = logging::level_name_to_slog_level(log_level_name).map_err(|e| anyhow!(e))?;

    // Setup logger
    let writer = io::stdout();
    let (logger, _logger_guard) = logging::create_logger(name, name, log_level, writer);

    let dump_only = args.is_present("dump-only");

    announce(&logger, version, dump_only);

    let trace_name: &str = args
        .value_of("trace-name")
        .ok_or(anyhow!("BUG: trace name not set"))
        .map_or_else(
            |e| Err(anyhow!(e)),
            |n| {
                if n.is_empty() {
                    Err(anyhow!("Need non-blank trace name"))
                } else {
                    Ok(n)
                }
            },
        )?;

    // Handle the Hybrid VSOCK option first (since it cannot be defaulted).
    let vsock = if let Some(socket_path) = args.value_of("socket-path") {
        handle_hybrid_vsock(socket_path, args.value_of("vsock-port"))
    } else {
        // The default is standard VSOCK
        handle_standard_vsock(args.value_of("vsock-cid"), args.value_of("vsock-port"))
    }?;

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

    if jaeger_host.is_empty() {
        return Err(anyhow!("Jaeger host cannot be blank"));
    }

    let server = server::VsockTraceServer::new(
        &logger,
        vsock,
        jaeger_host,
        jaeger_port,
        trace_name,
        dump_only,
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
    if let Err(e) = real_main() {
        eprintln!("ERROR: {:#?}", e);
        exit(1);
    }
    exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_result;
    use utils::{
        ERR_HVSOCK_SOC_PATH_EMPTY, ERR_VSOCK_CID_EMPTY, ERR_VSOCK_CID_NOT_NUMERIC,
        ERR_VSOCK_PORT_EMPTY, ERR_VSOCK_PORT_NOT_NUMERIC, ERR_VSOCK_PORT_ZERO, VSOCK_CID_ANY,
    };

    #[test]
    fn test_handle_hybrid_vsock() {
        #[derive(Debug)]
        struct TestData<'a> {
            socket_path: &'a str,
            port: Option<&'a str>,
            result: Result<VsockType>,
        }

        let tests = &[
            TestData {
                socket_path: "",
                port: None,
                result: Err(anyhow!(ERR_HVSOCK_SOC_PATH_EMPTY)),
            },
            TestData {
                socket_path: "/foo/bar",
                port: None,
                result: Ok(VsockType::Hybrid {
                    socket_path: format!("/foo/bar_{}", DEFAULT_KATA_VSOCK_TRACING_PORT),
                }),
            },
            TestData {
                socket_path: "/foo/bar",
                port: Some(""),
                result: Err(anyhow!(ERR_VSOCK_PORT_EMPTY)),
            },
            TestData {
                socket_path: "/foo/bar",
                port: Some("foo bar"),
                result: Err(anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC)),
            },
            TestData {
                socket_path: "/foo/bar",
                port: Some("9"),
                result: Ok(VsockType::Hybrid {
                    socket_path: "/foo/bar_9".into(),
                }),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = handle_hybrid_vsock(d.socket_path, d.port);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
        }
    }

    #[test]
    fn test_handle_standard_vsock() {
        #[derive(Debug)]
        struct TestData<'a> {
            cid: Option<&'a str>,
            port: Option<&'a str>,
            result: Result<VsockType>,
        }

        let tests = &[
            TestData {
                cid: None,
                port: None,
                result: Ok(VsockType::Standard {
                    cid: VSOCK_CID_ANY,
                    port: DEFAULT_KATA_VSOCK_TRACING_PORT.parse::<u32>().unwrap(),
                }),
            },
            TestData {
                cid: Some(""),
                port: None,
                result: Err(anyhow!(ERR_VSOCK_CID_EMPTY)),
            },
            TestData {
                cid: Some("1"),
                port: Some(""),
                result: Err(anyhow!(ERR_VSOCK_PORT_EMPTY)),
            },
            TestData {
                cid: Some("1 foo"),
                port: None,
                result: Err(anyhow!(ERR_VSOCK_CID_NOT_NUMERIC)),
            },
            TestData {
                cid: None,
                port: Some("1 foo"),
                result: Err(anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC)),
            },
            TestData {
                cid: Some("1"),
                port: Some("0"),
                result: Err(anyhow!(ERR_VSOCK_PORT_ZERO)),
            },
            TestData {
                cid: Some("1"),
                port: None,
                result: Ok(VsockType::Standard {
                    cid: 1,
                    port: DEFAULT_KATA_VSOCK_TRACING_PORT.parse::<u32>().unwrap(),
                }),
            },
            TestData {
                cid: Some("123"),
                port: Some("999"),
                result: Ok(VsockType::Standard {
                    cid: 123,
                    port: 999,
                }),
            },
            TestData {
                cid: Some(VSOCK_CID_ANY_STR),
                port: Some("999"),
                result: Ok(VsockType::Standard {
                    cid: VSOCK_CID_ANY,
                    port: 999,
                }),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = handle_standard_vsock(d.cid, d.port);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
        }
    }
}
