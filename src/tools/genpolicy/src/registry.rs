// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

// Allow Docker image config field names.
#![allow(non_snake_case)]

use crate::containerd;
use crate::layers_cache::ImageLayersCache;
use crate::policy;
use crate::utils::Config;

use anyhow::{anyhow, bail, Result};
use docker_credential::{CredentialRetrievalError, DockerCredential};
use log::{debug, info, warn, LevelFilter};
use oci_client::{
    client::{linux_amd64_resolver, ClientConfig, ClientProtocol},
    manifest,
    secrets::RegistryAuth,
    Client, Reference,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io, io::Read, io::Write, path::Path};
use tokio::io::AsyncWriteExt;

/// Container image properties obtained from an OCI repository.
#[derive(Clone, Debug, Default)]
pub struct Container {
    #[allow(dead_code)]
    pub image: String,
    pub config_layer: DockerConfigLayer,
    pub passwd: String,
    pub group: String,
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
    pub passwd: String,
    pub group: String,
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

/// A single record in a Unix group file.
#[derive(Debug)]
struct GroupRecord {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub validate: bool,
    pub gid: u32,
    pub user_list: Vec<String>,
}

/// Path to /etc/passwd in a container layer's tar file.
const PASSWD_FILE_TAR_PATH: &str = "etc/passwd";

/// Path to /etc/group in a container layer's tar file.
const GROUP_FILE_TAR_PATH: &str = "etc/group";

/// Path to a file indicating a whiteout of the /etc/passwd file in a container
/// layer's tar file (i.e., /etc/passwd was deleted in the layer).
const PASSWD_FILE_WHITEOUT_TAR_PATH: &str = "etc/.wh.passwd";

/// Path to a file indicating a whiteout of the /etc/group file in a container
/// layer's tar file (i.e., /etc/group was deleted in the layer).
const GROUP_FILE_WHITEOUT_TAR_PATH: &str = "etc/.wh.group";

/// A marker used to track whether a particular container layer has had its
/// /etc/* file deleted, and thus any such files read from previous, lower
/// layers should be discarded.
pub const WHITEOUT_MARKER: &str = "WHITEOUT";

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
                    &config.layers_cache,
                    &mut client,
                    &reference,
                    &manifest,
                    &config_layer,
                )
                .await
                .unwrap();

                // Find the last layer with an /etc/* file, respecting whiteouts.
                let mut passwd = String::new();
                let mut group = String::new();
                // Nydus/guest_pull doesn't make available passwd/group files from layers properly.
                // See issue https://github.com/kata-containers/kata-containers/issues/11162
                if !config.settings.cluster_config.guest_pull {
                    for layer in &image_layers {
                        if layer.passwd == WHITEOUT_MARKER {
                            passwd = String::new();
                        } else if !layer.passwd.is_empty() {
                            passwd = layer.passwd.clone();
                        }

                        if layer.group == WHITEOUT_MARKER {
                            group = String::new();
                        } else if !layer.group.is_empty() {
                            group = layer.group.clone();
                        }
                    }
                } else {
                    info!("Guest pull is enabled, skipping passwd/group file parsing");
                }

                Ok(Container {
                    image: image_string,
                    config_layer,
                    passwd,
                    group,
                })
            }
            Err(oci_client::errors::OciDistributionError::AuthenticationFailure(message)) => {
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

    pub fn get_gid_from_passwd_uid(&self, uid: u32) -> Result<u32> {
        if self.passwd.is_empty() {
            return Err(anyhow!(
                "No /etc/passwd file is available, unable to parse gids from uid"
            ));
        }
        match parse_passwd_file(&self.passwd) {
            Ok(records) => {
                if let Some(record) = records.iter().find(|&r| r.uid == uid) {
                    Ok(record.gid)
                } else {
                    Err(anyhow!("Failed to find uid {} in /etc/passwd", uid))
                }
            }
            Err(inner_e) => Err(anyhow!("Failed to parse /etc/passwd - error {inner_e}")),
        }
    }

    pub fn get_uid_gid_from_passwd_user(&self, user: String) -> Result<(u32, u32)> {
        if user.is_empty() {
            return Err(anyhow!("User is empty"));
        }

        if self.passwd.is_empty() {
            return Err(anyhow!(
                "No /etc/passwd file is available, unable to parse uid/gid from user"
            ));
        }

        match parse_passwd_file(&self.passwd) {
            Ok(records) => {
                if let Some(record) = records.iter().find(|&r| r.user == user) {
                    Ok((record.uid, record.gid))
                } else {
                    Err(anyhow!("Failed to find user {} in /etc/passwd", user))
                }
            }
            Err(inner_e) => Err(anyhow!("Failed to parse /etc/passwd - error {inner_e}.")),
        }
    }

    fn get_user_from_passwd_uid(&self, uid: u32) -> Result<String> {
        for record in parse_passwd_file(&self.passwd)? {
            if record.uid == uid {
                return Ok(record.user);
            }
        }
        Err(anyhow!("No user found with uid {uid}"))
    }

    pub fn get_additional_groups_from_uid(&self, uid: u32) -> Result<Vec<u32>> {
        if self.group.is_empty() || self.passwd.is_empty() {
            return Err(anyhow!(
                "No /etc/group, /etc/passwd file is available, unable to parse additional group membership from uid"
            ));
        }

        let user = self.get_user_from_passwd_uid(uid)?;

        match parse_group_file(&self.group) {
            Ok(records) => {
                let mut groups = Vec::new();
                for record in records.iter() {
                    record.user_list.iter().for_each(|u| {
                        if u == &user && &record.name != u {
                            // The second condition works around containerd bug
                            // https://github.com/containerd/containerd/issues/11937.
                            groups.push(record.gid);
                        }
                    });
                }
                Ok(groups)
            }
            Err(inner_e) => Err(anyhow!("Failed to parse /etc/group - error {inner_e}")),
        }
    }

    fn parse_user_string(&self, user: &str) -> u32 {
        if user.is_empty() {
            return 0;
        }

        match user.parse::<u32>() {
            Ok(uid) => uid,
            // If the user is not a number, interpret it as a user name.
            Err(outer_e) => {
                debug!(
                    "Failed to parse {} as u32, using it as a user name - error {outer_e}",
                    user
                );
                match self.get_uid_gid_from_passwd_user(user.to_string().clone()) {
                    Ok((uid, _)) => uid,
                    Err(err) => {
                        warn!(
                            "could not resolve named user {}, defaulting to uid 0: {}",
                            user, err
                        );
                        0
                    }
                }
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
         * 5. Contain a (user name:group name) pair, which we need to translate into a UID/GID pair
         * 6. Be erroneus, somehow
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
                        process.User.UID = self.parse_user_string(user[0]);

                        debug!(
                            "Overriding OCI container GID with UID:GID mapping from /etc/passwd"
                        );
                    }
                } else {
                    debug!("Parsing uid from image_user = {}", image_user);
                    process.User.UID = self.parse_user_string(image_user);

                    debug!("Using UID:GID mapping from /etc/passwd");
                }
                process.User.GID = self.get_gid_from_passwd_uid(process.User.UID).unwrap_or(0);
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
}

async fn get_image_layers(
    layers_cache: &ImageLayersCache,
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
                let mut imageLayer = get_users_from_layer(
                    layers_cache,
                    client,
                    reference,
                    &layer.digest,
                    &config_layer.rootfs.diff_ids[layer_index].clone(),
                )
                .await?;
                imageLayer.diff_id = config_layer.rootfs.diff_ids[layer_index].clone();
                layers.push(imageLayer);
            } else {
                return Err(anyhow!("Too many Docker gzip layers"));
            }

            layer_index += 1;
        }
    }

    Ok(layers)
}

