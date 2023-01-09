// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// This crate is used to share code among tests

use std::path::PathBuf;

use rand::{
    distributions::Alphanumeric,
    {thread_rng, Rng},
};

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

pub fn gen_id(len: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}
