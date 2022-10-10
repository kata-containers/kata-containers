// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use clap::{Args, Parser, Subcommand};

use thiserror::Error;

#[derive(Parser, Debug)]
#[clap(name = "kata-ctl", author, about = "Kata Containers control tool")]
pub struct KataCtlCli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Tests if system can run Kata Containers
    Check(CheckArgument),

    /// Directly assign a volume to Kata Containers to manage
    DirectVolume,

    /// Display settings
    Env,

    /// Enter into guest by debug console
    Exec,

    /// Manage vm factory
    Factory,

    /// Manages iptables
    Iptables(IptablesCommand),

    /// Gather metrics associated with infrastructure used to run a sandbox
    Metrics(MetricsCommand),

    /// Display version details
    Version,
}

#[derive(Debug, Args, Error)]
#[error("Argument is not valid")]
pub struct CheckArgument {
    #[clap(subcommand)]
    pub command: CheckSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum CheckSubCommand {
    /// Runs all checks
    All,

    /// Runs all checks but excluding network checks.
    NoNetworkChecks,

    /// Only compare the current and latest available versions
    CheckVersionOnly,
}

#[derive(Debug, Args)]
pub struct MetricsCommand {
    #[clap(subcommand)]
    pub metrics_cmd: MetricsSubCommand,
}

#[derive(Debug, Subcommand)]
pub enum MetricsSubCommand {
    /// Arguments for metrics
    MetricsArgs,
}

// #[derive(Parser, Debug)]
#[derive(Debug, Args)]
pub struct IptablesCommand {
    #[clap(subcommand)]
    pub iptables: IpTablesArguments,
}

#[derive(Debug, Subcommand)]
pub enum IpTablesArguments {
    /// Configure iptables
    Metrics,
}
