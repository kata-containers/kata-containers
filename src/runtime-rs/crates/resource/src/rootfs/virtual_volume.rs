// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
#![allow(dead_code)]

use std::path::Path;
use std::str::FromStr;
use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use oci_spec::runtime as oci;
use serde_json;
use tokio::sync::RwLock;

use hypervisor::device::device_manager::DeviceManager;
use kata_types::{
    annotations,
    container::ContainerType,
    mount::{KataVirtualVolume, KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL},
};

/// Image guest-pull related consts
const KUBERNETES_CRI_IMAGE_NAME: &str = "io.kubernetes.cri.image-name";
const KUBERNETES_CRIO_IMAGE_NAME: &str = "io.kubernetes.cri-o.ImageName";
const KATA_VIRTUAL_VOLUME_PREFIX: &str = "io.katacontainers.volume=";
const KATA_VIRTUAL_VOLUME_TYPE_OVERLAY_FS: &str = "overlayfs";
const KATA_GUEST_ROOT_SHARED_FS: &str = "/run/kata-containers/";

const CRI_CONTAINER_TYPE_KEY_LIST: &[&str] = &[
    // cri containerd
    annotations::cri_containerd::CONTAINER_TYPE_LABEL_KEY,
    // cri-o
    annotations::crio::CONTAINER_TYPE_LABEL_KEY,
];

/// Retrieves the image reference from OCI spec annotations.
///
/// It checks known Kubernetes CRI and CRI-O annotation keys for the container type.
/// If the container is a PodSandbox, it returns "pause".
/// Otherwise, it attempts to find the image name using the appropriate Kubernetes
/// annotation key.
pub fn get_image_reference(spec_annotations: &HashMap<String, String>) -> Result<&str> {
    info!(
        sl!(),
        "get image reference from spec annotation: {:?}", spec_annotations
    );
    for &key in CRI_CONTAINER_TYPE_KEY_LIST {
        if let Some(type_value) = spec_annotations.get(key) {
            if let Ok(container_type) = ContainerType::from_str(type_value) {
                return match container_type {
                    ContainerType::PodSandbox => Ok("pause"),
                    _ => {
                        let image_name_key = if key == annotations::crio::CONTAINER_TYPE_LABEL_KEY {
                            KUBERNETES_CRIO_IMAGE_NAME
                        } else {
                            KUBERNETES_CRI_IMAGE_NAME
                        };

                        spec_annotations
                            .get(image_name_key)
                            .map(AsRef::as_ref)
                            .ok_or_else(|| anyhow!("get image reference failed"))
                    }
                };
            }
        }
    }

    Err(anyhow!("no target image reference found"))
}

/// Handles storage configuration for KataVirtualVolume based on its type.
/// Specifically processes `KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL` type.
fn handle_virtual_volume_storage(
    cid: &str,
    annotations: &HashMap<String, String>,
    virt_volume: &KataVirtualVolume,
) -> Result<agent::Storage> {
    if virt_volume.volume_type.as_str() == KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL {
        let mut vol = handle_image_guest_pull_virtual_volume(annotations, virt_volume)
            .context("handle image guest pull failed")?;
        vol.fs_type = KATA_VIRTUAL_VOLUME_TYPE_OVERLAY_FS.to_string();
        vol.mount_point = Path::new(KATA_GUEST_ROOT_SHARED_FS)
            .join(cid)
            .join("rootfs")
            .display()
            .to_string();

        Ok(vol)
    } else {
        Err(anyhow!("Error in handle_virtual_volume_storage"))
    }
}

/// Processes a virtual volume specifically for image guest-pull scenarios.
/// It enriches the volume info with image reference and metadata, then serializes it into agent.Storage.
fn handle_image_guest_pull_virtual_volume(
    annotations: &HashMap<String, String>,
    virtual_volume_info: &KataVirtualVolume,
) -> Result<agent::Storage> {
    let container_annotations = annotations.clone();
    let image_ref = get_image_reference(annotations).context("get image reference failed.")?;
    let mut virtual_volume_info = virtual_volume_info.clone();
    virtual_volume_info.source = image_ref.to_owned();

    // Merge metadata
    for (k, v) in container_annotations {
        if let Some(ref mut image_pull) = virtual_volume_info.image_pull {
            image_pull.metadata.insert(k, v);
        }
    }

    let mut vol = agent::Storage {
        driver: KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL.to_string(),
        driver_options: Vec::new(),
        ..Default::default()
    };

    // Serialize ImagePull as JSON
    let no = serde_json::to_string(&virtual_volume_info.image_pull)
        .map_err(|e| anyhow!(e.to_string()))?;
    vol.driver_options
        .push(format!("{}={}", KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL, no));

    vol.source = virtual_volume_info.source;

    Ok(vol.clone())
}

