// Copyright (c) 2025 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// Description: Boot UVM for testing container storages/volumes.

use anyhow::{anyhow, Context, Result};
use kata_types::config::TomlConfig;
use slog::info;

// Helper function to parse a configuration file.
pub fn load_config(config_file: &str) -> Result<TomlConfig> {
    info!(sl!(), "Load kata configuration file {}", config_file);

    let (mut toml_config, _) = TomlConfig::load_from_file(config_file)
        .context("Failed to load kata configuration file")?;

    // Update the agent kernel params in hypervisor config
    update_agent_kernel_params(&mut toml_config)?;

    // validate configuration and return the error
    toml_config.validate()?;

    info!(sl!(), "parsed config content {:?}", &toml_config);
    Ok(toml_config)
}

pub fn to_kernel_string(key: String, val: String) -> Result<String> {
    if key.is_empty() && val.is_empty() {
        Err(anyhow!("Empty key and value"))
    } else if key.is_empty() {
        Err(anyhow!("Empty key"))
    } else if val.is_empty() {
        Ok(key.to_string())
    } else {
        Ok(format!("{}{}{}", key, "=", val))
    }
}

fn update_agent_kernel_params(config: &mut TomlConfig) -> Result<()> {
    let mut params = vec![];
    if let Ok(kv) = config.get_agent_kernel_params() {
        for (k, v) in kv.into_iter() {
            if let Ok(s) = to_kernel_string(k.to_owned(), v.to_owned()) {
                params.push(s);
            }
        }
        if let Some(h) = config.hypervisor.get_mut(&config.runtime.hypervisor_name) {
            h.boot_info.add_kernel_params(params);
        }
    }
    Ok(())
}
