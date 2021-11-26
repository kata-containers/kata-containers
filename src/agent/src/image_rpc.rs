// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, ensure, Result};
use async_trait::async_trait;
use protocols::image;
use std::convert::TryFrom;
use std::fs::File;
use tokio::sync::Mutex;
use ttrpc::{self, error::get_rpc_status as ttrpc_error};

use crate::rpc::{verify_cid, CONTAINER_BASE};
use crate::sandbox::Sandbox;
use crate::AGENT_CONFIG;

use oci_distribution::client::{ImageData, ImageLayer};
use oci_distribution::manifest::{OciDescriptor, OciManifest};
use oci_distribution::{manifest, secrets::RegistryAuth, Client, Reference};
use ocicrypt_rs::config::CryptoConfig;
use ocicrypt_rs::encryption::decrypt_layer;
use ocicrypt_rs::helpers::create_decrypt_config;
use ocicrypt_rs::spec::{
    MEDIA_TYPE_LAYER_ENC, MEDIA_TYPE_LAYER_GZIP_ENC, MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_ENC,
    MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_GZIP_ENC,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};

const SKOPEO_PATH: &str = "/usr/bin/skopeo";
const UMOCI_PATH: &str = "/usr/local/bin/umoci";
const IMAGE_OCI: &str = "image_oci";
const AA_PATH: &str = "/usr/local/bin/attestation-agent";
const AA_PORT: &str = "127.0.0.1:50000";
const OCICRYPT_CONFIG_PATH: &str = "/tmp/ocicrypt_config.json";
const OCI_ANNOTATION_REF_NAME: &str = "org.opencontainers.image.ref.name";
const OCI_IMAGE_MANIFEST_NAME: &str = "application/vnd.oci.image.manifest.v1+json";
const OCI_LAYOUT: &str = r#"{"imageLayoutVersion": "1.0.0"}"#;
const IMAGE_DOCKER_LAYER_FOREIGN_GZIP_MEDIA_TYPE: &str =
    "application/vnd.docker.image.rootfs.foreign.diff.tar.gzip";
const DIGEST_SHA256: &str = "sha256";

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

#[derive(Serialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IndexDescriptor {
    pub schema_version: u8,
    pub manifests: Vec<OciDescriptor>,
}

pub struct ImageService {
    sandbox: Arc<Mutex<Sandbox>>,
    attestation_agent_started: AtomicBool,
}

impl ImageService {
    pub fn new(sandbox: Arc<Mutex<Sandbox>>) -> Self {
        Self {
            sandbox,
            attestation_agent_started: AtomicBool::new(false),
        }
    }

    fn build_oci_path(cid: &str) -> PathBuf {
        let mut oci_path = PathBuf::from("/tmp");
        oci_path.push(cid);
        oci_path.push(IMAGE_OCI);
        oci_path
    }

    fn decrypt_layer_data(
        layer: &ImageLayer,
        layer_digest: &str,
        image_manifest: &mut OciManifest,
        crypto_config: &CryptoConfig,
        oci_blob_path: &Path,
    ) -> Result<()> {
        if let Some(decrypt_config) = &crypto_config.decrypt_config {
            for layer_desc in image_manifest.layers.iter_mut() {
                if layer_desc.digest.as_str() == layer_digest {
                    let (layer_decryptor, _dec_digest) =
                        decrypt_layer(decrypt_config, layer.data.as_slice(), layer_desc, false)?;
                    let mut plaintxt_data: Vec<u8> = Vec::new();
                    let mut decryptor =
                        layer_decryptor.ok_or_else(|| anyhow!("Missing layer decryptor"))?;

                    decryptor.read_to_end(&mut plaintxt_data)?;
                    let layer_name = format!("{:x}", Sha256::digest(&plaintxt_data));
                    let mut out_file = File::create(oci_blob_path.join(&layer_name))?;
                    out_file.write_all(&plaintxt_data)?;
                    layer_desc.media_type = manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE.to_string();

                    layer_desc.digest = format!("{}:{}", DIGEST_SHA256, layer_name);
                }
            }
        } else {
            return Err(anyhow!("No decrypt config available"));
        }

        Ok(())
    }

