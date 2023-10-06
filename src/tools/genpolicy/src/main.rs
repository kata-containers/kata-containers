// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use clap::Parser;
use env_logger;
use log::{debug, info};

mod config_map;
mod containerd;
mod daemon_set;
mod deployment;
mod infra;
mod job;
mod kata;
mod list;
mod no_policy;
mod obj_meta;
mod pause_container;
mod persistent_volume_claim;
mod pod;
mod pod_template;
mod policy;
mod registry;
mod replica_set;
mod replication_controller;
mod secret;
mod stateful_set;
mod utils;
mod volume;
mod yaml;

#[derive(Debug, Parser)]
struct CommandLineOptions {
    #[clap(short, long, help = "Kubernetes YAML input file path")]
    yaml_file: Option<String>,

    #[clap(
        short,
        long,
        help = "Optional Kubernetes config map YAML input file path"
    )]
    config_map_file: Option<String>,

    #[clap(
        short = 'j',
        long,
        default_value_t = String::from("genpolicy-settings.json"),
        help = "genpolicy settings file name"
    )]
    settings_file_name: String,

    #[clap(
        short,
        long,
        default_value_t = String::from("."),
        help = "Path to the rules.rego and settings input files"
    )]
    input_files_path: String,

    #[clap(
        short,
        long,
        help = "Create and use a cache of container image layer contents and dm-verity information (in ./layers_cache/)"
    )]
    use_cached_files: bool,

    #[clap(
        short,
        long,
        help = "Print the output Rego policy text to standard output"
    )]
    raw_out: bool,

    #[clap(
        short,
        long,
        help = "Print the base64 encoded output Rego policy to standard output"
    )]
    base64_out: bool,

    #[clap(
        short,
        long,
        help = "Ignore unsupported input Kubernetes YAML fields. This is not recommeded unless you understand exactly how genpolicy works!"
    )]
    silent_unsupported_fields: bool,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let args = CommandLineOptions::parse();

    let mut config_map_files = Vec::new();
    if let Some(config_map_file) = &args.config_map_file {
        config_map_files.push(config_map_file.clone());
    }

    let config = utils::Config::new(
        args.use_cached_files,
        args.yaml_file,
        &args.input_files_path,
        &args.settings_file_name,
        &config_map_files,
        args.silent_unsupported_fields,
        args.raw_out,
        args.base64_out,
    );

    debug!("Creating policy from yaml, settings, and rules.rego files...");
    let mut policy = policy::AgentPolicy::from_files(&config).await.unwrap();

    debug!("Exporting policy to yaml file...");
    policy.export_policy();
    info!("Success!");
}
