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

    match version_checker::check_versions_recursive("root", &versions, &args) {
        Err(error) => {
            println!("Unable to check versions in {}: {:?}", &args.versions_file.display(), error);
            return;
        },
        _ => ()
    }
}

