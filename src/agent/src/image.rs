// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021, 2023 IBM Corporation
// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use safe_path::scoped_join;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use image_rs::image::ImageClient;
use kata_sys_util::validate::verify_id;
use tokio::sync::Mutex;

use crate::rpc::CONTAINER_BASE;
use crate::AGENT_CONFIG;

const KATA_IMAGE_WORK_DIR: &str = "/run/kata-containers/image/";
const CONFIG_JSON: &str = "config.json";
const KATA_PAUSE_BUNDLE: &str = "/pause_bundle";

const K8S_CONTAINER_TYPE_KEYS: [&str; 2] = [
    "io.kubernetes.cri.container-type",
    "io.kubernetes.cri-o.ContainerType",
];

#[rustfmt::skip]
lazy_static! {
    pub static ref IMAGE_SERVICE: Arc<Mutex<Option<ImageService>>> = Arc::new(Mutex::new(None));
}

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "image"))
}

// Function to copy a file if it does not exist at the destination
fn copy_if_not_exists(src: &Path, dst: &Path) -> Result<()> {
    if let Some(dst_dir) = dst.parent() {
        fs::create_dir_all(dst_dir)?;
    }
    fs::copy(src, dst)?;
    Ok(())
}

pub struct ImageService {
    image_client: ImageClient,
}

impl ImageService {
    pub fn new() -> Self {
        Self {
            image_client: ImageClient::new(PathBuf::from(KATA_IMAGE_WORK_DIR)),
        }
    }

    fn get_security_config(&mut self) {
        // Read enable signature verification from the agent config and set it in the image_client
        let enable_signature_verification = &AGENT_CONFIG.enable_signature_verification;
        info!(
            sl(),
            "enable_signature_verification set to: {}", enable_signature_verification
        );
        self.image_client.config.security_validate = *enable_signature_verification;
    }

    /// pause image is packaged in rootfs
    fn unpack_pause_image(cid: &str) -> Result<String> {
        verify_id(cid).context("The guest pause image cid contains invalid characters.")?;

        let guest_pause_bundle = Path::new(KATA_PAUSE_BUNDLE);
        if !guest_pause_bundle.exists() {
            bail!("Pause image not present in rootfs");
        }
        let guest_pause_config = scoped_join(guest_pause_bundle, CONFIG_JSON)?;
        info!(sl(), "use guest pause image cid {:?}", cid);

        let image_oci = oci::Spec::load(guest_pause_config.to_str().ok_or_else(|| {
            anyhow!(
                "Failed to load the guest pause image config from {:?}",
                guest_pause_config
            )
        })?)
        .context("load image config file")?;

        let image_oci_process = image_oci.process.ok_or_else(|| {
            anyhow!("The guest pause image config does not contain a process specification. Please check the pause image.")
        })?;
        info!(
            sl(),
            "pause image oci process {:?}",
            image_oci_process.clone()
        );

        // Ensure that the args vector is not empty before accessing its elements.
        let args = image_oci_process.args;
        // Check the number of arguments.
        if args.is_empty() {
            bail!("The number of args should be greater than or equal to one! Please check the pause image.");
        }

        let pause_bundle = scoped_join(CONTAINER_BASE, cid)?;
        fs::create_dir_all(&pause_bundle)?;
        let pause_rootfs = scoped_join(&pause_bundle, "rootfs")?;
        fs::create_dir_all(&pause_rootfs)?;
        info!(sl(), "pause_rootfs {:?}", pause_rootfs);

        copy_if_not_exists(&guest_pause_config, &pause_bundle.join(CONFIG_JSON))?;
        let arg_path = Path::new(&args[0]).strip_prefix("/")?;
        copy_if_not_exists(
            &guest_pause_bundle.join("rootfs").join(arg_path),
            &pause_rootfs.join(arg_path),
        )?;
        Ok(pause_rootfs.display().to_string())
    }

