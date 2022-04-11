// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use kata_sys_util::mount::Mounter;
use kata_types::mount::Mount;

use super::{Rootfs, ROOTFS};
use crate::share_fs::{ShareFsMount, ShareFsRootfsConfig};

pub(crate) struct ShareFsRootfs {
    guest_path: String,
}

impl ShareFsRootfs {
    pub async fn new(
        share_fs_mount: &Arc<dyn ShareFsMount>,
        cid: &str,
        bundle_path: &str,
        rootfs: &Mount,
    ) -> Result<Self> {
        let bundle_rootfs = format!("{}/{}", bundle_path, ROOTFS);
        rootfs.mount(&bundle_rootfs).context(format!(
            "mount rootfs from {:?} to {}",
            &rootfs, &bundle_rootfs
        ))?;

        let mount_result = share_fs_mount
            .share_rootfs(ShareFsRootfsConfig {
                cid: cid.to_string(),
                source: bundle_rootfs.to_string(),
                target: ROOTFS.to_string(),
                readonly: false,
            })
            .await
            .context("share rootfs")?;

        Ok(ShareFsRootfs {
            guest_path: mount_result.guest_path,
        })
    }
}

#[async_trait]
impl Rootfs for ShareFsRootfs {
    async fn get_guest_rootfs_path(&self) -> Result<String> {
        Ok(self.guest_path.clone())
    }

    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>> {
        todo!()
    }
}
