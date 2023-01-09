// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use async_trait::async_trait;

use super::Volume;

#[derive(Debug)]
pub(crate) struct DefaultVolume {
    mount: oci::Mount,
}

/// DefaultVolume: passthrough the mount to guest
impl DefaultVolume {
    pub fn new(mount: &oci::Mount) -> Result<Self> {
        Ok(Self {
            mount: mount.clone(),
        })
    }
}

#[async_trait]
impl Volume for DefaultVolume {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(vec![])
    }

    async fn cleanup(&self) -> Result<()> {
        // TODO: Clean up DefaultVolume
        warn!(sl!(), "Cleaning up DefaultVolume is still unimplemented.");
        Ok(())
    }
}
