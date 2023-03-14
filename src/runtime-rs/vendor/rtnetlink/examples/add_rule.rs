// SPDX-License-Identifier: MIT

use std::env;

use ipnetwork::Ipv4Network;
use rtnetlink::{new_connection, Error, Handle};

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        usage();
        return Ok(());
    }

    let dest: Ipv4Network = args[1].parse().unwrap_or_else(|_| {
        eprintln!("invalid destination");
        std::process::exit(1);
    });

    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    if let Err(e) = add_rule(&dest, handle.clone()).await {
        eprintln!("{}", e);
    }
    Ok(())
}

async fn add_rule(dest: &Ipv4Network, handle: Handle) -> Result<(), Error> {
    let rule = handle.rule();
    rule.add()
        .v4()
        .destination_prefix(dest.ip(), dest.prefix())
        .execute()
        .await?;

    Ok(())
}

fn usage() {
    eprintln!(
        "usage:
    cargo run --example add_rule -- <destination>/<prefix_length> <gateway>

Note that you need to run this program as root. Instead of running cargo as root,
build the example normally:

    cd rtnetlink ; cargo build --example add_rule

Then find the binary in the target directory:

    cd ../target/debug/example ; sudo ./add_rule <destination>/<prefix_length> <gateway>"
    );
}
