// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use clap::Parser;
use env_logger;
use log::info;

mod containerd;
mod infra;
mod kata;
mod yaml;
mod policy;
mod registry;
mod utils;

#[derive(Debug, Parser)]
struct CommandLineOptions {
    #[clap(short, long)]
    yaml_file: String,

    #[clap(short, long)]
    input_files_path: Option<String>,

    #[clap(short, long)]
    output_policy_file: Option<String>,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = CommandLineOptions::parse();

    let mut input_files_path = ".".to_string();
    if let Some(path) = args.input_files_path {
        input_files_path = path.clone();
    }
    let rules_file = input_files_path.to_owned() + "/rules.rego";
    info!("Rules file: {:?}", &rules_file);

    let infra_data_file = input_files_path.to_owned() + "/data.json";
    info!("Infra data file: {:?}", &infra_data_file);

    info!("Creating policy from yaml, infra data and rules files...");
    let policy = policy::PodPolicy::from_files(
        &args.yaml_file,
        &rules_file,
        &infra_data_file,
    )
    .unwrap();

    info!("Exporting policy to yaml file...");
    if let Err(e) = policy.export_policy(args.output_policy_file).await {
        println!("export_policy failed: {:?}", e);
        std::process::exit(1);
    }

    info!("Success!");
}
