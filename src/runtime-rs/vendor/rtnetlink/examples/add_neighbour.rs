// SPDX-License-Identifier: MIT

use futures::stream::TryStreamExt;
use rtnetlink::{new_connection, Error, Handle};
use std::{env, net::IpAddr};

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        usage();
        return Ok(());
    }

    let link_name = &args[1];
    let ip: IpAddr = args[2].parse().unwrap_or_else(|_| {
        eprintln!("invalid IP address");
        std::process::exit(1);
    });

    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    if let Err(e) = add_neighbour(link_name, ip, handle.clone()).await {
        eprintln!("{}", e);
    }
    Ok(())
}

async fn add_neighbour(link_name: &str, ip: IpAddr, handle: Handle) -> Result<(), Error> {
    let mut links = handle
        .link()
        .get()
        .match_name(link_name.to_string())
        .execute();
    if let Some(link) = links.try_next().await? {
        handle
            .neighbours()
            .add(link.header.index, ip)
            .execute()
            .await?;
        println!("Done");
    }

    Ok(())
}

fn usage() {
    eprintln!(
        "usage:
    cargo run --example add_neighbour -- <link_name> <ip_address>

Note that you need to run this program as root. Instead of running cargo as root,
build the example normally:

    cd rtnetlink ; cargo build --example add_neighbour

Then find the binary in the target directory:

    cd ../target/debug/example ; sudo ./add_neighbour <link_name> <ip_address>"
    );
}
