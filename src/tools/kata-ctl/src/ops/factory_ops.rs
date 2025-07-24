use anyhow::Result;

use crate::factory;
use crate::args::{FactoryArgs, FactorySubCommand};

use slog::{info};

macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "factory_ops"))
    };
}


pub fn handle_factory(factory_args: FactoryArgs) -> Result<()> {
    info!(sl!(), "handle_factory");
    match &factory_args.command {
        FactorySubCommand::Init => {
           let _ = factory::init_factory_command();
        }
        FactorySubCommand::Destroy => {
           let _ = factory::destroy_factory_command();
        }
        FactorySubCommand::Status => {
           let _ = factory::status_factory_command();
        }
    }

    Ok(())
}