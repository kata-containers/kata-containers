// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
//

pub mod nydus_client;

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountRequest {
    pub fs_type: String,
    pub source: PathBuf,
    pub config: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
}

#[allow(dead_code)]
impl MountRequest {
    pub fn new(fs_type: &str, source: &PathBuf, config: &str) -> Self {
        Self {
            fs_type: fs_type.to_string(),
            source: source.clone(),
            config: config.to_string(),
            overlay: None,
        }
    }

    pub fn new_with_overlay(fs_type: &str, source: &PathBuf, config: &str, overlay_config: &str) -> Self {
        Self {
            fs_type: fs_type.to_string(),
            source: source.clone(),
            config: config.to_string(),
            overlay: Some(overlay_config.to_string()),
        }
    }
}
