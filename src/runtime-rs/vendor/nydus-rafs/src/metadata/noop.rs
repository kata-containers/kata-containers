// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! A noop meta data driver for place-holding.

use std::io::Result;
use std::sync::Arc;

use nydus_utils::digest;
use storage::device::BlobInfo;

use crate::metadata::{Inode, RafsInode, RafsSuperBlock, RafsSuperInodes};
use crate::{RafsIoReader, RafsResult};

#[derive(Default)]
pub struct NoopSuperBlock {}

impl NoopSuperBlock {
    pub fn new() -> Self {
        Self::default()
    }
}

impl RafsSuperInodes for NoopSuperBlock {
    fn get_max_ino(&self) -> Inode {
        unimplemented!()
    }

    fn get_inode(&self, _ino: Inode, _digest_validate: bool) -> Result<Arc<dyn RafsInode>> {
        unimplemented!()
    }

    fn validate_digest(
        &self,
        _inode: Arc<dyn RafsInode>,
        _recursive: bool,
        _digester: digest::Algorithm,
    ) -> Result<bool> {
        unimplemented!()
    }
}

impl RafsSuperBlock for NoopSuperBlock {
    fn load(&mut self, _r: &mut RafsIoReader) -> Result<()> {
        unimplemented!()
    }

    fn update(&self, _r: &mut RafsIoReader) -> RafsResult<()> {
        unimplemented!()
    }

    fn destroy(&mut self) {}

    fn get_blob_infos(&self) -> Vec<Arc<BlobInfo>> {
        Vec::new()
    }

    fn root_ino(&self) -> u64 {
        unimplemented!()
    }
}
