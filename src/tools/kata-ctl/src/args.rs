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
    /// Test if system can run Kata Containers
    Check(CheckArgument),

    /// Directly assign a volume to Kata Containers to manage
    DirectVolume(DirectVolumeCommand),

    /// Display settings
    Env,

    /// Enter into guest VM by debug console
    Exec,

    /// Manage VM factory
    Factory,

    /// Manage guest VM iptables
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
    /// Run all checks
    All,

    /// Run all checks but excluding network checks.
    NoNetworkChecks,

    /// Only compare the current and latest available versions
    CheckVersionOnly,

    /// List official release packages
    OnlyListReleases,

    /// List all official and pre-release packages
    IncludeAllReleases,

    /// List all available checks
    List,
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

#[derive(Debug, Args)]
pub struct DirectVolumeCommand {
    #[clap(subcommand)]
    pub directvol_cmd: DirectVolSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum DirectVolSubcommand {
    /// Add a direct assigned block volume device to the Kata Containers runtime
    Add(DirectVolAddArgs),

    /// Remove a direct assigned block volume device from the Kata Containers runtime
    Remove(DirectVolRemoveArgs),

    /// Get the filesystem stat of a direct assigned volume
    Stats(DirectVolStatsArgs),

    /// Resize a direct assigned block volume
    Resize(DirectVolResizeArgs),
}

#[derive(Debug, Args)]
pub struct DirectVolAddArgs {
    pub volume_path: String,
    pub mount_info: String,
}

#[derive(Debug, Args)]
pub struct DirectVolRemoveArgs {
    pub volume_path: String,
}

#[derive(Debug, Args)]
pub struct DirectVolStatsArgs {
    pub volume_path: String,
}

#[derive(Debug, Args)]
pub struct DirectVolResizeArgs {
    pub volume_path: String,
    pub resize_size: u64,
}
