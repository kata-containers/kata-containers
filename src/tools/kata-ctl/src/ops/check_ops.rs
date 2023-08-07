// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::arch::arch_specific::get_checks;

use crate::args::{
    CheckArgument, CheckSubCommand, IptablesCommand, MetricsCommand, MonitorArgument,
};

use crate::check;

use crate::monitor::http_server;

use crate::ops::version;

use crate::types::*;

use anyhow::{anyhow, Context, Result};

const MONITOR_DEFAULT_SOCK_ADDR: &str = "127.0.0.1:8090";

use slog::{info, o, warn};

const NAME: &str = "kata-ctl";

macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "check_ops"))
    };
}

// This function retrieves the cmd function passes as argument
fn get_builtin_check_func(name: CheckType) -> Result<BuiltinCmdFp> {
    if let Some(check_list) = get_checks() {
        for check in check_list {
            if check.name.eq(&name) {
                return Ok(check.fp);
            }
        }
    }

    Err(anyhow!("Invalid command: {:?}", name))
}

// This function is called from each 'kata-ctl check' argument section
fn handle_builtin_check(check: CheckType, args: &str) -> Result<()> {
    let f = match get_builtin_check_func(check) {
        Ok(fp) => fp,
        Err(e) => return Err(e),
    };

    f(args)
}

fn get_client_cmd_details() -> Vec<String> {
    let mut cmds = Vec::new();
    if let Some(check_list) = get_checks() {
        for cmd in check_list {
            cmds.push(format!("{} ({}. Mode: {})", cmd.name, cmd.descr, cmd.perm));
        }
    }

    cmds
}

fn print_check_list() -> Result<()> {
    let cmds = get_client_cmd_details();

    if cmds.is_empty() {
        warn!(sl!(), "Checks not found!\n");

        return Ok(());
    }

    cmds.iter().for_each(|n| info!(sl!(), " - {}", n));

    Ok(())
}

pub fn handle_check(checkcmd: CheckArgument) -> Result<()> {
    let command = checkcmd.command;

    match command {
        CheckSubCommand::All => {
            // run architecture-specific tests
            handle_builtin_check(CheckType::Cpu, "")?;

            // run code that uses network checks
            check::run_network_checks()?;

            // run kernel module checks
            handle_builtin_check(CheckType::KernelModules, "")?;

            // run kvm checks
            handle_builtin_check(CheckType::KvmIsUsable, "")?;
        }

        CheckSubCommand::NoNetworkChecks => {
            // run architecture-specific tests
            handle_builtin_check(CheckType::Cpu, "")?;
        }

        CheckSubCommand::CheckVersionOnly => {
            handle_version()?;
        }

        CheckSubCommand::List => {
            print_check_list()?;
        }
        CheckSubCommand::OnlyListReleases => {
            // retrieve official release
            check::check_official_releases()?;
        }
        CheckSubCommand::IncludeAllReleases => {
            // retrieve ALL releases including prerelease
            check::check_all_releases()?;
        }
    }

    Ok(())
}

pub fn handle_factory() -> Result<()> {
    Ok(())
}

pub fn handle_iptables(_args: IptablesCommand) -> Result<()> {
    Ok(())
}

pub fn handle_metrics(_args: MetricsCommand) -> Result<()> {
    Ok(())
}

pub fn handle_monitor(monitor_args: MonitorArgument) -> Result<()> {
    tokio::runtime::Runtime::new()
        .context("failed to new runtime for aync http server")?
        .block_on(http_server::http_server_setup(
            monitor_args
                .address
                .as_deref()
                .unwrap_or(MONITOR_DEFAULT_SOCK_ADDR),
        ))
}

pub fn handle_version() -> Result<()> {
    let version = version::get().unwrap();

    info!(sl!(), "{} version {:?} (type: rust)", NAME, version);
    Ok(())
}
