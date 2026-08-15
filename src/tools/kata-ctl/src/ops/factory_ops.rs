// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;

use anyhow::{Context, Result};
use common::config::load_runtime_config;
use tokio::runtime::Runtime;
use virt_container::{factory, register_hypervisor_config_plugins};

use crate::args::{FactoryArgs, FactorySubCommand};

pub fn handle_factory(factory_args: FactoryArgs) -> Result<()> {
    register_hypervisor_config_plugins();
    let (toml_config, _) = load_runtime_config(&HashMap::new(), None)
        .context("failed to load runtime configuration")?;

    let rt = Runtime::new().context("failed to create Tokio runtime")?;
    rt.block_on(async {
        match &factory_args.command {
            FactorySubCommand::Init => {
                factory::init_factory_command(toml_config)
                    .await
                    .context("failed to initialize factory")?;
            }
            FactorySubCommand::Destroy => {
                factory::destroy_factory_command(toml_config)
                    .await
                    .context("failed to destroy factory")?;
            }
            FactorySubCommand::Status => {
                factory::status_factory_command(toml_config)
                    .await
                    .context("failed to query factory status")?;
            }
        }
        Ok(())
    })
}
