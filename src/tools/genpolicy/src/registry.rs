// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow Docker image config field names.
#![allow(non_snake_case)]

use crate::containerd;
use crate::policy;
use crate::utils::Config;
use crate::verity;

use anyhow::{anyhow, Result};
use docker_credential::{CredentialRetrievalError, DockerCredential};
use fs2::FileExt;
use log::{debug, info, warn, LevelFilter};
use oci_distribution::{
    client::{linux_amd64_resolver, ClientConfig, ClientProtocol},
    manifest,
    secrets::RegistryAuth,
    Client, Reference,
};
use serde::{Deserialize, Serialize};
use sha2::{digest::typenum::Unsigned, digest::OutputSizeUser, Sha256};
use std::{
    collections::BTreeMap, fs::OpenOptions, io, io::BufWriter, io::Read, io::Seek, io::Write,
    path::Path,
};
use tokio::io::AsyncWriteExt;

/// Container image properties obtained from an OCI repository.
#[derive(Clone, Debug, Default)]
pub struct Container {
    #[allow(dead_code)]
    pub image: String,
    pub config_layer: DockerConfigLayer,
    pub image_layers: Vec<ImageLayer>,
}

/// Image config layer properties.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DockerConfigLayer {
    architecture: String,
    pub config: DockerImageConfig,
    pub rootfs: DockerRootfs,
}

/// See: https://docs.docker.com/reference/dockerfile/.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DockerImageConfig {
    User: Option<String>,
    Tty: Option<bool>,
    Env: Option<Vec<String>>,
    Cmd: Option<Vec<String>>,
    WorkingDir: Option<String>,
    Entrypoint: Option<Vec<String>>,
    pub Volumes: Option<BTreeMap<String, DockerVolumeHostDirectory>>,
}

/// Container rootfs information.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DockerRootfs {
    r#type: String,
    pub diff_ids: Vec<String>,
}

/// This application's image layer properties.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageLayer {
    pub diff_id: String,
    pub verity_hash: String,
    pub passwd: String,
}

/// See https://docs.docker.com/reference/dockerfile/#volume.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DockerVolumeHostDirectory {
    // This struct is empty because, according to the documentation:
    // "The VOLUME instruction does not support specifying a host-dir
    // parameter. You must specify the mountpoint when you create or
    // run the container."
}

/// A single record in a Unix passwd file.
#[derive(Debug)]
struct PasswdRecord {
    pub user: String,
    #[allow(dead_code)]
    pub validate: bool,
    pub uid: u32,
    pub gid: u32,
    #[allow(dead_code)]
    pub gecos: String,
    #[allow(dead_code)]
    pub home: String,
    #[allow(dead_code)]
    pub shell: String,
}

/// Path to /etc/passwd in a container layer's tar file.
const PASSWD_FILE_TAR_PATH: &str = "etc/passwd";

/// Path to a file indicating a whiteout of the /etc/passwd file in a container
/// layer's tar file (i.e., /etc/passwd was deleted in the layer).
const PASSWD_FILE_WHITEOUT_TAR_PATH: &str = "etc/.wh.passwd";

/// A marker used to track whether a particular container layer has had its
/// /etc/passwd file deleted, and thus any such files read from previous, lower
/// layers should be discarded.
const WHITEOUT_MARKER: &str = "WHITEOUT";

