// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;

use super::Volume;

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

impl Volume for DefaultVolume {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(vec![])
    }

    fn cleanup(&self) -> Result<()> {
        todo!()
    }
}
