// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

use anyhow::{anyhow, Context, Result};
use nix::{
    mount::{mount, MsFlags},
    sched::{self, CloneFlags},
};
use shim::{config, Args, Error, ShimExecutor};

// default tokio runtime worker threads
const DEFAULT_TOKIO_RUNTIME_WORKER_THREADS: usize = 2;
// env to config tokio runtime worker threads
const ENV_TOKIO_RUNTIME_WORKER_THREADS: &str = "TOKIO_RUNTIME_WORKER_THREADS";

#[derive(Debug)]
enum Action {
    Run(Args),
    Start(Args),
    Delete(Args),
    Help,
    Version,
}

fn parse_args(args: &[OsString]) -> Result<Action> {
    let mut help = false;
    let mut version = false;
    let mut shim_args = Args::default();

    // Crate `go_flag` is used to keep compatible with go/flag package.
    let rest_args = go_flag::parse_args_with_warnings::<String, _, _>(&args[1..], None, |flags| {
        flags.add_flag("address", &mut shim_args.address);
        flags.add_flag("bundle", &mut shim_args.bundle);
        flags.add_flag("debug", &mut shim_args.debug);
        flags.add_flag("id", &mut shim_args.id);
        flags.add_flag("namespace", &mut shim_args.namespace);
        flags.add_flag("publish-binary", &mut shim_args.publish_binary);
        flags.add_flag("help", &mut help);
        flags.add_flag("version", &mut version);
    })
    .context(Error::ParseArgument(format!("{:?}", args)))?;

    if help {
        Ok(Action::Help)
    } else if version {
        Ok(Action::Version)
    } else if rest_args.is_empty() {
        Ok(Action::Run(shim_args))
    } else if rest_args[0] == "start" {
        Ok(Action::Start(shim_args))
    } else if rest_args[0] == "delete" {
        Ok(Action::Delete(shim_args))
    } else {
        Err(anyhow!(Error::InvalidArgument))
    }
}

fn show_help(cmd: &OsStr) {
    let path = PathBuf::from(cmd);
    let name = match path.file_name() {
        Some(v) => v.to_str(),
        None => None,
    };

    let name = name.unwrap_or(config::RUNTIME_NAME);

    println!(
        r#"Usage of {}:
  -address string
        grpc address back to main containerd
  -bundle string
        path to the bundle if not workdir
  -debug
        enable debug output in logs
  -id string
        id of the task
  -namespace string
        namespace that owns the shim
  -publish-binary string
        path to publish binary (used for publishing events) (default "containerd")
  --version
        show the runtime version detail and exit
"#,
        name
    );
}

fn show_version(err: Option<anyhow::Error>) {
    let data = format!(
        r#"{} containerd shim (Rust): id: {}, version: {}, commit: {}"#,
        config::PROJECT_NAME,
        config::CONTAINERD_RUNTIME_NAME,
        config::RUNTIME_VERSION,
        config::RUNTIME_GIT_COMMIT,
    );

    if let Some(err) = err {
        eprintln!(
            "{}\r\nERROR: {} failed: {:?}",
            data,
            config::RUNTIME_NAME,
            err
        );
    } else {
        println!("{}", data)
    }
}

fn get_tokio_runtime() -> Result<tokio::runtime::Runtime> {
    let worker_threads = std::env::var(ENV_TOKIO_RUNTIME_WORKER_THREADS)
        .unwrap_or_default()
        .parse()
        .unwrap_or(DEFAULT_TOKIO_RUNTIME_WORKER_THREADS);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_threads)
        .enable_all()
        .build()
        .context("prepare tokio runtime")?;
    Ok(rt)
}

fn real_main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.is_empty() {
        return Err(anyhow!(Error::ArgumentIsEmpty(
            "command-line arguments".to_string()
        )));
    }

    let action = parse_args(&args).context("parse args")?;
    match action {
        Action::Start(args) => ShimExecutor::new(args).start().context("shim start")?,
        Action::Delete(args) => {
            let mut shim = ShimExecutor::new(args);
            let rt = get_tokio_runtime().context("get tokio runtime")?;
            rt.block_on(shim.delete())?;
        }
        Action::Run(args) => {
            // set mnt namespace
            // need setup before other async call
            setup_mnt().context("setup mnt")?;

            let mut shim = ShimExecutor::new(args);
            let rt = get_tokio_runtime().context("get tokio runtime")?;
            rt.block_on(shim.run())?;
        }
        Action::Help => show_help(&args[0]),
        Action::Version => show_version(None),
    }
    Ok(())
}
fn main() {
    if let Err(err) = real_main() {
        show_version(Some(err));
    }
}

fn setup_mnt() -> Result<()> {
    // Unshare the mount namespace, so that the calling process has a private copy of its namespace
    // which is not shared with any other process.
    sched::unshare(CloneFlags::CLONE_NEWNS).context("unshare clone newns")?;

    // Mount and unmount events propagate into this mount from the (master) shared peer group of
    // which it was formerly a member. Mount and unmount events under this mount do not propagate
    // to any peer.
    mount(
        Some("none"),
        "/",
        Some(""),
        MsFlags::MS_REC | MsFlags::MS_SLAVE,
        Some(""),
    )
    .context("mount with slave")?;

    // Mount and unmount events immediately under this mount will propagate to the other mounts
    // that are members of this mount's peer group.
    mount(
        Some("none"),
        "/",
        Some(""),
        MsFlags::MS_REC | MsFlags::MS_SHARED,
        Some(""),
    )
    .context("mount with shared")?;
    Ok(())
}