    /// pull_image is used for call image-rs to pull image in the guest.
    /// # Parameters
    /// - `image`: Image name (exp: quay.io/prometheus/busybox:latest)
    /// - `cid`: Container id
    /// - `image_metadata`: Annotations about the image (exp: "containerd.io/snapshot/cri.layer-digest": "sha256:24fb2886d6f6c5d16481dd7608b47e78a8e92a13d6e64d87d57cb16d5f766d63")
    /// # Returns
    /// - The image rootfs bundle path. (exp. /run/kata-containers/cb0b47276ea66ee9f44cc53afa94d7980b57a52c3f306f68cb034e58d9fbd3c6/rootfs)
    pub async fn pull_image(
        &mut self,
        image: &str,
        cid: &str,
        image_metadata: &HashMap<String, String>,
    ) -> Result<String> {
        info!(sl(), "image metadata: {image_metadata:?}");

        //Check whether the image is for sandbox or for container.
        let mut is_sandbox = false;
        for key in K8S_CONTAINER_TYPE_KEYS.iter() {
            if let Some(value) = image_metadata.get(key as &str) {
                if value == "sandbox" {
                    is_sandbox = true;
                    break;
                }
            }
        }

        if is_sandbox {
            let mount_path = Self::unpack_pause_image(cid)?;
            return Ok(mount_path);
        }

        // Image layers will store at KATA_IMAGE_WORK_DIR, generated bundles
        // with rootfs and config.json will store under CONTAINER_BASE/cid/images.
        let bundle_path = scoped_join(CONTAINER_BASE, cid)?;
        fs::create_dir_all(&bundle_path)?;
        info!(sl(), "pull image {image:?}, bundle path {bundle_path:?}");

        self.get_security_config();

        let res = self
            .image_client
            .pull_image(image, &bundle_path, &None, &None)
            .await;
        match res {
            Ok(image) => {
                info!(
                    sl(),
                    "pull and unpack image {image:?}, cid: {cid:?} succeeded."
                );
            }
            Err(e) => {
                error!(
                    sl(),
                    "pull and unpack image {image:?}, cid: {cid:?} failed with {:?}.",
                    e.to_string()
                );
                return Err(e);
            }
        };
        let image_bundle_path = scoped_join(&bundle_path, "rootfs")?;
        Ok(image_bundle_path.as_path().display().to_string())
    }
}

/// Set proxy environment from AGENT_CONFIG
pub async fn set_proxy_env_vars() {
    if env::var("HTTPS_PROXY").is_err() {
        let https_proxy = &AGENT_CONFIG.https_proxy;
        if !https_proxy.is_empty() {
            env::set_var("HTTPS_PROXY", https_proxy);
        }
    }

    match env::var("HTTPS_PROXY") {
        Ok(val) => info!(sl(), "https_proxy is set to: {}", val),
        Err(e) => info!(sl(), "https_proxy is not set ({})", e),
    };

    if env::var("NO_PROXY").is_err() {
        let no_proxy = &AGENT_CONFIG.no_proxy;
        if !no_proxy.is_empty() {
            env::set_var("NO_PROXY", no_proxy);
        }
    }

    match env::var("NO_PROXY") {
        Ok(val) => info!(sl(), "no_proxy is set to: {}", val),
        Err(e) => info!(sl(), "no_proxy is not set ({})", e),
    };
}

/// Init the image service
pub async fn init_image_service() {
    let image_service = ImageService::new();
    *IMAGE_SERVICE.lock().await = Some(image_service);
}

pub async fn pull_image(
    image: &str,
    cid: &str,
    image_metadata: &HashMap<String, String>,
) -> Result<String> {
    let image_service = IMAGE_SERVICE.clone();
    let mut image_service = image_service.lock().await;
    let image_service = image_service
        .as_mut()
        .expect("Image Service not initialized");

    image_service.pull_image(image, cid, image_metadata).await
}
