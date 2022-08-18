// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use async_trait::async_trait;

use super::{utils, ShareFsMount, ShareFsMountResult, ShareFsRootfsConfig, ShareFsVolumeConfig};

pub struct VirtiofsShareMount {
    id: String,
}

impl VirtiofsShareMount {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

#[async_trait]
impl ShareFsMount for VirtiofsShareMount {
    async fn share_rootfs(&self, config: ShareFsRootfsConfig) -> Result<ShareFsMountResult> {
        // TODO: select virtiofs or support nydus
        let guest_path = utils::share_to_guest(
            &config.source,
            &config.target,
            &self.id,
            &config.cid,
            config.readonly,
            false,
        )
        .context("share to guest")?;
        Ok(ShareFsMountResult { guest_path })
    }

    async fn share_volume(&self, config: ShareFsVolumeConfig) -> Result<ShareFsMountResult> {
        let guest_path = utils::share_to_guest(
            &config.source,
            &config.target,
            &self.id,
            &config.cid,
            config.readonly,
            true,
        )
        .context("share to guest")?;
        Ok(ShareFsMountResult { guest_path })
    }
}
