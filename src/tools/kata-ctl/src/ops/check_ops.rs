// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::arch;
use crate::check;
use crate::ops::version;

use crate::args::{CheckArgument, CheckSubCommand, IptablesCommand, MetricsCommand};

use anyhow::Result;

const NAME: &str = "kata-ctl";

pub fn handle_check(checkcmd: CheckArgument) -> Result<()> {
    let command = checkcmd.command;

    match command {
        CheckSubCommand::All => {
            // run architecture-specific tests
            arch::check()?;

            // run code that uses network checks
            check::run_network_checks()?;
        }

        CheckSubCommand::NoNetworkChecks => {
            // run architecture-specific tests
            arch::check()?;
        }

        CheckSubCommand::CheckVersionOnly => {
            // retrieve latest release
            check::check_version()?;
        }

        CheckSubCommand::Network => {
            // run local network checks only
            check::run_network_checks()?;
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
