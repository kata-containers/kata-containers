use anyhow::Result;
use slog::info;

// use clap::Parser;
use crate::args::{FactoryArgs, FactorySubCommand};
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "kata-ctl_main"))
    };
}

pub fn handle_factory(factory_args: FactoryArgs) -> Result<()> {
    
    // info!(sl!(), "vmtemplate called");
    match &factory_args.command {
        FactorySubCommand::Init => {
            println!("Factory init called");
        }
        FactorySubCommand::Destroy => {
            println!("Factory destroy called");
        }
        FactorySubCommand::Status => {
            println!("Factory status called");
        }
    }

    Ok(())
}
