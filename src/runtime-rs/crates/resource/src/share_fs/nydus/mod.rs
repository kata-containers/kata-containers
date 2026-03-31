// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
//

pub mod nydus_client;
pub mod nydus_daemon;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountRequest {
    pub fs_type: String,
    pub source: PathBuf,
    pub config: String,
}

impl MountRequest {
    pub fn new(fs_type: &str, source: &Path, config: &str) -> Self {
        Self {
            fs_type: fs_type.to_string(),
            source: source.to_path_buf(),
            config: config.to_string(),
        }
    }
}
