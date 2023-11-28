// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow Docker image config field names.
#![allow(non_snake_case)]

use crate::policy;
use crate::verity;

use anyhow::{anyhow, Result};
use docker_credential::{CredentialRetrievalError, DockerCredential};
use log::warn;
use log::{debug, info, LevelFilter};
use oci_distribution::client::{linux_amd64_resolver, ClientConfig};
use oci_distribution::{manifest, secrets::RegistryAuth, Client, Reference};
use serde::{Deserialize, Serialize};
use sha2::{digest::typenum::Unsigned, digest::OutputSizeUser, Sha256};
use std::{io, io::Seek, io::Write, path::Path};
use tokio::{io::AsyncWriteExt};
use std::io::{BufWriter};
use std::fs::OpenOptions;
use fs2::FileExt;


/// Container image properties obtained from an OCI repository.
#[derive(Clone, Debug, Default)]
pub struct Container {
    config_layer: DockerConfigLayer,
    image_layers: Vec<ImageLayer>,
}

/// Image config layer properties.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DockerConfigLayer {
    architecture: String,
    config: DockerImageConfig,
    rootfs: DockerRootfs,
}

/// Image config properties.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DockerImageConfig {
    User: Option<String>,
    Tty: Option<bool>,
    Env: Vec<String>,
    Cmd: Option<Vec<String>>,
    WorkingDir: Option<String>,
    Entrypoint: Option<Vec<String>>,
}

/// Container rootfs information.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct DockerRootfs {
    r#type: String,
    diff_ids: Vec<String>,
}

/// This application's image layer properties.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageLayer {
    pub diff_id: String,
    pub verity_hash: String,
}

impl Container {
    pub async fn new(use_cached_files: bool, image: &str) -> Result<Self> {
        info!("============================================");
        info!("Pulling manifest and config for {:?}", image);
        let reference: Reference = image.to_string().parse().unwrap();
        let auth = build_auth(&reference);

        let mut client = Client::new(ClientConfig {
            platform_resolver: Some(Box::new(linux_amd64_resolver)),
            ..Default::default()
        });

        match client.pull_manifest_and_config(&reference, &auth).await {
            Ok((manifest, digest_hash, config_layer_str)) => {
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

                let config_layer: DockerConfigLayer =
                    serde_json::from_str(&config_layer_str).unwrap();
                let image_layers = get_image_layers(
                    use_cached_files,
                    &mut client,
                    &reference,
                    &manifest,
                    &config_layer,
                )
                .await
                .unwrap();

                Ok(Container {
                    config_layer,
                    image_layers,
                })
            }
            Err(oci_distribution::errors::OciDistributionError::AuthenticationFailure(message)) => {
                panic!("Container image registry authentication failure ({}). Are docker credentials set-up for current user?", &message);
            }
            Err(e) => {
                panic!(
                    "Failed to pull container image manifest and config - error: {:#?}",
                    &e
                );
            }
        }
    }

    // Convert Docker image config to policy data.
    pub fn get_process(
        &self,
        process: &mut policy::KataProcess,
        yaml_has_command: bool,
        yaml_has_args: bool,
    ) {
        debug!("Getting process field from docker config layer...");
        let docker_config = &self.config_layer.config;

        if let Some(image_user) = &docker_config.User {
            if !image_user.is_empty() {
                debug!("Splitting Docker config user = {:?}", image_user);
                let user: Vec<&str> = image_user.split(':').collect();
                if !user.is_empty() {
                    debug!("Parsing uid from user[0] = {}", &user[0]);
                    match user[0].parse() {
                        Ok(id) => process.User.UID = id,
                        Err(e) => {
                            // "image: prom/prometheus" has user = "nobody", but
                            // process.User.UID is an u32 value.
                            warn!(
                                "Failed to parse {} as u32, using uid = 0 - error {e}",
                                &user[0]
                            );
                            process.User.UID = 0;
                        }
                    }
                }
                if user.len() > 1 {
                    debug!("Parsing gid from user[1] = {:?}", user[1]);
                    process.User.GID = user[1].parse().unwrap();
                }
            }
        }

        if let Some(terminal) = docker_config.Tty {
            process.Terminal = terminal;
        } else {
            process.Terminal = false;
        }

        for env in &docker_config.Env {
            process.Env.push(env.clone());
        }

        let policy_args = &mut process.Args;
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
                process.Cwd = working_dir.clone();
            }
        }

        debug!("get_process succeeded.");
    }

    pub fn get_image_layers(&self) -> Vec<ImageLayer> {
        self.image_layers.clone()
    }
}

