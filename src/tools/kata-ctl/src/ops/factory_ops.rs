use anyhow::Result;

use crate::args::{FactoryArgs, FactorySubCommand};
use virt_container::factory;

use slog::info;

macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "factory_ops"))
    };
}

pub async fn handle_factory(factory_args: FactoryArgs) -> Result<()> {
    info!(sl!(), "handle_factory");
    match &factory_args.command {
        FactorySubCommand::Init => {
            if let Err(e) = factory::init_factory_command().await {
                error!(sl!(), "Failed to initialize factory command: {}", e);
            }
        }
        FactorySubCommand::Destroy => {
            if let Err(e) = factory::destroy_factory_command().await {
                error!(sl!(), "Failed to destory factory: {}", e);
            }
        }
        FactorySubCommand::Status => {
            if let Err(e) = factory::status_factory_command().await {
                error!(sl!(), "Failed to status factory: {}", e);
            }
        }
    }

    Ok(())
}
