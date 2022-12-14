// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use async_trait::async_trait;

use super::Volume;

#[derive(Debug)]
pub(crate) struct BlockVolume {}

/// BlockVolume: block device volume
impl BlockVolume {
    pub(crate) fn new(_m: &oci::Mount) -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl Volume for BlockVolume {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        todo!()
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        todo!()
    }

    async fn cleanup(&self) -> Result<()> {
        warn!(sl!(), "Cleaning up BlockVolume is still unimplemented.");
        Ok(())
    }
}

pub(crate) fn is_block_volume(_m: &oci::Mount) -> bool {
    // attach block device
    false
}
