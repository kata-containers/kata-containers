// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};
use std::sync::Arc;

use anyhow::{anyhow, ensure, Result};
use async_trait::async_trait;
use protocols::image;
use tokio::sync::Mutex;
use ttrpc::{self, error::get_rpc_status as ttrpc_error};

use crate::rpc::{verify_cid, CONTAINER_BASE};
use crate::sandbox::Sandbox;

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

    fn build_oci_path(cid: &str) -> PathBuf {
        let mut oci_path = PathBuf::from("/tmp");
        oci_path.push(cid);
        oci_path.push(IMAGE_OCI);
        oci_path
    }

    fn pull_image_from_registry(image: &str, cid: &str, source_creds: &Option<&str>) -> Result<()> {
        let source_image = format!("{}{}", "docker://", image);

        let mut manifest_path = PathBuf::from("/tmp");
        manifest_path.push(cid);
        manifest_path.push("image_manifest");
        let target_path_manifest = format!("dir://{}", manifest_path.to_string_lossy());

        // Define the target transport and path for the OCI image, without signature
        let oci_path = Self::build_oci_path(cid);
        let target_path_oci = format!("oci://{}", oci_path.to_string_lossy());

        fs::create_dir_all(&manifest_path)?;
        fs::create_dir_all(&oci_path)?;

        info!(sl!(), "Attempting to pull image {}...", &source_image);

        let mut pull_command = Command::new(SKOPEO_PATH);
        pull_command
            // TODO: need to create a proper policy
            .arg("--insecure-policy")
            .arg("copy")
            .arg(source_image)
            .arg(&target_path_manifest);

        if let Some(source_creds) = source_creds {
            pull_command.arg("--src-creds").arg(source_creds);
        }

        let status: ExitStatus = pull_command.status()?;
        ensure!(
            status.success(),
            "failed to copy image manifest: {:?}",
            status,
        );

        // Copy image from one local file-system to another
        // Resulting image is still stored in manifest format, but no longer includes the signature
        // The image with a signature can then be unpacked into a bundle
        let status: ExitStatus = Command::new(SKOPEO_PATH)
            .arg("--insecure-policy")
            .arg("copy")
            .arg(&target_path_manifest)
            .arg(&target_path_oci)
            .arg("--remove-signatures")
            .status()?;

        ensure!(status.success(), "failed to copy image oci: {:?}", status);

        // To save space delete the manifest.
        // TODO LATER - when verify image is added, this will need moving the end of that, if required
        fs::remove_dir_all(&manifest_path)?;
        Ok(())
    }

    fn unpack_image(cid: &str) -> Result<()> {
        let source_path_oci = Self::build_oci_path(cid);
        let target_path_bundle = format!("{}{}{}", CONTAINER_BASE, "/", cid);

        info!(sl!(), "unpack image"; "cid" => cid, "target_bundle_path" => &target_path_bundle);

        // Unpack image
        let status: ExitStatus = Command::new(UMOCI_PATH)
            .arg("unpack")
            .arg("--image")
            .arg(&source_path_oci)
            .arg(&target_path_bundle)
            .status()?;

        ensure!(status.success(), "failed to unpack image: {:?}", status);

        // To save space delete the oci image after unpack
        fs::remove_dir_all(&source_path_oci)?;

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

        Self::pull_image_from_registry(image, cid, &source_creds)?;
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