async fn get_users_from_layer(
    layers_cache: &ImageLayersCache,
    client: &mut Client,
    reference: &Reference,
    layer_digest: &str,
    diff_id: &str,
) -> Result<ImageLayer> {
    if let Some(layer) = layers_cache.get_layer(diff_id) {
        info!("Using cache file");
        return Ok(layer);
    }

    let temp_dir = tempfile::tempdir_in(".")?;
    let base_dir = temp_dir.path();
    // Use file names supported by both Linux and Windows.
    let file_name = str::replace(layer_digest, ":", "-");
    let mut decompressed_path = base_dir.join(file_name);
    decompressed_path.set_extension("tar");

    let mut compressed_path = decompressed_path.clone();
    compressed_path.set_extension("gz");

    if let Err(e) = create_decompressed_layer_file(
        client,
        reference,
        layer_digest,
        &decompressed_path,
        &compressed_path,
    )
    .await
    {
        bail!(format!("Failed to decompress image layer, error {e}"));
    };

    match get_users_from_decompressed_layer(&decompressed_path) {
        Err(e) => {
            temp_dir.close()?;
            bail!(format!("Failed to get users from image layer, error {e}"));
        }
        Ok((passwd, group)) => {
            let layer = ImageLayer {
                diff_id: diff_id.to_string(),
                passwd,
                group,
            };
            layers_cache.insert_layer(&layer);
            Ok(layer)
        }
    }
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

    decompressed_file.flush().map_err(|e| anyhow!(e))?;
    Ok(())
}

