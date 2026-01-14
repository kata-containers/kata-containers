// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use tokio::runtime::Runtime;
use virt_container::factory;

use crate::args::{FactoryArgs, FactorySubCommand};

pub fn handle_factory(factory_args: FactoryArgs) -> Result<()> {
    let rt = Runtime::new().context("failed to create Tokio runtime")?;
    rt.block_on(async {
        match &factory_args.command {
            FactorySubCommand::Init => {
                factory::init_factory_command()
                    .await
                    .context("failed to initialize factory")?;
            }
            FactorySubCommand::Destroy => {
                factory::destroy_factory_command()
                    .await
                    .context("failed to destroy factory")?;
            }
            FactorySubCommand::Status => {
                factory::status_factory_command()
                    .await
                    .context("failed to query factory status")?;
            }
        }
        Ok(())
    })
}
