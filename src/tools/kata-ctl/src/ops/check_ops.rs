// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::arch::x86_64::get_checks;

use crate::args::{CheckArgument, CheckSubCommand, IptablesCommand, MetricsCommand};

use crate::check;

use crate::ops::version;

use crate::types::*;

use anyhow::{anyhow, Result};

const NAME: &str = "kata-ctl";

// This function retrieves the cmd function passes as argument
fn get_builtin_check_func(name: CheckType) -> Result<BuiltinCmdFp> {
    let check_list = get_checks();

    for check in check_list {
        if check.name.eq(&name) {
            return Ok(check.fp);
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
    let check_list = get_checks();

    for cmd in check_list {
        cmds.push(format!("{} ({}. Mode: {})", cmd.name, cmd.descr, cmd.perm));
    }

    cmds
}

fn print_check_list() -> Result<()> {
    let cmds = get_client_cmd_details();

    if cmds.is_empty() {
        println!("Checks not found!\n");

        return Ok(());
    }

    cmds.iter().for_each(|n| println!(" - {}", n));

    println!();

    Ok(())
}

pub fn handle_check(checkcmd: CheckArgument) -> Result<()> {
    let command = checkcmd.command;

    match command {
        CheckSubCommand::All => {
            // run architecture-specific tests
            handle_builtin_check(CheckType::CheckCpu, "")?;

            // run code that uses network checks
            check::run_network_checks()?;
        }

        CheckSubCommand::NoNetworkChecks => {
            // run architecture-specific tests
            handle_builtin_check(CheckType::CheckCpu, "")?;
        }

        CheckSubCommand::CheckVersionOnly => {
            handle_version()?;
        }

        CheckSubCommand::List => {
            print_check_list()?;
        }
        CheckSubCommand::OnlyListReleases => {
            // retrieve official release
            #[cfg(any(
                target_arch = "aarch64",
                target_arch = "powerpc64le",
                target_arch = "x86_64"
            ))]
            check::check_official_releases()?;
        }
        CheckSubCommand::IncludeAllReleases => {
            // retrieve ALL releases including prerelease
            #[cfg(any(
                target_arch = "aarch64",
                target_arch = "powerpc64le",
                target_arch = "x86_64"
            ))]
            check::check_all_releases()?;
        }
    }

    Ok(())
}

pub fn handle_check_volume() -> Result<()> {
    Ok(())
}

pub fn handle_env() -> Result<()> {
    Ok(())
}

pub fn handle_exec() -> Result<()> {
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

pub fn handle_version() -> Result<()> {
    let version = version::get().unwrap();

    println!("{} version {:?} (type: rust)", NAME, version);
    Ok(())
}
