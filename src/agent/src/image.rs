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
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use image_rs::builder::ClientBuilder;
use image_rs::image::ImageClient;
use kata_sys_util::validate::verify_id;
use oci_spec::runtime as oci;
use tokio::sync::Mutex;

use crate::rpc::CONTAINER_BASE;
use crate::AGENT_CONFIG;

use kata_types::mount::KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL;
use protocols::agent::Storage;

pub const KATA_IMAGE_WORK_DIR: &str = "/run/kata-containers/image/";
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
    pub async fn new() -> Result<Self> {
        let mut image_client_builder =
            ClientBuilder::default().work_dir(KATA_IMAGE_WORK_DIR.into());
        #[cfg(feature = "guest-pull")]
        {
            if !AGENT_CONFIG.image_registry_auth.is_empty() {
                let registry_auth = &AGENT_CONFIG.image_registry_auth;
                debug!(sl(), "Set registry auth file {:?}", registry_auth);
                image_client_builder = image_client_builder
                    .authenticated_registry_credentials_uri(registry_auth.into());
            }

            let enable_signature_verification = &AGENT_CONFIG.enable_signature_verification;
            debug!(
                sl(),
                "Enable image signature verification: {:?}", enable_signature_verification
            );
            if !AGENT_CONFIG.image_policy_file.is_empty() && *enable_signature_verification {
                let image_policy_file = &AGENT_CONFIG.image_policy_file;
                debug!(sl(), "Use image policy file {:?}", image_policy_file);
                image_client_builder =
                    image_client_builder.image_security_policy_uri(image_policy_file.into());
            }
        }
        let image_client = image_client_builder.build().await?;
        Ok(Self { image_client })
    }

    /// get guest pause image process specification
    fn get_pause_image_process() -> Result<oci::Process> {
        let guest_pause_bundle = Path::new(KATA_PAUSE_BUNDLE);
        if !guest_pause_bundle.exists() {
            bail!("Pause image not present in rootfs");
        }
        let guest_pause_config = scoped_join(guest_pause_bundle, CONFIG_JSON)?;

        let image_oci = oci::Spec::load(guest_pause_config.to_str().ok_or_else(|| {
            anyhow!(
                "Failed to load the guest pause image config from {:?}",
                guest_pause_config
            )
        })?)
        .context("load image config file")?;

        let image_oci_process = image_oci.process().as_ref().ok_or_else(|| {
            anyhow!("The guest pause image config does not contain a process specification. Please check the pause image.")
        })?;
        Ok(image_oci_process.clone())
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

        let image_oci_process = image_oci.process().as_ref().ok_or_else(|| {
            anyhow!("The guest pause image config does not contain a process specification. Please check the pause image.")
        })?;
        info!(
            sl(),
            "pause image oci process {:?}",
            image_oci_process.clone()
        );

        // Ensure that the args vector is not empty before accessing its elements.
        // Check the number of arguments.
        let args = if let Some(args_vec) = image_oci_process.args() {
            args_vec
        } else {
            bail!("The number of args should be greater than or equal to one! Please check the pause image.");
        };

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

    /// check whether the image is for sandbox or for container.
    fn is_sandbox(image_metadata: &HashMap<String, String>) -> bool {
        let mut is_sandbox = false;
        for key in K8S_CONTAINER_TYPE_KEYS.iter() {
            if let Some(value) = image_metadata.get(key as &str) {
                if value == "sandbox" {
                    is_sandbox = true;
                    break;
                }
            }
        }
        is_sandbox
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

        if Self::is_sandbox(image_metadata) {
            let mount_path = Self::unpack_pause_image(cid)?;
            return Ok(mount_path);
        }

        // Image layers will store at KATA_IMAGE_WORK_DIR, generated bundles
        // with rootfs and config.json will store under CONTAINER_BASE/cid/images.
        let bundle_path = scoped_join(CONTAINER_BASE, cid)?;
        fs::create_dir_all(&bundle_path)?;
        info!(sl(), "pull image {image:?}, bundle path {bundle_path:?}");

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

/// get_process overrides the OCI process spec with pause image process spec if needed
pub fn get_process(
    ocip: &oci::Process,
    oci: &oci::Spec,
    storages: Vec<Storage>,
) -> Result<oci::Process> {
    let mut guest_pull = false;
    for storage in storages {
        if storage.driver == KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL {
            guest_pull = true;
            break;
        }
    }
    if guest_pull {
        match oci.annotations() {
            Some(a) => {
                if ImageService::is_sandbox(a) {
                    return ImageService::get_pause_image_process();
                }
            }
            None => {}
        }
    }
    Ok(ocip.clone())
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
pub async fn init_image_service() -> Result<()> {
    let image_service = ImageService::new().await?;
    *IMAGE_SERVICE.lock().await = Some(image_service);
    Ok(())
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
