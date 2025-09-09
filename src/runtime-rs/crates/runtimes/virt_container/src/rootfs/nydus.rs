// Copyright (c) 2024 Kata Containers
//
// SPDX-License-Identifier: Apache-2.0
//

use nydusd::Nydusd;
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use base64;
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

const KATA_GUEST_NYDUS_IMAGE_DIR: &str = "/run/kata-containers/shared/images";
const KATA_GUEST_SHARE_DIR: &str = "/run/kata-containers/shared/containers";
const SNAPSHOT_DIR: &str = "snapshotdir";
const LOWER_DIR: &str = "lowerdir";
const UPPER_DIR: &str = "upperdir";
const WORK_DIR: &str = "workdir";
const OVERLAY_FS_TYPE: &str = "overlay";
const KATA_OVERLAY_DEV_TYPE: &str = "overlayfs";
const EXTRA_OPTION_KEY: &str = "extraoption=";

#[derive(Deserialize, Debug)]
struct ExtraOption {
    #[serde(rename = "source")]
    source: String,
    #[serde(rename = "config")]
    config: String,
    #[serde(rename = "snapshotdir")]
    snapshotdir: String,
}

pub struct NydusRootfs {
    nydusd: Arc<RwLock<dyn Nydusd>>,
    container_id: String,
    rootfs_suffix: String,
}

impl NydusRootfs {
    pub fn new(nydusd: Arc<RwLock<dyn Nydusd>>, container_id: &str) -> Self {
        Self {
            nydusd,
            container_id: container_id.to_string(),
            rootfs_suffix: "rootfs".to_string(),
        }
    }

    pub async fn setup(&self, options: &[String]) -> Result<Vec<Storage>> {
        let extra_option = parse_extra_option(options).context("failed to parse extra option")?;
        let nydusd = self.nydusd.read().await;

        let _mount_path = nydusd.mount(&extra_option.source).await?;

        // Bind mount snapshot dir
        let container_share_dir = Path::new(KATA_GUEST_SHARE_DIR).join(&self.container_id);
        let snapshot_share_dir = container_share_dir.join(SNAPSHOT_DIR);
        // The bind mount logic will be handled by the caller, here we just specify the paths.
        // This is a simplification from the Go implementation where bind mounting is done here.
        // In Rust, it's better to handle this in the resource manager.
        // For now, we'll assume the snapshot dir is available at the guest path.

        let rootfs_guest_path = Path::new(KATA_GUEST_SHARE_DIR)
            .join(&self.container_id)
            .join(&self.rootfs_suffix);

        let rootfs = Storage {
            mount_point: rootfs_guest_path.to_str().unwrap().to_string(),
            source: OVERLAY_FS_TYPE.to_string(),
            fs_type: OVERLAY_FS_TYPE.to_string(),
            driver: KATA_OVERLAY_DEV_TYPE.to_string(),
            options: vec![
                format!(
                    "{}={}",
                    UPPER_DIR,
                    snapshot_share_dir.join("fs").to_str().unwrap()
                ),
                format!(
                    "{}={}",
                    WORK_DIR,
                    snapshot_share_dir.join("work").to_str().unwrap()
                ),
                format!(
                    "{}={}",
                    LOWER_DIR,
                    Path::new(KATA_GUEST_NYDUS_IMAGE_DIR)
                        .join(&self.container_id)
                        .join(LOWER_DIR)
                        .to_str()
                        .unwrap()
                ),
                "index=off".to_string(),
            ],
            ..Default::default()
        };

        Ok(vec![rootfs])
    }

    pub async fn teardown(&self) -> Result<()> {
        let nydusd = self.nydusd.read().await;
        nydusd.umount(&self.rafs_mount_path()).await
    }

    fn rafs_mount_path(&self) -> String {
        Path::new(KATA_GUEST_NYDUS_IMAGE_DIR)
            .join(&self.container_id)
            .join(LOWER_DIR)
            .to_str()
            .unwrap()
            .to_string()
    }
}

fn parse_extra_option(options: &[String]) -> Result<ExtraOption> {
    let extra_opt_str = options
        .iter()
        .find(|&opt| opt.starts_with(EXTRA_OPTION_KEY))
        .ok_or_else(|| anyhow!("no extraoption found"))?
        .trim_start_matches(EXTRA_OPTION_KEY);

    let decoded = base64::decode(extra_opt_str).context("base64 decoding failed")?;
    let extra_option: ExtraOption =
        serde_json::from_slice(&decoded).context("json unmarshal failed")?;

    if extra_option.config.is_empty()
        || extra_option.snapshotdir.is_empty()
        || extra_option.source.is_empty()
    {
        return Err(anyhow!("extra option is not correct"));
    }

    Ok(extra_option)
}