async fn get_image_layers(
    use_cached_files: bool,
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
                    verity_hash: get_verity_hash(
                        use_cached_files,
                        client,
                        reference,
                        &layer.digest,
                        &config_layer.rootfs.diff_ids[layer_index].clone(),
                    )
                    .await?,
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
    use_cached_files: bool,
    client: &mut Client,
    reference: &Reference,
    layer_digest: &str,
    diff_id: &str,
) -> Result<String> {
    let temp_dir = tempfile::tempdir_in(".")?;
    let base_dir = temp_dir.path();
    let cache_file = "layers-cache.json";
    // Use file names supported by both Linux and Windows.
    let file_name = str::replace(&layer_digest, ":", "-");
    let mut decompressed_path = base_dir.join(file_name);
    decompressed_path.set_extension("tar");

    let mut compressed_path = decompressed_path.clone();
    compressed_path.set_extension("gz");

    let mut verity_hash = "".to_string();
    let mut error_message = "".to_string();
    let mut error = false;

    // get value from store and return if it exists
    if use_cached_files {
        verity_hash = read_verity_from_store(&cache_file, &diff_id)?;
        info!("Using cache file");
        info!("dm-verity root hash: {verity_hash}");
    }

    // create the layer files
    if verity_hash == "" {
        if let Err(e) = create_decompressed_layer_file(
            client,
            reference,
            layer_digest,
            &decompressed_path,
            &compressed_path,
        )
        .await
        {
            error_message = format!(
                "Failed to create verity hash for {layer_digest}, error {e}"
            );
            error = true
        };

        if !error {
            match get_verity_hash_value(&decompressed_path) {
                Err(e) => {
                    error_message = format!("Failed to get verity hash {e}");
                    error = true;
                }
                Ok(v) => {
                    verity_hash = v;
                    if use_cached_files {
                        add_verity_to_store(&cache_file, &diff_id, &verity_hash)?;
                    }
                    info!("dm-verity root hash: {verity_hash}");
                }
            }
        }
    }

    temp_dir.close()?;
    if error {
        // remove the cache file if we're using it
        if use_cached_files {
            std::fs::remove_file(&cache_file)?;
        }
        warn!("{error_message}");
    }
    Ok(verity_hash)
}

// the store is a json file that matches layer hashes to verity hashes
fn add_verity_to_store(
    cache_file: &str,
    diff_id: &str,
    verity_hash: &str,
) -> Result<()> {
    // open the json file in read mode, create it if it doesn't exist
    let read_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(cache_file)?;

    let mut data: Vec<ImageLayer> = if let Ok(vec) = serde_json::from_reader(read_file) {
        vec
    } else {
        // Delete the malformed file here if it's present
        Vec::new()
    };

    // Add new data to the deserialized JSON
    data.push(ImageLayer{
        diff_id: diff_id.to_string(),
        verity_hash: verity_hash.to_string(),
    });

    // Serialize in pretty format
    let serialized = serde_json::to_string_pretty(&data)?;

    // Open the JSON file to write
    let file = OpenOptions::new()
        .write(true)
        .open(cache_file)?;

    // try to lock the file, if it fails, get the error
    let result = file.try_lock_exclusive();
    if result.is_err() {
        warn!("Waiting to lock file: {cache_file}");
        file.lock_exclusive()?;
    }
    // Write the serialized JSON to the file
    let mut writer = BufWriter::new(&file);
    writeln!(writer, "{}", serialized)?;
    writer.flush()?;
    file.unlock()?;
    Ok(())
}

