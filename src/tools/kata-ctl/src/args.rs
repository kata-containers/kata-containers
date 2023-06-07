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
    #[clap(short, long, value_enum, value_parser = parse_log_level)]
    /// Sets the minimum log level required for log messages to be displayed. Default is 'info'.
    /// Valid values are: trace, debug, info, warning, error, critical
    pub log_level: Option<slog::Level>,
    #[clap(short, long, action)]
    /// If enabled, log messages will be JSON formatted for easier machine parsing
    pub json_logging: bool,
}

fn parse_log_level(arg: &str) -> Result<slog::Level, String> {
    match arg {
        "trace" => Ok(slog::Level::Trace),
        "debug" => Ok(slog::Level::Debug),
        "info" => Ok(slog::Level::Info),
        "warning" => Ok(slog::Level::Warning),
        "error" => Ok(slog::Level::Error),
        "critical" => Ok(slog::Level::Critical),
        _ => Err("Must be one of [trace, debug, info, warning, error, critical]".to_string()),
    }
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Test if system can run Kata Containers
    Check(CheckArgument),

    /// Directly assign a volume to Kata Containers to manage
    DirectVolume(DirectVolumeCommand),

    /// Display settings
    Env(EnvArgument),

    /// Enter into guest VM by debug console
    Exec(ExecArguments),

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
pub struct EnvArgument {
    /// Format output as JSON
    #[arg(long)]
    pub json: bool,
    /// File to write env output to
    #[arg(short = 'f', long = "file")]
    pub file: Option<String>,
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

#[derive(Debug, Args)]
pub struct ExecArguments {
    /// pod sandbox ID.
    pub sandbox_id: String,
    #[clap(short = 'p', long = "kata-debug-port", default_value_t = 1026)]
    /// kata debug console vport same as configuration, default is 1026.
    pub vport: u32,
}
