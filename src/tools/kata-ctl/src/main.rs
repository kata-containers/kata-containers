// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use clap::{crate_name, App, Arg, SubCommand};
use std::process::exit;

mod arch;
mod check;
mod utils;
mod version;

const DESCRIPTION_TEXT: &str = r#"DESCRIPTION:
    kata-ctl description placeholder."#;

const ABOUT_TEXT: &str = "Kata Containers control tool";

const NAME: &str = "kata-ctl";

fn run_checks(global_args: clap::ArgMatches) -> Result<()> {
    let args = global_args
        .subcommand_matches("check")
        .ok_or_else(|| anyhow!("BUG: missing sub-command arguments"))?;

    let no_network_check = args.is_present("no-network-checks");

    // run architecture-agnostic tests
    if !no_network_check {
        // run code that uses network checks
        let _network_checks = check::run_network_checks();
    }

    // run architecture-specific tests
    let _all_checks = arch::check(global_args);

    Ok(())
}

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
                .arg(
                    Arg::with_name("no-network-checks")
                        .long("no-network-checks")
                        .help("run check with no network checks")
                        .takes_value(false),
                ),
        )
        .subcommand(
            SubCommand::with_name("direct-volume")
                .about("directly assign a volume to Kata Containers to manage"),
        )
        .subcommand(SubCommand::with_name("env").about("display settings. Default to TOML"))
        .subcommand(SubCommand::with_name("exec").about("enter into guest by debug console"))
        .subcommand(SubCommand::with_name("factory").about("manage vm factory"))
        .subcommand(
            SubCommand::with_name("help").about("shows a list of commands or help for one command"),
        )
        .subcommand(SubCommand::with_name("iptables").about(""))
        .subcommand(
            SubCommand::with_name("metrics")
                .about("gather metrics associated with infrastructure used to run a sandbox"),
        )
        .subcommand(SubCommand::with_name("version").about("display version details"));

    let args = app.get_matches();

    let subcmd = args
        .subcommand_name()
        .ok_or_else(|| anyhow!("need sub-command"))?;

    match subcmd {
        "check" => {
            match run_checks(args) {
                Ok(_result) => println!("check may not be fully implemented"),
                Err(err) => println!("{}", err),
            }
            Ok(())
        }
        "direct-volume" => {
            unimplemented!("Not implemented");
        }
        "env" => {
            unimplemented!("Not implemented");
        }
        "exec" => {
            unimplemented!("Not implemented");
        }
        "factory" => {
            unimplemented!("Not implemented");
        }
        "help" => {
            unimplemented!("Not implemented");
        }
        "iptables" => {
            unimplemented!("Not implemented");
        }
        "metrics" => {
            unimplemented!("Not implemented");
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
