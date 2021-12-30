// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use kata::{ShimArgs, ShimExecutor};
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

#[derive(Debug)]
enum Action {
    Run(ShimArgs),
    Start(ShimArgs),
    Delete(ShimArgs),
    Error(String),
    Help,
    Version,
}

fn parse_args(args: &[OsString]) -> Action {
    let mut help = false;
    let mut version = false;
    let mut shim_args = ShimArgs::default();

    // Crate `go_flag` is used to keep compatible with go/flag package.
    match go_flag::parse_args_with_warnings::<String, _, _>(&args[1..], None, |flags| {
        flags.add_flag("address", &mut shim_args.address);
        flags.add_flag("bundle", &mut shim_args.bundle);
        flags.add_flag("debug", &mut shim_args.debug);
        flags.add_flag("id", &mut shim_args.id);
        flags.add_flag("namespace", &mut shim_args.namespace);
        flags.add_flag("publish-binary", &mut shim_args.publish_binary);
        flags.add_flag("help", &mut help);
        flags.add_flag("version", &mut version);
    }) {
        Ok(rest_args) => {
            if help {
                Action::Help
            } else if version {
                Action::Version
            } else if rest_args.is_empty() {
                Action::Run(shim_args)
            } else if rest_args[0] == "start" {
                Action::Start(shim_args)
            } else if rest_args[0] == "delete" {
                Action::Delete(shim_args)
            } else {
                Action::Error(format!("unknown parameters: {}", rest_args.join(" ")))
            }
        }

        Err(e) => Action::Error(format!("{}", e)),
    }
}

fn show_help(cmd: &OsStr) {
    let path = PathBuf::from(cmd);
    let name = match path.file_name() {
        Some(v) => v.to_str(),
        None => None,
    };

    let name = name.unwrap_or("containerd-shim-kata-v2");

    eprintln!(
        r#"Usage of {}:
    -address string
          grpc address back to main containerd
    -bundle string
          path to the bundle if not containerd workdir
    -debug
          enable debug output in logs (e.g. run containerd in debug mode)
    -id string
          id of the task
    -namespace string
          namespace that owns the shim
    -publish-binary string
          path to publish binary (used for publishing events) (default "containerd")
    -help
          show help
    -version
          show verion
"#,
        name
    );
}

fn show_version() {
    eprintln!(
        r#"Kata runtime rust version:
    Build Version: {}
    Commit SHA: {}
    Rustc Version: {}
"#,
        env!("VERGEN_BUILD_SEMVER"),
        env!("VERGEN_GIT_SHA"),
        env!("VERGEN_RUSTC_SEMVER"),
    );
}

fn main() {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.is_empty() {
        eprintln!("invalid commandline arguments");
        return;
    }

    let action = parse_args(&args);

    match action {
        Action::Start(args) => ShimExecutor::new(args).start(),
        Action::Delete(args) => ShimExecutor::new(args).delete(),
        Action::Run(args) => ShimExecutor::new(args).run(),
        Action::Error(estr) => {
            eprintln!("{}", estr);
            show_help(&args[0]);
        }
        Action::Help => show_help(&args[0]),
        Action::Version => show_version(),
    }
}
