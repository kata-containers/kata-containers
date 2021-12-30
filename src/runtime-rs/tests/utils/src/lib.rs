// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

// This crate is used to share code among tests

use std::path::PathBuf;

pub fn get_kata_config_file() -> PathBuf {
    let target = format!(
        "{}/../texture/kata-containers-configuration.toml",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::canonicalize(target).unwrap()
}

pub fn get_image_bundle_path() -> PathBuf {
    let target = format!("{}/../texture/image-bundle", env!("CARGO_MANIFEST_DIR"));
    std::fs::canonicalize(target).unwrap()
}