    fn handle_layer_data(
        image_data: &ImageData,
        image_manifest: &mut OciManifest,
        crypto_config: &CryptoConfig,
        oci_blob_path: &Path,
    ) -> Result<()> {
        for layer in image_data.layers.iter() {
            let layer_digest = layer.clone().sha256_digest();

            if layer.media_type == MEDIA_TYPE_LAYER_GZIP_ENC
                || layer.media_type == MEDIA_TYPE_LAYER_ENC
            {
                Self::decrypt_layer_data(
                    layer,
                    &layer_digest,
                    image_manifest,
                    crypto_config,
                    oci_blob_path,
                )?;
            } else if let Some(layer_name) =
                layer_digest.strip_prefix(format!("{}:", DIGEST_SHA256).as_str())
            {
                let mut out_file = File::create(oci_blob_path.join(&layer_name))?;
                out_file.write_all(&layer.data)?;
            } else {
                error!(
                    sl!(),
                    "layer digest algo not supported:: {:?}", layer_digest
                );
            }
        }

        Ok(())
    }

    #[tokio::main]
    async fn download_image(
        image: &str,
        auth: &RegistryAuth,
    ) -> anyhow::Result<(OciManifest, String, ImageData)> {
        let reference = Reference::try_from(image)?;
        let mut client = Client::default();
        let (image_manifest, _image_digest, image_config) =
            client.pull_manifest_and_config(&reference, auth).await?;

        // TODO: Get the value from config
        let max_attempt = 2;
        let attempt_interval = 1;
        for i in 1..max_attempt {
            match client
                .pull(
                    &reference,
                    auth,
                    vec![
                        manifest::IMAGE_DOCKER_LAYER_GZIP_MEDIA_TYPE,
                        manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE,
                        MEDIA_TYPE_LAYER_GZIP_ENC,
                        MEDIA_TYPE_LAYER_ENC,
                        MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_ENC,
                        MEDIA_TYPE_LAYER_NON_DISTRIBUTABLE_GZIP_ENC,
                    ],
                )
                .await
            {
                Ok(data) => return Ok((image_manifest, image_config, data)),
                Err(e) => {
                    info!(
                        sl!(),
                        "Got error on pull call attempt #{}. Will retry in {}s: {:?}",
                        attempt_interval,
                        i,
                        e
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(attempt_interval)).await;
                }
            }
        }

        Err(anyhow!("Failed to download image data"))
    }

    fn pull_image_with_oci_distribution(
        image: &str,
        cid: &str,
        source_creds: &Option<String>,
        aa_kbc_params: &str,
    ) -> Result<()> {
        let oci_path = Self::build_oci_path(cid);
        fs::create_dir_all(&oci_path)?;

        let mut auth = RegistryAuth::Anonymous;
        if let Some(source_creds) = source_creds {
            if let Some((username, password)) = source_creds.split_once(':') {
                auth = RegistryAuth::Basic(username.to_string(), password.to_string());
            } else {
                return Err(anyhow!("Invalid authentication info ({:?})", source_creds));
            }
        }

        let (mut image_manifest, image_config, image_data) = Self::download_image(image, &auth)?;

        // Prepare OCI layout storage for umoci
        image_manifest.config.media_type = manifest::IMAGE_CONFIG_MEDIA_TYPE.to_string();
        // TODO: support other digest algo like sha512
        let oci_blob_path = oci_path.join(format!("blobs/{}", DIGEST_SHA256));
        fs::create_dir_all(&oci_blob_path)?;

        if let Some(config_name) = &image_manifest
            .config
            .digest
            .strip_prefix(format!("{}:", DIGEST_SHA256).as_str())
        {
            let mut out_file = File::create(oci_blob_path.join(config_name))?;
            out_file.write_all(image_config.as_bytes())?;
        }

        let mut cc = CryptoConfig::default();

        if !aa_kbc_params.is_empty() {
            let decrypt_config = format!("provider:attestation-agent:{}", aa_kbc_params);
            cc = create_decrypt_config(vec![decrypt_config], vec![])?;
        }

        // Covert docker layer media type to OCI type
        for layer_desc in image_manifest.layers.iter_mut() {
            if layer_desc.media_type == manifest::IMAGE_DOCKER_LAYER_GZIP_MEDIA_TYPE
                || layer_desc.media_type == IMAGE_DOCKER_LAYER_FOREIGN_GZIP_MEDIA_TYPE
            {
                layer_desc.media_type = manifest::IMAGE_LAYER_GZIP_MEDIA_TYPE.to_string();
            }
        }

        Self::handle_layer_data(&image_data, &mut image_manifest, &cc, &oci_blob_path)?;

        let manifest_json = serde_json::to_string(&image_manifest)?;
        let manifest_digest = format!("{:x}", Sha256::digest(manifest_json.as_bytes()));

        let mut out_file = File::create(oci_blob_path.join(manifest_digest))?;
        out_file.write_all(manifest_json.as_bytes())?;

        let mut annotations = HashMap::new();
        annotations.insert(OCI_ANNOTATION_REF_NAME.to_string(), "latest".to_string());

        let manifest_descriptor = OciDescriptor {
            media_type: OCI_IMAGE_MANIFEST_NAME.to_string(),
            digest: format!(
                "{}:{:x}",
                DIGEST_SHA256,
                Sha256::digest(manifest_json.as_bytes())
            ),
            size: manifest_json.len() as i64,
            annotations: Some(annotations),
            ..Default::default()
        };

        let index_descriptor = IndexDescriptor {
            schema_version: image_manifest.schema_version,
            manifests: vec![manifest_descriptor],
        };

        let mut out_file = File::create(format!("{}/index.json", oci_path.to_string_lossy()))?;
        out_file.write_all(serde_json::to_string(&index_descriptor)?.as_bytes())?;

        let mut out_file = File::create(format!("{}/oci-layout", oci_path.to_string_lossy()))?;
        out_file.write_all(OCI_LAYOUT.as_bytes())?;

        Ok(())
    }

