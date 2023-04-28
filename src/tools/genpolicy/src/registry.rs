// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow Docker image config field names.
#![allow(non_snake_case)]

use crate::policy;

use anyhow::Result;
use log::{info, LevelFilter};
use oci_distribution::client::{linux_amd64_resolver, ClientConfig};
use oci_distribution::{secrets::RegistryAuth, Client, Reference};
use serde::{Deserialize, Serialize};
use std::io;

#[derive(Default)]
pub struct Container {
    config_layer: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DockerConfigLayer {
    architecture: String,
    #[serde(default)]
    config: DockerImageConfig,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DockerImageConfig {
    #[serde(default)]
    User: String,

    #[serde(default)]
    Tty: bool,

    #[serde(default)]
    Env: Vec<String>,

    #[serde(default)]
    Cmd: Vec<String>,

    #[serde(default)]
    WorkingDir: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    Entrypoint: Option<Vec<String>>,
}

impl Container {
    pub async fn new(image: &str) -> Result<Self> {
        info!("============================================");
        info!("Pulling manifest and config for {:?}", image);
        let reference: Reference = image.to_string().parse().unwrap();
        let mut client = Client::new(ClientConfig {
            platform_resolver: Some(Box::new(linux_amd64_resolver)),
            ..Default::default()
        });

        let (manifest, _digest_hash, config_layer) = client
            .pull_manifest_and_config(&reference, &RegistryAuth::Anonymous)
            .await?;

        info!(
            "manifest: {}",
            serde_json::to_string_pretty(&manifest).unwrap()
        );

        // Log the contents of the config layer.
        if log::max_level() >= LevelFilter::Info {
            println!("config layer:");
            let mut deserializer = serde_json::Deserializer::from_str(&config_layer);
            let mut serializer = serde_json::Serializer::pretty(io::stderr());
            serde_transcode::transcode(&mut deserializer, &mut serializer).unwrap();
        }

        Ok(Container { config_layer })
    }

    // Convert Docker image config to policy data.
    pub fn get_process(
        &self,
        process: &mut policy::OciProcess,
        yaml_has_command: bool,
        yaml_has_args: bool,
    ) -> Result<()> {
        info!("Getting process field from docker config layer...");
        let config_layer: DockerConfigLayer = serde_json::from_str(&self.config_layer)?;
        let docker_config = &config_layer.config;

        if !docker_config.User.is_empty() {
            info!("Splitting Docker config user = {:?}", docker_config.User);
            let user: Vec<&str> = docker_config.User.split(':').collect();
            if user.len() > 0 {
                info!("Parsing user[0] = {:?}", user[0]);
                process.user.uid = user[0].parse()?;
                info!("string: {:?} => uid: {}", user[0], process.user.uid);
            }
            if user.len() > 1 {
                info!("Parsing user[1] = {:?}", user[1]);
                process.user.gid = user[1].parse()?;
                info!("string: {:?} => gid: {}", user[1], process.user.gid);
            }
        }

        process.terminal = docker_config.Tty;

        for env in &docker_config.Env {
            process.env.push(env.clone());
        }

        let policy_args = &mut process.args;
        info!("Already existing policy args: {:?}", policy_args);

        if let Some(entry_points) = &docker_config.Entrypoint {
            info!("Image Entrypoint: {:?}", entry_points);
            if !yaml_has_command {
                    info!("Inserting Entrypoint into policy args");

                    let mut reversed_entry_points = entry_points.clone();
                    reversed_entry_points.reverse();

                    for entry_point in reversed_entry_points {
                        policy_args.insert(0, entry_point.clone());
                    }
            } else {
                info!("Ignoring image Entrypoint because YAML specified the container command");
            }
        } else {
            info!("No image Entrypoint");
        }

        info!("Updated policy args: {:?}", policy_args);

        if yaml_has_command {
            info!("Ignoring image Cmd because YAML specified the container command");
        } else if yaml_has_args {
            info!("Ignoring image Cmd because YAML specified the container args");
        } else {
            info!("Adding to policy args the image Cmd: {:?}", docker_config.Cmd);

            for cmd in &docker_config.Cmd {
                policy_args.push(cmd.clone());
            }
        }

        info!("Updated policy args: {:?}", policy_args);

        if !docker_config.WorkingDir.is_empty() {
            process.cwd = docker_config.WorkingDir.clone();
        }

        info!("get_process succeeded.");
        Ok(())
    }
}
