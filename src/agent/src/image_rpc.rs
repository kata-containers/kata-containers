// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, ensure, Result};
use async_trait::async_trait;
use protocols::image;
use tokio::sync::Mutex;
use ttrpc::{self, error::get_rpc_status as ttrpc_error};

use crate::rpc::{verify_cid, CONTAINER_BASE};
use crate::sandbox::Sandbox;
use crate::AGENT_CONFIG;

use image_rs::image::ImageClient;
use std::io::Write;

const SKOPEO_PATH: &str = "/usr/bin/skopeo";
const UMOCI_PATH: &str = "/usr/local/bin/umoci";
const IMAGE_OCI: &str = "image_oci";
const AA_PATH: &str = "/usr/local/bin/attestation-agent";
const AA_KEYPROVIDER_PORT: &str = "127.0.0.1:50000";
const AA_GETRESOURCE_PORT: &str = "127.0.0.1:50001";
const OCICRYPT_CONFIG_PATH: &str = "/tmp/ocicrypt_config.json";
// kata rootfs is readonly, use tmpfs before CC storage is implemented.
const KATA_CC_IMAGE_WORK_DIR: &str = "/run/image/";
const KATA_CC_PAUSE_BUNDLE: &str = "/pause_bundle";
const CONFIG_JSON: &str = "config.json";
const OFFLINE_FS_KBC_RESOURCE_PATH: &str = "/etc/aa-offline_fs_kbc-resources.json";

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

pub struct ImageService {
    sandbox: Arc<Mutex<Sandbox>>,
    attestation_agent_started: AtomicBool,
    image_client: Arc<Mutex<ImageClient>>,
}

impl ImageService {
    pub fn new(sandbox: Arc<Mutex<Sandbox>>) -> Self {
        env::set_var("CC_IMAGE_WORK_DIR", KATA_CC_IMAGE_WORK_DIR);
        Self {
            sandbox,
            attestation_agent_started: AtomicBool::new(false),
            image_client: Arc::new(Mutex::new(ImageClient::default())),
        }
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
                let _ = write!(
                    error_message,
                    " and clean up of temporary container directory {:?} failed with error {:?}",
                    tmp_cid_path, e
                );
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

    // pause image is packaged in rootfs for CC
    fn unpack_pause_image(cid: &str) -> Result<()> {
        let cc_pause_bundle = Path::new(KATA_CC_PAUSE_BUNDLE);
        if !cc_pause_bundle.exists() {
            return Err(anyhow!("Pause image not present in rootfs"));
        }

        info!(sl!(), "use guest pause image cid {:?}", cid);
        let pause_bundle = Path::new(CONTAINER_BASE).join(&cid);
        let pause_rootfs = pause_bundle.join("rootfs");
        let pause_config = pause_bundle.join(CONFIG_JSON);
        let pause_binary = pause_rootfs.join("pause");
        fs::create_dir_all(&pause_rootfs)?;
        if !pause_config.exists() {
            fs::copy(
                cc_pause_bundle.join(CONFIG_JSON),
                pause_bundle.join(CONFIG_JSON),
            )?;
        }
        if !pause_binary.exists() {
            fs::copy(cc_pause_bundle.join("rootfs").join("pause"), pause_binary)?;
        }

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
                    "grpc":AA_KEYPROVIDER_PORT
                }
            }
        });

        let mut config_file = fs::File::create(config_path).unwrap();
        config_file
            .write_all(ocicrypt_config.to_string().as_bytes())
            .unwrap();

        // The Attestation Agent will run for the duration of the guest.
        Command::new(AA_PATH)
            .arg("--keyprovider_sock")
            .arg(AA_KEYPROVIDER_PORT)
            .arg("--getresource_sock")
            .arg(AA_GETRESOURCE_PORT)
            .spawn()
            .unwrap();
    }

    async fn pull_image(&self, req: &image::PullImageRequest) -> Result<String> {
        env::set_var("OCICRYPT_KEYPROVIDER_CONFIG", OCICRYPT_CONFIG_PATH);

        let https_proxy = &AGENT_CONFIG.read().await.https_proxy;
        if !https_proxy.is_empty() {
            env::set_var("HTTPS_PROXY", https_proxy);
        }

        let no_proxy = &AGENT_CONFIG.read().await.no_proxy;
        if !no_proxy.is_empty() {
            env::set_var("NO_PROXY", no_proxy);
        }

        let image = req.get_image();
        let mut cid = req.get_container_id().to_string();

        let aa_kbc_params = &AGENT_CONFIG.read().await.aa_kbc_params;

        if cid.is_empty() {
            let v: Vec<&str> = image.rsplit('/').collect();
            if !v[0].is_empty() {
                // ':' have special meaning for umoci during upack
                cid = v[0].replace(':', "_");
            } else {
                return Err(anyhow!("Invalid image name. {}", image));
            }
        } else {
            verify_cid(&cid)?;
        }

        // Can switch to use cid directly when we remove umoci
        let v: Vec<&str> = image.rsplit('/').collect();
        if !v[0].is_empty() && v[0].starts_with("pause:") {
            Self::unpack_pause_image(&cid)?;

            let mut sandbox = self.sandbox.lock().await;
            sandbox.images.insert(String::from(image), cid.to_string());
            return Ok(image.to_owned());
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

            Self::unpack_image(&cid)?;
        } else {
            // TODO #4888 - Create a better way to enable signature verification. This is temporary for the PoC
            if aa_kbc_params.eq("offline_fs_kbc::null")
                && Path::new(OFFLINE_FS_KBC_RESOURCE_PATH).exists()
            {
                info!(sl!(), "Enabling security_validate on image_client");
                self.image_client.lock().await.config.security_validate = true;
            }

            let bundle_path = Path::new(CONTAINER_BASE).join(&cid);
            fs::create_dir_all(&bundle_path)?;

            let decrypt_config = format!("provider:attestation-agent:{}", aa_kbc_params);

            info!(sl!(), "pull image {:?}, bundle path {:?}", cid, bundle_path);
            // Image layers will store at KATA_CC_IMAGE_WORK_DIR, generated bundles
            // with rootfs and config.json will store under CONTAINER_BASE/cid.
            self.image_client
                .lock()
                .await
                .pull_image(image, &bundle_path, &source_creds, &Some(&decrypt_config))
                .await?;
        }

        let mut sandbox = self.sandbox.lock().await;
        sandbox.images.insert(String::from(image), cid.to_string());
        Ok(image.to_owned())
    }
}

#[async_trait]
impl protocols::image_ttrpc_async::Image for ImageService {
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
