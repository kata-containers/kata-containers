// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

mod model;
mod version_checker;
mod error;
mod cli;
mod output;

use clap::Parser;
use std::fs;

fn main() {
    let args = cli::Args::parse();

    let contents = match fs::read_to_string(&args.versions_file) {
        Ok(contents) => contents,
        Err(_e) => {
            println!("Unable to read {}", &args.versions_file.display());
            return;
        }
    };

    let versions: serde_json::Value = match serde_yaml::from_str(contents.as_str()) {
        Ok(versions) => versions,
        Err(_e) => {
            println!("Unable to parse {}", &args.versions_file.display());
            return;
        }
    };

    let results: Vec<model::CheckResult> = version_checker::check_versions("root", &versions, &args);

    for r in &results {
        if (!r.up_to_date) {
            println!("{}\n\tcurrent_version: {}\n\tlatest_version: {}",
                r.project_name, r.current_version, r.latest_version);
        }
    }
}