impl Container {
    pub async fn new(config: &Config, image: &str) -> Result<Self> {
        info!("============================================");
        info!("Pulling manifest and config for {image}");
        let image_string = image.to_string();
        let reference: Reference = image_string.parse().unwrap();
        let auth = build_auth(&reference);

        let mut client = Client::new(ClientConfig {
            protocol: ClientProtocol::HttpsExcept(config.insecure_registries.clone()),
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
                debug!("config_layer: {:?}", &config_layer);

                let image_layers = get_image_layers(
                    config.layers_cache_file_path.clone(),
                    &mut client,
                    &reference,
                    &manifest,
                    &config_layer,
                )
                .await
                .unwrap();

                Ok(Container {
                    image: image_string,
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

        /*
         * The user field may:
         *
         * 1. Be empty
         * 2. Contain only a UID
         * 3. Contain a UID:GID pair, in that format
         * 4. Contain a user name, which we need to translate into a UID/GID pair
         * 5. Be erroneus, somehow
         */
        if let Some(image_user) = &docker_config.User {
            if !image_user.is_empty() {
                if image_user.contains(':') {
                    debug!("Splitting Docker config user = {:?}", image_user);
                    let user: Vec<&str> = image_user.split(':').collect();
                    let parts_count = user.len();
                    if parts_count != 2 {
                        warn!(
                            "Failed to split user, expected two parts, got {}, using uid = gid = 0",
                            parts_count
                        );
                    } else {
                        debug!("Parsing uid from user[0] = {}", &user[0]);
                        match user[0].parse() {
                            Ok(id) => process.User.UID = id,
                            Err(e) => {
                                warn!(
                                    "Failed to parse {} as u32, using uid = 0 - error {e}",
                                    &user[0]
                                );
                            }
                        }

                        debug!("Parsing gid from user[1] = {:?}", user[1]);
                        match user[1].parse() {
                            Ok(id) => process.User.GID = id,
                            Err(e) => {
                                warn!(
                                    "Failed to parse {} as u32, using gid = 0 - error {e}",
                                    &user[0]
                                );
                            }
                        }
                    }
                } else {
                    match image_user.parse::<u32>() {
                        Ok(uid) => process.User.UID = uid,
                        Err(outer_e) => {
                            // Find the last layer with an /etc/passwd file,
                            // respecting whiteouts.
                            let mut passwd = "".to_string();
                            for layer in self.get_image_layers() {
                                if !layer.passwd.is_empty() {
                                    passwd = layer.passwd
                                } else if layer.passwd == WHITEOUT_MARKER {
                                    passwd = "".to_string();
                                }
                            }

                            if passwd.is_empty() {
                                warn!("Failed to parse {} as u32 - error {outer_e} - and no /etc/passwd file is available, using uid = gid = 0", image_user);
                            } else {
                                match parse_passwd_file(passwd) {
                                    Ok(records) => {
                                        if let Some(record) =
                                            records.iter().find(|&r| r.user == *image_user)
                                        {
                                            process.User.UID = record.uid;
                                            process.User.GID = record.gid;
                                        }
                                    }
                                    Err(inner_e) => {
                                        warn!("Failed to parse {} as u32 - error {outer_e} - and failed to parse /etc/passwd - error {inner_e}, using uid = gid = 0", image_user);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(terminal) = docker_config.Tty {
            process.Terminal = terminal;
        } else {
            process.Terminal = false;
        }

        assert!(process.Env.is_empty());
        if let Some(config_env) = &docker_config.Env {
            for env in config_env {
                process.Env.push(env.clone());
            }
        } else {
            containerd::get_default_unix_env(&mut process.Env);
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
                process.Cwd.clone_from(working_dir);
            }
        }

        debug!("get_process succeeded.");
    }

    pub fn get_image_layers(&self) -> Vec<ImageLayer> {
        self.image_layers.clone()
    }
}

async fn get_image_layers(
    layers_cache_file_path: Option<String>,
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
            || layer.media_type.eq(manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE)
        {
            if layer_index < config_layer.rootfs.diff_ids.len() {
                let (verity_hash, passwd) = get_verity_and_users(
                    layers_cache_file_path.clone(),
                    client,
                    reference,
                    &layer.digest,
                    &config_layer.rootfs.diff_ids[layer_index].clone(),
                )
                .await?;
                layers.push(ImageLayer {
                    diff_id: config_layer.rootfs.diff_ids[layer_index].clone(),
                    verity_hash: verity_hash.to_owned(),
                    passwd: passwd.to_owned(),
                });
            } else {
                return Err(anyhow!("Too many Docker gzip layers"));
            }

            layer_index += 1;
        }
    }

    Ok(layers)
}

async fn get_verity_and_users(
    layers_cache_file_path: Option<String>,
    client: &mut Client,
    reference: &Reference,
    layer_digest: &str,
    diff_id: &str,
) -> Result<(String, String)> {
    let temp_dir = tempfile::tempdir_in(".")?;
    let base_dir = temp_dir.path();
    // Use file names supported by both Linux and Windows.
    let file_name = str::replace(layer_digest, ":", "-");
    let mut decompressed_path = base_dir.join(file_name);
    decompressed_path.set_extension("tar");

    let mut compressed_path = decompressed_path.clone();
    compressed_path.set_extension("gz");

    let mut verity_hash = "".to_string();
    let mut passwd = "".to_string();
    let mut error_message = "".to_string();
    let mut error = false;

    // get value from store and return if it exists
    if let Some(path) = layers_cache_file_path.as_ref() {
        let res = read_verity_and_users_from_store(path, diff_id)?;
        verity_hash = res.0;
        passwd = res.1;
        info!("Using cache file");
        info!("dm-verity root hash: {verity_hash}");
    }

    // create the layer files
    if verity_hash.is_empty() {
        if let Err(e) = create_decompressed_layer_file(
            client,
            reference,
            layer_digest,
            &decompressed_path,
            &compressed_path,
        )
        .await
        {
            error_message = format!("Failed to create verity hash for {layer_digest}, error {e}");
            error = true
        };

        if !error {
            match get_verity_hash_and_users(&decompressed_path) {
                Err(e) => {
                    error_message = format!("Failed to get verity hash {e}");
                    error = true;
                }
                Ok(res) => {
                    verity_hash = res.0;
                    passwd = res.1;
                    if let Some(path) = layers_cache_file_path.as_ref() {
                        add_verity_and_users_to_store(path, diff_id, &verity_hash, &passwd)?;
                    }
                    info!("dm-verity root hash: {verity_hash}");
                }
            }
        }
    }

    temp_dir.close()?;
    if error {
        // remove the cache file if we're using it
        if let Some(path) = layers_cache_file_path.as_ref() {
            std::fs::remove_file(path)?;
        }
        warn!("{error_message}");
    }
    Ok((verity_hash, passwd))
}

// the store is a json file that matches layer hashes to verity hashes
pub fn add_verity_and_users_to_store(
    cache_file: &str,
    diff_id: &str,
    verity_hash: &str,
    passwd: &str,
) -> Result<()> {
    // open the json file in read mode, create it if it doesn't exist
    let read_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(cache_file)?;

    let mut data: Vec<ImageLayer> = if let Ok(vec) = serde_json::from_reader(read_file) {
        vec
    } else {
        // Delete the malformed file here if it's present
        Vec::new()
    };

    // Add new data to the deserialized JSON
    data.push(ImageLayer {
        diff_id: diff_id.to_string(),
        verity_hash: verity_hash.to_string(),
        passwd: passwd.to_string(),
    });

    // Serialize in pretty format
    let serialized = serde_json::to_string_pretty(&data)?;

    // Open the JSON file to write
    let file = OpenOptions::new().write(true).open(cache_file)?;

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
pub fn read_verity_and_users_from_store(
    cache_file: &str,
    diff_id: &str,
) -> Result<(String, String)> {
    match OpenOptions::new().read(true).open(cache_file) {
        Ok(file) => match serde_json::from_reader(file) {
            Result::<Vec<ImageLayer>, _>::Ok(layers) => {
                for layer in layers {
                    if layer.diff_id == diff_id {
                        return Ok((layer.verity_hash, layer.passwd));
                    }
                }
            }
            Err(e) => {
                warn!("read_verity_and_users_from_store: failed to read cached image layers: {e}");
            }
        },
        Err(e) => {
            info!("read_verity_and_users_from_store: failed to open cache file: {e}");
        }
    }

    Ok((String::new(), String::new()))
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
        .pull_blob(reference, layer_digest, &mut file)
        .await
        .map_err(|e| anyhow!(e))?;
    file.flush().await.map_err(|e| anyhow!(e))?;

    info!("Decompressing layer");
    let compressed_file = std::fs::File::open(compressed_path).map_err(|e| anyhow!(e))?;
    let mut decompressed_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(decompressed_path)?;
    let mut gz_decoder = flate2::read::GzDecoder::new(compressed_file);
    std::io::copy(&mut gz_decoder, &mut decompressed_file).map_err(|e| anyhow!(e))?;

    info!("Adding tarfs index to layer");
    decompressed_file.seek(std::io::SeekFrom::Start(0))?;
    tarindex::append_index(&mut decompressed_file).map_err(|e| anyhow!(e))?;
    decompressed_file.flush().map_err(|e| anyhow!(e))?;

    Ok(())
}

pub fn get_verity_hash_and_users(path: &Path) -> Result<(String, String)> {
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

    file.seek(std::io::SeekFrom::Start(0))?;

    let mut passwd = String::new();
    for entry_wrap in tar::Archive::new(file).entries()? {
        let mut entry = entry_wrap?;
        let entry_path = entry.header().path()?;
        let path_str = entry_path.to_str().unwrap();
        if path_str == PASSWD_FILE_TAR_PATH {
            entry.read_to_string(&mut passwd)?;
            break;
        } else if path_str == PASSWD_FILE_WHITEOUT_TAR_PATH {
            passwd = WHITEOUT_MARKER.to_owned();
            break;
        }
    }

    Ok((result, passwd))
}

pub async fn get_container(config: &Config, image: &str) -> Result<Container> {
    if let Some(socket_path) = &config.containerd_socket_path {
        return Container::new_containerd_pull(
            config.layers_cache_file_path.clone(),
            image,
            socket_path,
        )
        .await;
    }
    Container::new(config, image).await
}

fn build_auth(reference: &Reference) -> RegistryAuth {
    debug!("build_auth: {:?}", reference);

    let server = reference
        .resolve_registry()
        .strip_suffix('/')
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

fn parse_passwd_file(passwd: String) -> Result<Vec<PasswdRecord>> {
    let mut records = Vec::new();

    for rec in passwd.lines() {
        let fields: Vec<&str> = rec.split(':').collect();

        let field_count = fields.len();
        if field_count != 7 {
            return Err(anyhow!(
                "Incorrect passwd record, expected 7 fields, got {}",
                field_count
            ));
        }

        records.push(PasswdRecord {
            user: fields[0].to_string(),
            validate: fields[1] == "x",
            uid: fields[2].parse().unwrap(),
            gid: fields[3].parse().unwrap(),
            gecos: fields[4].to_string(),
            home: fields[5].to_string(),
            shell: fields[6].to_string(),
        });
    }

    Ok(records)
}