// helper function to read the verity hash from the store
// returns empty string if not found or file does not exist
fn read_verity_from_store(cache_file: &str, diff_id: &str) -> Result<String> {
    // see if file exists, return empty if not
    if !Path::new(cache_file).exists() {
        return Ok("".to_string());
    }

    let file = OpenOptions::new()
        .read(true)
        .open(cache_file)?;

    // If the file is empty, return empty string
    if file.metadata()?.len() == 0 {
        return Ok("".to_string());
    }

    let data: Vec<ImageLayer> = serde_json::from_reader(file)?;
    for layer in data {
        if layer.diff_id == diff_id {
            return Ok(layer.verity_hash);
        }
    }
    Ok("".to_string())
}

async fn create_decompressed_layer_file(
    client: &mut Client,
    reference: &Reference,
    layer_digest: &str,
    decompressed_path: &Path,
    compressed_path: &Path,
) -> Result<()> {
    info!("Pulling layer {:?}", layer_digest);
    let mut file = tokio::fs::File::create(&compressed_path)
        .await
        .map_err(|e| anyhow!(e))?;
    client
        .pull_blob(&reference, layer_digest, &mut file)
        .await
        .map_err(|e| anyhow!(e))?;
    file.flush().await.map_err(|e| anyhow!(e))?;

    info!("Decompressing layer");
    let compressed_file = std::fs::File::open(&compressed_path).map_err(|e| anyhow!(e))?;
    let mut decompressed_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&decompressed_path)?;
    let mut gz_decoder = flate2::read::GzDecoder::new(compressed_file);
    std::io::copy(&mut gz_decoder, &mut decompressed_file).map_err(|e| anyhow!(e))?;

    info!("Adding tarfs index to layer");
    decompressed_file.seek(std::io::SeekFrom::Start(0))?;
    tarindex::append_index(&mut decompressed_file).map_err(|e| anyhow!(e))?;
    decompressed_file.flush().map_err(|e| anyhow!(e))?;

    Ok(())
}

fn get_verity_hash_value(path: &Path) -> Result<String> {
    info!("Calculating dm-verity root hash");
    let mut file = std::fs::File::open(path)?;
    let size = file.seek(std::io::SeekFrom::End(0))?;
    if size < 4096 {
        return Err(anyhow!("Block device {:?} is too small: {size}", &path));
    }

    let salt = [0u8; <Sha256 as OutputSizeUser>::OutputSize::USIZE];
    let v = verity::Verity::<Sha256>::new(size, 4096, 4096, &salt, 0)?;
    let hash = verity::traverse_file(&mut file, 0, false, v, &mut verity::no_write)?;
    let result = format!("{:x}", hash);

    Ok(result)
}

pub async fn get_container(use_cache: bool, image: &str) -> Result<Container> {
    Container::new(use_cache, image).await
}

fn build_auth(reference: &Reference) -> RegistryAuth {
    debug!("build_auth: {:?}", reference);

    let server = reference
        .resolve_registry()
        .strip_suffix("/")
        .unwrap_or_else(|| reference.resolve_registry());

    match docker_credential::get_credential(server) {
        Ok(DockerCredential::UsernamePassword(username, password)) => {
            debug!("build_auth: Found docker credentials");
            return RegistryAuth::Basic(username, password);
        }
        Ok(DockerCredential::IdentityToken(_)) => {
            warn!("build_auth: Cannot use contents of docker config, identity token not supported. Using anonymous access.");
        }
        Err(CredentialRetrievalError::ConfigNotFound) => {
            debug!("build_auth: Docker config not found - using anonymous access.");
        }
        Err(CredentialRetrievalError::NoCredentialConfigured) => {
            debug!("build_auth: Docker credentials not configured - using anonymous access.");
        }
        Err(CredentialRetrievalError::ConfigReadError) => {
            debug!("build_auth: Cannot read docker credentials - using anonymous access.");
        }
        Err(CredentialRetrievalError::HelperFailure { stdout, stderr }) => {
            if stdout == "credentials not found in native keychain\n" {
                // On WSL, this error is generated when credentials are not
                // available in ~/.docker/config.json.
                debug!("build_auth: Docker credentials not found - using anonymous access.");
            } else {
                warn!("build_auth: Docker credentials not found - using anonymous access. stderr = {}, stdout = {}",
                    &stderr, &stdout);
            }
        }
        Err(e) => panic!("Error handling docker configuration file: {}", e),
    }

    RegistryAuth::Anonymous
}