    fn pull_image_from_registry(
        image: &str,
        cid: &str,
        source_creds: &Option<&str>,
        policy_path: &Option<&String>,
        aa_kbc_params: &str,
    ) -> Result<()> {
        let source_image = format!("{}{}", "docker://", image);

        let tmp_cid_path = Path::new("/tmp/").join(cid);
        let oci_path = tmp_cid_path.join(IMAGE_OCI);
        let target_path_oci = format!("oci://{}:latest", oci_path.to_string_lossy());

        fs::create_dir_all(&oci_path)?;

        let mut pull_command = Command::new(SKOPEO_PATH);
        pull_command
            .arg("copy")
            .arg(source_image)
            .arg(&target_path_oci)
            .arg("--remove-signatures"); //umoci requires signatures to be removed

        // If source credentials were passed (so not using an anonymous registry), pass them through
        if let Some(source_creds) = source_creds {
            pull_command.arg("--src-creds").arg(source_creds);
        }

        // If a policy_path provided, use it, otherwise fall back to allow all image registries
        if let Some(policy_path) = policy_path {
            pull_command.arg("--policy").arg(policy_path);
        } else {
            info!(
                sl!(),
                "No policy path was supplied, so revert to allow all images to be pulled."
            );
            pull_command.arg("--insecure-policy");
        }

        debug!(sl!(), "skopeo command: {:?}", &pull_command);
        if !aa_kbc_params.is_empty() {
            // Skopeo will copy an unencrypted image even if the decryption key argument is provided.
            // Thus, this does not guarantee that the image was encrypted.
            pull_command
                .arg("--decryption-key")
                .arg(format!("provider:attestation-agent:{}", aa_kbc_params))
                .env("OCICRYPT_KEYPROVIDER_CONFIG", OCICRYPT_CONFIG_PATH);
        }

        let status: ExitStatus = pull_command.status()?;

        if !status.success() {
            let mut error_message = format!("failed to pull image: {:?}", status);

            if let Err(e) = fs::remove_dir_all(&tmp_cid_path) {
                error_message.push_str(&format!(
                    " and clean up of temporary container directory {:?} failed with error {:?}",
                    tmp_cid_path, e
                ));
            };
            return Err(anyhow!(error_message));
        }
        Ok(())
    }