#[derive(Clone, Debug, Default)]
pub(crate) struct VirtualVolume {
    guest_path: PathBuf,
    storages: Vec<agent::Storage>,
}

impl VirtualVolume {
    pub(crate) async fn new(
        cid: &str,
        annotations: &HashMap<String, String>,
        options: Vec<String>,
    ) -> Result<Self> {
        let mut volumes = Vec::new();

        for o in options.iter() {
            // Iterate over reference to avoid consuming options
            if let Some(stripped_str) = o.strip_prefix(KATA_VIRTUAL_VOLUME_PREFIX) {
                // Ensure `from_base64` provides a descriptive error on failure
                let virt_volume = KataVirtualVolume::from_base64(stripped_str).context(format!(
                    "Failed to decode KataVirtualVolume from base64: {}",
                    stripped_str
                ))?;

                let vol = handle_virtual_volume_storage(cid, annotations, &virt_volume)
                    .context("Failed to handle virtual volume storage object")?;
                volumes.push(vol);
            }
        }

        let guest_path = Path::new(KATA_GUEST_ROOT_SHARED_FS)
            .join(cid)
            .join("rootfs")
            .to_path_buf();

        Ok(VirtualVolume {
            guest_path,
            storages: volumes,
        })
    }
}

#[async_trait]
impl super::Rootfs for VirtualVolume {
    async fn get_guest_rootfs_path(&self) -> Result<String> {
        Ok(self.guest_path.display().to_string().clone())
    }

    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![])
    }

    async fn get_storage(&self) -> Option<agent::Storage> {
        Some(self.storages[0].clone())
    }

    async fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        Ok(())
    }
}

pub fn is_kata_virtual_volume(m: &kata_types::mount::Mount) -> bool {
    let options = m.options.clone();
    options
        .iter()
        .any(|o| o.starts_with(&KATA_VIRTUAL_VOLUME_PREFIX.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::get_image_reference;
    use std::collections::HashMap;

    #[test]
    fn test_get_image_ref() {
        // Test Standard cri-containerd image name
        let mut annotations_x = HashMap::new();
        annotations_x.insert(
            "io.kubernetes.cri.container-type".to_string(),
            "container".to_string(),
        );
        annotations_x.insert(
            "io.kubernetes.cri.image-name".to_string(),
            "example-image-x".to_string(),
        );
        let image_ref_result_crio = get_image_reference(&annotations_x);
        assert!(image_ref_result_crio.is_ok());
        assert_eq!(image_ref_result_crio.unwrap(), "example-image-x");

        // Test cri-o image name
        let mut annotations_crio = HashMap::new();
        annotations_crio.insert(
            "io.kubernetes.cri-o.ContainerType".to_string(),
            "container".to_string(),
        );
        annotations_crio.insert(
            "io.kubernetes.cri-o.ImageName".to_string(),
            "example-image-y".to_string(),
        );
        let image_ref_result_crio = get_image_reference(&annotations_crio);
        assert!(image_ref_result_crio.is_ok());
        assert_eq!(image_ref_result_crio.unwrap(), "example-image-y");

        // Test PodSandbox type
        let mut annotations_pod_sandbox = HashMap::new();
        annotations_pod_sandbox.insert(
            "io.kubernetes.cri.container-type".to_string(),
            "sandbox".to_string(),
        );
        let image_ref_result_pod_sandbox = get_image_reference(&annotations_pod_sandbox);
        assert!(image_ref_result_pod_sandbox.is_ok());
        assert_eq!(image_ref_result_pod_sandbox.unwrap(), "pause");
    }
}
