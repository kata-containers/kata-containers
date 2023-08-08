// Copyright (c) 2023 Gabe Venberg
//
// SPDX-License-Identifier: Apache-2.0

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name="kata-log-parser", author, version, about, long_about = None)] // Read from `Cargo.toml`
pub struct Cli {
    pub input_file: Vec<PathBuf>,

    #[arg(short, long)]
    pub output_file: Option<PathBuf>,

    #[arg(short, long, help = "check log files and only display output on error")]
    pub check_only: bool,

    #[arg(long, help = "error if any files are empty")]
    pub error_if_file_empty: bool,

    #[arg(long, help = "error if all logfiles are empty")]
    pub error_if_no_records: bool,

    #[arg(
        long,
        help = "do not make an error for lines with no pid, source, name, or level"
    )]
    pub ignore_missing_fields: bool,

    #[arg(
        short,
        long,
        help = "suppress warning messages that would otherwise go to stderr."
    )]
    pub quiet: bool,

    #[arg(
        short,
        long,
        help = "do not tolerate misformed agent messages (may be caused by non-Kata Containers log lines)"
    )]
    pub strict: bool,

    #[arg(long, value_enum, default_value_t = OutputFormat::Json, help="set the output format")]
    pub output_format: OutputFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum OutputFormat {
    Csv,
    Json,
    Ron,
    Text,
    Toml,
    Xml,
    Yaml,
}
