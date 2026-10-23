// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;

use anyhow::{Context, Result};
use common::{config::load_config, RuntimeHandler};
use tokio::runtime::Runtime;
use virt_container::{factory, VirtContainer};

use crate::args::{FactoryArgs, FactorySubCommand};

pub fn handle_factory(factory_args: FactoryArgs) -> Result<()> {
    VirtContainer::init().context("failed to initialize virt container")?;
    let toml_config =
        load_config(&HashMap::new(), &None).context("failed to load runtime configuration")?;

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