    fn unpack_image(cid: &str) -> Result<()> {
        let tmp_cid_path = Path::new("/tmp/").join(cid);
        let source_path_oci = tmp_cid_path.join(IMAGE_OCI);

        let target_path_bundle = Path::new(CONTAINER_BASE).join(cid);

        info!(sl!(), "unpack image {:?} to {:?}", cid, target_path_bundle);

        // Unpack image
        let status: ExitStatus = Command::new(UMOCI_PATH)
            .arg("unpack")
            .arg("--image")
            .arg(&source_path_oci)
            .arg(&target_path_bundle)
            .status()?;

        ensure!(status.success(), "failed to unpack image: {:?}", status);

        // To save space delete the oci image after unpack
        fs::remove_dir_all(&tmp_cid_path)?;

        Ok(())
    }

    // If we fail to start the AA, Skopeo/ocicrypt won't be able to unwrap keys
    // and container decryption will fail.
    fn init_attestation_agent() {
        let config_path = OCICRYPT_CONFIG_PATH;

        // The image will need to be encrypted using a keyprovider
        // that has the same name (at least according to the config).
        let ocicrypt_config = serde_json::json!({
            "key-providers": {
                "attestation-agent":{
                    "grpc":AA_PORT
                }
            }
        });

        let mut config_file = fs::File::create(config_path).unwrap();
        config_file
            .write_all(ocicrypt_config.to_string().as_bytes())
            .unwrap();

        // The Attestation Agent will run for the duration of the guest.
        Command::new(AA_PATH)
            .arg("--grpc_sock")
            .arg(AA_PORT)
            .spawn()
            .unwrap();
    }

    async fn pull_image(&self, req: &image::PullImageRequest) -> Result<String> {
        env::set_var("OCICRYPT_KEYPROVIDER_CONFIG", OCICRYPT_CONFIG_PATH);

        let image = req.get_image();
        let mut cid = req.get_container_id().to_string();

        let aa_kbc_params = &AGENT_CONFIG.read().await.aa_kbc_params;

        if cid.is_empty() {
            let v: Vec<&str> = image.rsplit('/').collect();
            if !v[0].is_empty() {
                // ':' have special meaning for umoci during upack
                cid = v[0].replace(":", "_");
            } else {
                return Err(anyhow!("Invalid image name. {}", image));
            }
        } else {
            verify_cid(&cid)?;
        }

        if !aa_kbc_params.is_empty() {
            match self.attestation_agent_started.compare_exchange_weak(
                false,
                true,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => Self::init_attestation_agent(),
                Err(_) => info!(sl!(), "Attestation Agent already running"),
            }
        }

        let source_creds = (!req.get_source_creds().is_empty()).then(|| req.get_source_creds());

        if Path::new(SKOPEO_PATH).exists() {
            // Read the policy path from the agent config
            let config_policy_path = &AGENT_CONFIG.read().await.container_policy_path;
            let policy_path = (!config_policy_path.is_empty()).then(|| config_policy_path);

            Self::pull_image_from_registry(
                image,
                &cid,
                &source_creds,
                &policy_path,
                aa_kbc_params,
            )?;
        } else {
            let image = image.to_string();
            let cid = cid.to_string();
            let source_creds =
                (!req.get_source_creds().is_empty()).then(|| req.get_source_creds().to_string());
            let aa_kbc_params = aa_kbc_params.to_string();

            // ocicrypt-rs keyprovider module will create a new runtime to talk with
            // attestation agent, to avoid startup a runtime within a runtime, we
            // spawn a new thread here.
            tokio::task::spawn_blocking(move || {
                Self::pull_image_with_oci_distribution(&image, &cid, &source_creds, &aa_kbc_params)
                    .map_err(|err| warn!(sl!(), "pull image failed: {:?}", err))
                    .ok();
            })
            .await?;
        }

        Self::unpack_image(&cid)?;

        let mut sandbox = self.sandbox.lock().await;
        sandbox.images.insert(String::from(image), cid.to_string());
        Ok(image.to_owned())
    }
}

#[async_trait]
impl protocols::image_ttrpc::Image for ImageService {
    async fn pull_image(
        &self,
        _ctx: &ttrpc::r#async::TtrpcContext,
        req: image::PullImageRequest,
    ) -> ttrpc::Result<image::PullImageResponse> {
        match self.pull_image(&req).await {
            Ok(r) => {
                let mut resp = image::PullImageResponse::new();
                resp.image_ref = r;
                return Ok(resp);
            }
            Err(e) => {
                return Err(ttrpc_error(ttrpc::Code::INTERNAL, e.to_string()));
            }
        }
    }
}
