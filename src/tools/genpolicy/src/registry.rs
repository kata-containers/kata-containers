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

pub struct Container {
    config_layer: DockerConfigLayer,
}

#[derive(Debug, Serialize, Deserialize)]
struct DockerConfigLayer {
    architecture: String,
    config: DockerImageConfig,
    rootfs: DockerRootfs,
}

#[derive(Debug, Serialize, Deserialize)]
struct DockerImageConfig {
    User: Option<String>,
    Tty: Option<bool>,
    Env: Vec<String>,
    Cmd: Option<Vec<String>>,
    WorkingDir: Option<String>,
    Entrypoint: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DockerRootfs {
    r#type: String,
    diff_ids: Vec<String>,
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

        Ok(Container { config_layer: serde_json::from_str(&config_layer)?})
    }

    // Convert Docker image config to policy data.
    pub fn get_process(
        &self,
        process: &mut policy::OciProcess,
        yaml_has_command: bool,
        yaml_has_args: bool,
    ) -> Result<()> {
        info!("Getting process field from docker config layer...");
        let docker_config = &self.config_layer.config;

        if let Some(image_user) = &docker_config.User {
            if !image_user.is_empty() {
                info!("Splitting Docker config user = {:?}", image_user);
                let user: Vec<&str> = image_user.split(':').collect();
                if !user.is_empty() {
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
        }

        if let Some(terminal) = docker_config.Tty {
            process.terminal = terminal;
        } else {
            process.terminal = false;
        }

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
        } else if let Some(commands) = &docker_config.Cmd {
            info!("Adding to policy args the image Cmd: {:?}", commands);

            for cmd in commands {
                policy_args.push(cmd.clone());
            }
        } else {
            info!("Image Cmd field is not present");
        }

        info!("Updated policy args: {:?}", policy_args);

        if let Some(working_dir) = &docker_config.WorkingDir {
            if !working_dir.is_empty() {
                process.cwd = working_dir.clone();
            }
        }

        info!("get_process succeeded.");
        Ok(())
    }
}
