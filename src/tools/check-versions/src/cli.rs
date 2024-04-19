// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
/// Version checking utility to identify which project components may need upgrading to the latest
/// version. Currently only some types of components can be automatically checked for updates - this is not an
/// exhaustive list.
pub struct Args {
    #[arg(short, long, default_value = "../../../versions.yaml")]
    /// The versions yaml file listing components to check for updates.
    /// Intended to be used with the versions.yaml file in the root of the kata-containers project.
    pub versions_file: PathBuf,

    #[arg(long, default_value_t = false)]
    /// If specified, output will not be printed to the console. Useful with --outfile
    pub verbose: bool,

    #[arg(long, default_value_t = false)]
    /// If true, items which are up to date will not be output
    pub suppress_uptodate: bool,

    #[arg(long, default_value_t = false)]
    /// If true, items which are out of date will not be output
    pub suppress_outofdate: bool,

    #[arg(long, default_value_t = false)]
    /// If true, items which fail to validate will not be output
    pub suppress_errors: bool,

    #[arg(short, long, env = "GITHUB_TOKEN")]
    /// GitHub authentication token to enable more API requests per hour.
    /// Can also be set via GITHUB_TOKEN environment variable.
    /// This argument overrides the environment variable if both are specified.
    pub github_token: Option<String>,
}
