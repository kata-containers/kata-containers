// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::sync::Arc;

use anyhow::{anyhow, ensure, Result};
use async_trait::async_trait;
use protocols::image;
use tokio::sync::Mutex;
use ttrpc::{self, error::get_rpc_status as ttrpc_error};

use crate::rpc::{verify_cid, CONTAINER_BASE};
use crate::sandbox::Sandbox;
use crate::AGENT_CONFIG;

const SKOPEO_PATH: &str = "/usr/bin/skopeo";
const UMOCI_PATH: &str = "/usr/local/bin/umoci";
const IMAGE_OCI: &str = "image_oci:latest";

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

pub struct ImageService {
    sandbox: Arc<Mutex<Sandbox>>,
}

impl ImageService {
    pub fn new(sandbox: Arc<Mutex<Sandbox>>) -> Self {
        Self { sandbox }
    }

    fn pull_image_from_registry(
        image: &str,
        cid: &str,
        source_creds: &Option<&str>,
        policy_path: &Option<&String>,
    ) -> Result<()> {
        let source_image = format!("{}{}", "docker://", image);

        let tmp_cid_path = Path::new("/tmp/").join(cid);
        let oci_path = tmp_cid_path.join(IMAGE_OCI);
        let target_path_oci = format!("oci://{}", oci_path.to_string_lossy());

        fs::create_dir_all(&oci_path)?;

        info!(sl!(), "Attempting to pull image {}...", &source_image);

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

    async fn pull_image(&self, req: &image::PullImageRequest) -> Result<String> {
        let image = req.get_image();
        let mut cid = req.get_container_id();

        if cid.is_empty() {
            let v: Vec<&str> = image.rsplit('/').collect();
            if !v[0].is_empty() {
                cid = v[0]
            } else {
                return Err(anyhow!("Invalid image name. {}", image));
            }
        } else {
            verify_cid(cid)?;
        }

        let source_creds = (!req.get_source_creds().is_empty()).then(|| req.get_source_creds());

        // Read the policy path from the agent config
        let config_policy_path = &AGENT_CONFIG.read().await.container_policy_path;
        let policy_path = (!config_policy_path.is_empty()).then(|| config_policy_path);
        info!(sl!(), "Using container policy_path: {:?}...", &policy_path);

        Self::pull_image_from_registry(image, cid, &source_creds, &policy_path)?;
        Self::unpack_image(cid)?;

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
