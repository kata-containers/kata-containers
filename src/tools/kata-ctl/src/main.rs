// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use clap::{crate_name, App, Arg, SubCommand};
use std::process::exit;

mod utils;
mod version;

const DESCRIPTION_TEXT: &str = r#"DESCRIPTION:
    kata-ctl description placeholder."#;

const ABOUT_TEXT: &str = "Kata Containers control tool";

const NAME: &str = "kata-ctl";

fn real_main() -> Result<()> {
    let name = crate_name!();
    let version = version::get();

    let app = App::new(name)
        .version(&*version)
        .about(ABOUT_TEXT)
        .long_about(DESCRIPTION_TEXT)
        .subcommand(
            SubCommand::with_name("check")
            .about("tests if system can run Kata Containers")
        )
        .subcommand(
            SubCommand::with_name("command-example")
            .about("(remove when other subcommands have sufficient detail)")
            .arg(
                Arg::with_name("arg-example-1")
                .long("arg-example-1")
                .help("arg example for command-example")
                .takes_value(true)
                )
        )
        .subcommand(
            SubCommand::with_name("direct-volume")
            .about("directly assign a volume to Kata Containers to manage")
        )
        .subcommand(
            SubCommand::with_name("env")
            .about("display settings. Default to TOML")
        )
        .subcommand(
            SubCommand::with_name("exec")
            .about("enter into guest by debug console")
        )
        .subcommand(
            SubCommand::with_name("factory")
            .about("manage vm factory")
        )
        .subcommand(
            SubCommand::with_name("help")
            .about("shows a list of commands or help for one command")
        )
        .subcommand(
            SubCommand::with_name("iptables")
            .about("")
        )
        .subcommand(
            SubCommand::with_name("metrics")
            .about("gather metrics associated with infrastructure used to run a sandbox")
        )
        .subcommand(
            SubCommand::with_name("version")
            .about("display version details")

        );

    let args = app.get_matches();

    let subcmd = args
        .subcommand_name()
        .ok_or_else(|| anyhow!("need sub-command"))?;

    match subcmd {
        "command-example" => {
            println!("{}", utils::command_example(args));
            Ok(())
        }
        "check" => {
            println!("Not implemented");
            Ok(())
        }
        "direct-volume" => {
            println!("Not implemented");
            Ok(())
        }
        "env" => {
            println!("Not implemented");
            Ok(())
        }
        "exec" => {
            println!("Not implemented");
            Ok(())
        }
        "factory" => {
            println!("Not implemented");
            Ok(())
        }
        "help" => {
            println!("Not implemented");
            Ok(())
        }
        "iptables" => {
            println!("Not implemented");
            Ok(())
        }
        "metrics" => {
            println!("Not implemented");
            Ok(())
        }
        "version" => {
            println!("{} version {} (type: rust)", NAME, version);
            Ok(())
        }
        _ => return Err(anyhow!(format!("invalid sub-command: {:?}", subcmd))),
    }

}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("ERROR: {:#?}", e);
        exit(1);
    }
}
