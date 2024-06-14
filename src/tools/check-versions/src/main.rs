// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

mod model;
mod version_checker;
mod error;
mod cli;
mod output;

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use std::fs;
use std::process::exit;

fn real_main() -> Result<()> {
    let args = cli::Args::parse();

    let contents = fs::read_to_string(&args.versions_file)
        .context(format!("Unable to read {}", &args.versions_file.display()))?;

    let versions: serde_json::Value = serde_yaml::from_str(contents.as_str())
        .context(format!("Unable to parse {}", &args.versions_file.display()))?;

    let results: Vec<model::CheckResult> = version_checker::check_versions("root", &versions, &args)?;

    output::output_results(&results, &args)?;

    Ok(())
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("ERROR: {:#?}", e);
        exit(1)
    }
}
