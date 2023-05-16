// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow Docker image config field names.
#![allow(non_snake_case)]

use crate::policy;

use anyhow::{anyhow, Result};
use log::{debug, info, LevelFilter};
use oci_distribution::client::{linux_amd64_resolver, ClientConfig};
use oci_distribution::{manifest, secrets::RegistryAuth, Client, Reference};
use serde::{Deserialize, Serialize};
use sha2::{digest::typenum::Unsigned, digest::OutputSizeUser, Sha256};
use std::{io, io::Seek, io::Write};
use tempfile::tempdir;
use tokio::{fs, io::AsyncWriteExt};

pub struct Container {
    config_layer: DockerConfigLayer,
    dm_verity_hashes: Vec<String>,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DockerRootfs {
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

        let (manifest, _digest_hash, config_layer_str) = client
            .pull_manifest_and_config(&reference, &RegistryAuth::Anonymous)
            .await?;

        info!(
            "manifest: {}",
            serde_json::to_string_pretty(&manifest).unwrap()
        );

        // Log the contents of the config layer.
        if log::max_level() >= LevelFilter::Info {
            println!("config layer:");
            let mut deserializer = serde_json::Deserializer::from_str(&config_layer_str);
            let mut serializer = serde_json::Serializer::pretty(io::stderr());
            serde_transcode::transcode(&mut deserializer, &mut serializer).unwrap();
            println!("");
        }

        Ok(Container {
            config_layer: serde_json::from_str(&config_layer_str)?,
            dm_verity_hashes: get_dm_verity_hashes(&mut client, &reference, &manifest).await?,
        })
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

    pub fn get_rootfs(&self) -> DockerRootfs {
        self.config_layer.rootfs.clone()
    }

    pub fn get_verity_hashes(&self) -> Vec<String> {
        self.dm_verity_hashes.clone()
    }
}

async fn get_dm_verity_hashes(
    client: &mut Client,
    reference: &Reference,
    manifest: &manifest::OciImageManifest,
) -> Result<Vec<String>> {
    let mut hashes = Vec::new();

    for layer in &manifest.layers {
        if layer
            .media_type
            .eq(manifest::IMAGE_DOCKER_LAYER_GZIP_MEDIA_TYPE)
        {
            hashes.push(get_dm_verity_hash(client, reference, layer).await?)
        }
    }

    Ok(hashes)
}

async fn get_dm_verity_hash(
    client: &mut Client,
    reference: &Reference,
    layer: &manifest::OciDescriptor,
) -> Result<String> {
    let base_dir = tempdir().unwrap();
    let mut file_path = base_dir.path().join("image_layer");
    file_path.set_extension("gz");

    {
        let mut file = tokio::fs::File::create(&file_path).await?;

        info!("Downloading layer {:?} to {:?}", &layer.digest, &file_path);
        if let Err(err) = client.pull_blob(&reference, &layer.digest, &mut file).await {
            drop(file);
            debug!("Download failed: {:?}", err);
            let _ = fs::remove_file(&file_path);
            return Err(anyhow!("unable to pull blob"));
        } else {
            file.flush().await.unwrap();
        }
    }

    info!("Decompressing {:?}", &file_path);
    if !tokio::process::Command::new("gunzip")
        .arg(&file_path)
        .arg("-f")
        .arg("-k")
        .spawn()?
        .wait()
        .await?
        .success()
    {
        let _ = fs::remove_file(&file_path);
        return Err(anyhow!("unable to decompress layer"));
    }

    {
        file_path.set_extension("");

        info!("Appending index to {:?}", &file_path);
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&file_path)?;
        tarindex::append_index(&mut file)?;
        file.flush().unwrap();
    }

    create_verity_hash(&file_path.to_string_lossy())
}

fn create_verity_hash(path: &str) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let size = file.seek(std::io::SeekFrom::End(0))?;
    if size < 4096 {
        return Err(anyhow!("Block device ({path}) is too small: {size}"));
    }

    let salt = [0u8; <Sha256 as OutputSizeUser>::OutputSize::USIZE];
    let v = verity::Verity::<Sha256>::new(size, 4096, 4096, &salt, None)?;
    let hash = verity::traverse_file(&file, 0, false, v)?;
    let result = format!("{:x}", hash);
    info!("dm-verity root hash: {:?}", &result);

    Ok(result)
}