pub fn get_users_from_decompressed_layer(path: &Path) -> Result<(String, String)> {
    let file = std::fs::File::open(path)?;
    let mut passwd = String::new();
    let mut group = String::new();
    let (mut found_passwd, mut found_group) = (false, false);
    for entry_wrap in tar::Archive::new(file).entries()? {
        let mut entry = entry_wrap?;
        let entry_path = entry.header().path()?;
        let path_str = entry_path.to_str().unwrap();
        if path_str == PASSWD_FILE_TAR_PATH {
            entry.read_to_string(&mut passwd)?;
            found_passwd = true;
            if found_passwd && found_group {
                break;
            }
        } else if path_str == PASSWD_FILE_WHITEOUT_TAR_PATH {
            passwd = WHITEOUT_MARKER.to_owned();
            found_passwd = true;
            if found_passwd && found_group {
                break;
            }
        } else if path_str == GROUP_FILE_TAR_PATH {
            entry.read_to_string(&mut group)?;
            found_group = true;
            if found_passwd && found_group {
                break;
            }
        } else if path_str == GROUP_FILE_WHITEOUT_TAR_PATH {
            group = WHITEOUT_MARKER.to_owned();
            found_group = true;
            if found_passwd && found_group {
                break;
            }
        }
    }

    Ok((passwd, group))
}

pub async fn get_container(config: &Config, image: &str) -> Result<Container> {
    if let Some(socket_path) = &config.containerd_socket_path {
        return Container::new_containerd_pull(config, image, socket_path).await;
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

fn parse_passwd_file(passwd: &str) -> Result<Vec<PasswdRecord>> {
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

fn parse_group_file(group: &str) -> Result<Vec<GroupRecord>> {
    let mut records = Vec::new();

    for rec in group.lines() {
        let fields: Vec<&str> = rec.split(':').collect();

        let field_count = fields.len();
        if field_count != 4 {
            return Err(anyhow!(
                "Incorrect group record, expected 3 fields, got {}",
                field_count
            ));
        }

        let mut user_list = vec![];
        if !fields[3].is_empty() {
            user_list = fields[3].split(',').map(|s| s.to_string()).collect();
        }

        records.push(GroupRecord {
            name: fields[0].to_string(),
            validate: fields[1] == "x",
            gid: fields[2].parse().unwrap(),
            user_list,
        });
    }

    Ok(records)
}
