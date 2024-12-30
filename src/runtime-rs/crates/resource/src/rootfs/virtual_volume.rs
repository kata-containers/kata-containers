// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use oci_spec::runtime as oci;
use serde_json;
use tokio::sync::RwLock;

use hypervisor::device::device_manager::DeviceManager;
use kata_types::{annotations, container::ContainerType, mount::KataVirtualVolume};

// Image guest-pull related consts
const KUBERNETES_CRI_IMAGE_NAME: &str = "io.kubernetes.cri.image-name";
const KUBERNETES_CRIO_IMAGE_NAME: &str = "io.kubernetes.cri-o.ImageName";
const KATA_VIRTUAL_VOLUME_PREFIX: &str = "io.katacontainers.volume=";
const KATA_VIRTUAL_VOLUME_TYPE_IMAGE_GUEST_PULL: &str = "image_guest_pull";
const KATA_VIRTUAL_VOLUME_TYPE_OVERLAY_FS: &str = "overlayfs";
const KATA_GUEST_ROOT_SHARED_FS: &str = "/run/kata-containers/";

const CRI_CONTAINER_TYPE_KEY_LIST: &[&str] = &[
    // cri containerd
    annotations::cri_containerd::CONTAINER_TYPE_LABEL_KEY,
    // cri-o
    annotations::crio::CONTAINER_TYPE_LABEL_KEY,
    // docker
    annotations::dockershim::CONTAINER_TYPE_LABEL_KEY,
];

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

fn handle_virtual_volume_storage(
    cid: &str,
    annotations: &HashMap<String, String>,
    virt_volume: &KataVirtualVolume,
) -> Result<agent::Storage> {
    if virt_volume.volume_type.as_str() == KATA_VIRTUAL_VOLUME_TYPE_IMAGE_GUEST_PULL {
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
        driver: KATA_VIRTUAL_VOLUME_TYPE_IMAGE_GUEST_PULL.to_string(),
        driver_options: Vec::new(),
        ..Default::default()
    };

    // Serialize ImagePull as JSON
    let no = serde_json::to_string(&virtual_volume_info.image_pull)
        .map_err(|e| anyhow!(e.to_string()))?;
    vol.driver_options.push(format!(
        "{}={}",
        KATA_VIRTUAL_VOLUME_TYPE_IMAGE_GUEST_PULL, no
    ));

    vol.source = virtual_volume_info.source;

    Ok(vol.clone())
}

#[derive(Clone, Debug, Default)]
pub(crate) struct VirtualVolume {
    guest_path: String,
    storages: Vec<agent::Storage>,
}

impl VirtualVolume {
    pub(crate) async fn new(
        cid: &str,
        annotations: &HashMap<String, String>,
        options: Vec<String>,
    ) -> Result<Self> {
        let mut volumes = Vec::new();

        for o in &options {
            if let Some(stripped_str) = o.strip_prefix(KATA_VIRTUAL_VOLUME_PREFIX) {
                let virt_volume =
                    KataVirtualVolume::from_base64(stripped_str).map_err(|e| anyhow!(e))?;
                if let Ok(vol) = handle_virtual_volume_storage(cid, annotations, &virt_volume) {
                    volumes.push(vol);
                } else {
                    return Err(anyhow!(
                        "Error handling virtual volume storage object".to_string()
                    ));
                }
            }
        }
        let guest_path = Path::new(KATA_GUEST_ROOT_SHARED_FS)
            .join(cid)
            .join("rootfs")
            .display()
            .to_string();

        Ok(VirtualVolume {
            guest_path,
            storages: volumes,
        })
    }
}

#[async_trait]
impl super::Rootfs for VirtualVolume {
    async fn get_guest_rootfs_path(&self) -> Result<String> {
        Ok(self.guest_path.clone())
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
    use std::collections::HashMap;

    use super::get_image_reference;

    #[test]
    fn test_get_image_ref() {
        let mut annotations = HashMap::new();
        //annotations.insert("io.kubernetes.cri-o.ImageName".to_string(), "example-image-cri-o".to_string());
        annotations.insert(
            "io.kubernetes.cri.image-name".to_string(),
            "example-image-cri".to_string(),
        );
        let image_ref_result = get_image_reference(&annotations);
        assert!(image_ref_result.is_ok());
        assert_eq!(image_ref_result.unwrap(), "example-image-cri");
    }
}
