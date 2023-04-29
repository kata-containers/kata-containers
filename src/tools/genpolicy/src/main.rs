// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use clap::Parser;
use env_logger;
use log::info;

mod config_maps;
mod containerd;
mod infra;
mod kata;
mod policy;
mod registry;
mod utils;
mod yaml;

#[derive(Debug, Parser)]
struct CommandLineOptions {
    #[clap(short, long)]
    yaml_file: Option<String>,

    #[clap(short, long)]
    input_files_path: Option<String>,

    #[clap(short, long)]
    output_policy_file: Option<String>,

    #[clap(short, long)]
    config_map_file: Option<String>,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = CommandLineOptions::parse();

    let mut config_map_files = Vec::new();
    if let Some(config_map_file) = &args.config_map_file {
        config_map_files.push(config_map_file.clone());
    }

    let in_out_files = utils::InOutFiles::new(
        args.yaml_file,
        args.input_files_path,
        args.output_policy_file,
        &config_map_files,
    );

    info!("Creating policy from yaml, infra data and rules files...");
    let mut policy = policy::PodPolicy::from_files(&in_out_files).unwrap();

    info!("Exporting policy to yaml file...");
    if let Err(e) = policy.export_policy(&in_out_files).await {
        println!("export_policy failed: {:?}", e);
        std::process::exit(1);
    }

    info!("Success!");
}
