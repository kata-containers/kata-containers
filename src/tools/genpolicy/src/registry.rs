// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow Docker image config field names.
#![allow(non_snake_case)]

use crate::pod;
use crate::policy;

use anyhow::{anyhow, Result};
use log::{debug, info, LevelFilter};
use oci_distribution::client::{linux_amd64_resolver, ClientConfig};
use oci_distribution::{manifest, secrets::RegistryAuth, Client, Reference};
use serde::{Deserialize, Serialize};
use sha2::{digest::typenum::Unsigned, digest::OutputSizeUser, Sha256};
use std::{io, io::Seek, io::SeekFrom, io::Write};
use tokio::{fs, io::AsyncWriteExt};

#[derive(Clone, Debug)]
pub struct Container {
    config_layer: DockerConfigLayer,
    image_layers: Vec<ImageLayer>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DockerConfigLayer {
    architecture: String,
    config: DockerImageConfig,
    rootfs: DockerRootfs,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DockerImageConfig {
    User: Option<String>,
    Tty: Option<bool>,
    Env: Vec<String>,
    Cmd: Option<Vec<String>>,
    WorkingDir: Option<String>,
    Entrypoint: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DockerRootfs {
    r#type: String,
    diff_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ImageLayer {
    pub diff_id: String,
    pub verity_hash: String,
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

        let (manifest, digest_hash, config_layer_str) = client
            .pull_manifest_and_config(&reference, &RegistryAuth::Anonymous)
            .await
            .unwrap();

        debug!("digest_hash: {:?}", digest_hash);
        debug!(
            "manifest: {}",
            serde_json::to_string_pretty(&manifest).unwrap()
        );

        // Log the contents of the config layer.
        if log::max_level() >= LevelFilter::Debug {
            let mut deserializer = serde_json::Deserializer::from_str(&config_layer_str);
            let mut serializer = serde_json::Serializer::pretty(io::stderr());
            serde_transcode::transcode(&mut deserializer, &mut serializer).unwrap();
        }

        let config_layer: DockerConfigLayer = serde_json::from_str(&config_layer_str).unwrap();
        let image_layers = get_image_layers(&mut client, &reference, &manifest, &config_layer)
            .await
            .unwrap();

        Ok(Container {
            config_layer,
            image_layers,
        })
    }

    // Convert Docker image config to policy data.
    pub fn get_process(
        &self,
        process: &mut policy::OciProcess,
        yaml_has_command: bool,
        yaml_has_args: bool,
    ) -> Result<()> {
        debug!("Getting process field from docker config layer...");
        let docker_config = &self.config_layer.config;

        if let Some(image_user) = &docker_config.User {
            if !image_user.is_empty() {
                debug!("Splitting Docker config user = {:?}", image_user);
                let user: Vec<&str> = image_user.split(':').collect();
                if !user.is_empty() {
                    debug!("Parsing user[0] = {:?}", user[0]);
                    process.user.uid = user[0].parse()?;
                    debug!("string: {:?} => uid: {}", user[0], process.user.uid);
                }
                if user.len() > 1 {
                    debug!("Parsing user[1] = {:?}", user[1]);
                    process.user.gid = user[1].parse()?;
                    debug!("string: {:?} => gid: {}", user[1], process.user.gid);
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
        debug!("Already existing policy args: {:?}", policy_args);

        if let Some(entry_points) = &docker_config.Entrypoint {
            debug!("Image Entrypoint: {:?}", entry_points);
            if !yaml_has_command {
                debug!("Inserting Entrypoint into policy args");

                let mut reversed_entry_points = entry_points.clone();
                reversed_entry_points.reverse();

                for entry_point in reversed_entry_points {
                    policy_args.insert(0, entry_point.clone());
                }
            } else {
                debug!("Ignoring image Entrypoint because YAML specified the container command");
            }
        } else {
            debug!("No image Entrypoint");
        }

        debug!("Updated policy args: {:?}", policy_args);

        if yaml_has_command {
            debug!("Ignoring image Cmd because YAML specified the container command");
        } else if yaml_has_args {
            debug!("Ignoring image Cmd because YAML specified the container args");
        } else if let Some(commands) = &docker_config.Cmd {
            debug!("Adding to policy args the image Cmd: {:?}", commands);

            for cmd in commands {
                policy_args.push(cmd.clone());
            }
        } else {
            debug!("Image Cmd field is not present");
        }

        debug!("Updated policy args: {:?}", policy_args);

        if let Some(working_dir) = &docker_config.WorkingDir {
            if !working_dir.is_empty() {
                process.cwd = working_dir.clone();
            }
        }

        debug!("get_process succeeded.");
        Ok(())
    }

    pub fn get_image_layers(&self) -> Vec<ImageLayer> {
        self.image_layers.clone()
    }
}

async fn get_image_layers(
    client: &mut Client,
    reference: &Reference,
    manifest: &manifest::OciImageManifest,
    config_layer: &DockerConfigLayer,
) -> Result<Vec<ImageLayer>> {
    let mut layer_index = 0;
    let mut layers = Vec::new();

    for layer in &manifest.layers {
        if layer
            .media_type
            .eq(manifest::IMAGE_DOCKER_LAYER_GZIP_MEDIA_TYPE)
        {
            if layer_index < config_layer.rootfs.diff_ids.len() {
                layers.push(ImageLayer {
                    diff_id: config_layer.rootfs.diff_ids[layer_index].clone(),
                    verity_hash: get_verity_hash(client, reference, &layer.digest).await?,
                });
            } else {
                return Err(anyhow!("Too many Docker gzip layers"));
            }

            layer_index += 1;
        }
    }

    Ok(layers)
}

async fn get_verity_hash(
    client: &mut Client,
    reference: &Reference,
    layer_digest: &str,
) -> Result<String> {
    let base_dir = std::path::Path::new("./layers_cache");
    std::fs::create_dir_all(&base_dir).unwrap();
    let mut compressed_path = base_dir.join(layer_digest);
    compressed_path.set_extension("gz");

    if compressed_path.exists() {
        info!("Using cached file {:?}", &compressed_path);
    } else {
        let mut file = tokio::fs::File::create(&compressed_path).await.unwrap();

        info!("Pulling layer {:?}", layer_digest);
        if let Err(err) = client.pull_blob(&reference, layer_digest, &mut file).await {
            drop(file);
            let _ = fs::remove_file(&compressed_path);
            panic!("Unable to pull blob {}, error {:?}", layer_digest, &err);
        } else {
            file.flush().await.unwrap();
        }
    }

    let mut decompressed_path = compressed_path.clone();
    decompressed_path.set_extension("");

    if decompressed_path.exists() {
        info!("Using cached file {:?}", &decompressed_path);
    } else {
        info!("Decompressing layer");

        if let Ok(compressed) = std::fs::File::open(&compressed_path) {
            if let Ok(mut decompressed) = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&decompressed_path) {

                let mut gz_decoder = flate2::read::GzDecoder::new(&compressed);
                if let Err(err) = std::io::copy(&mut gz_decoder, &mut decompressed) {
                    drop(compressed);
                    let _ = fs::remove_file(&compressed_path);
                    drop(decompressed);
                    let _ = fs::remove_file(&decompressed_path);
                    panic!("Unable to decompress file {:?}, error {:?}", &compressed_path, &err);
                }

                if let Err(err) = decompressed.seek(SeekFrom::Start(0)) {
                    drop(compressed);
                    let _ = fs::remove_file(&compressed_path);
                    drop(decompressed);
                    let _ = fs::remove_file(&decompressed_path);
                    panic!("Unable to rewind file {:?}, error {:?}", &decompressed_path, &err);
                }

                info!("Adding tarfs index to layer");
                if let Err(err) = tarindex::append_index(&mut decompressed) {
                    drop(compressed);
                    let _ = fs::remove_file(&compressed_path);
                    drop(decompressed);
                    let _ = fs::remove_file(&decompressed_path);
                    panic!("Unable to add tarfs index to file {:?}, error {:?}", &decompressed_path, &err);
                }

                if let Err(err) = decompressed.flush() {
                    drop(compressed);
                    let _ = fs::remove_file(&compressed_path);
                    drop(decompressed);
                    let _ = fs::remove_file(&decompressed_path);
                    panic!("Unable to flush file {:?}, error {:?}", &decompressed_path, &err);
                }
            } else {
                drop(compressed);
                let _ = fs::remove_file(&compressed_path);
                let _ = fs::remove_file(&decompressed_path);
                panic!("Unable to create file {:?}", &decompressed_path);
            }
        } else {
            let _ = fs::remove_file(&compressed_path);
            let _ = fs::remove_file(&decompressed_path);
            panic!("Unable to create file {:?}", &decompressed_path);
        }
    }

    info!("Calculating dm-verity root hash for layer");
    create_verity_hash(&decompressed_path.to_string_lossy())
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

pub async fn get_registry_containers(
    yaml_containers: &Vec<pod::Container>,
) -> Result<Vec<Container>> {
    let mut registry_containers = Vec::new();

    for yaml_container in yaml_containers {
        registry_containers.push(Container::new(&yaml_container.image).await?);
    }

    Ok(registry_containers)
}
