// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// This crate is used to share code among tests

use anyhow::{anyhow, Result};
use kata_types::config::{QemuConfig, TomlConfig};
use std::{fs, path::PathBuf};

use rand::{
    distributions::Alphanumeric,
    {thread_rng, Rng},
};

fn get_kata_config_file(hypervisor_name: String) -> PathBuf {
    let target = format!(
        "{}/../texture/configuration-{}.toml",
        env!("CARGO_MANIFEST_DIR"),
        hypervisor_name
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

pub fn load_test_config(hypervisor_name: String) -> Result<TomlConfig> {
    match hypervisor_name.as_str() {
        "qemu" => {
            let qemu = QemuConfig::new();
            qemu.register();
        }
        // TODO add other hypervisor test config
        _ => {
            return Err(anyhow!("invalid hypervisor {}", hypervisor_name));
        }
    }

    let content = fs::read_to_string(get_kata_config_file(hypervisor_name))?;
    Ok(TomlConfig::load(&content)?)
}
