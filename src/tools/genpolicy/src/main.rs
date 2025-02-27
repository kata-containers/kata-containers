// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use log::{debug, info};

mod config_map;
mod containerd;
mod cronjob;
mod daemon_set;
mod deployment;
mod job;
mod list;
mod mount_and_storage;
mod no_policy;
mod obj_meta;
mod persistent_volume_claim;
mod pod;
mod pod_template;
mod policy;
mod registry;
mod registry_containerd;
mod replica_set;
mod replication_controller;
mod secret;
mod settings;
mod stateful_set;
mod utils;
mod verity;
mod version;
mod volume;
mod yaml;

#[tokio::main]
async fn main() {
    env_logger::init();
    let config = utils::Config::new();

    if config.version {
        println!(
            "Kata Containers policy tool (Rust): id: {}, version: {}, commit: {}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            version::COMMIT_INFO
        );
        return;
    }

    debug!("Creating policy from yaml, settings, and rules.rego files...");
    let mut policy = policy::AgentPolicy::from_files(&config).await.unwrap();

    debug!("Exporting policy to yaml file...");
    policy.export_policy();
    info!("Success!");
}
