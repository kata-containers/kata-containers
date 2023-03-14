// SPDX-License-Identifier: MIT

use futures::TryStreamExt;
use std::{env, net::Ipv4Addr};

use ipnetwork::Ipv4Network;
use rtnetlink::{new_connection, Error, Handle};

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        usage();
        return Ok(());
    }

    let dest: Ipv4Network = args[1].parse().unwrap_or_else(|_| {
        eprintln!("invalid destination");
        std::process::exit(1);
    });
    let iface: String = args[2].parse().unwrap_or_else(|_| {
        eprintln!("invalid interface");
        std::process::exit(1);
    });
    let source: Ipv4Addr = args[3].parse().unwrap_or_else(|_| {
        eprintln!("invalid source");
        std::process::exit(1);
    });

    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    if let Err(e) = add_route(&dest, iface, source, handle.clone()).await {
        eprintln!("{}", e);
    }
    Ok(())
}

async fn add_route(
    dest: &Ipv4Network,
    iface: String,
    source: Ipv4Addr,
    handle: Handle,
) -> Result<(), Error> {
    let iface_idx = handle
        .link()
        .get()
        .match_name(iface)
        .execute()
        .try_next()
        .await?
        .unwrap()
        .header
        .index;

    let route = handle.route();
    route
        .add()
        .v4()
        .destination_prefix(dest.ip(), dest.prefix())
        .output_interface(iface_idx)
        .pref_source(source)
        .execute()
        .await?;
    Ok(())
}

fn usage() {
    eprintln!(
        "usage:
    cargo run --example add_route_pref_src -- <destination>/<prefix_length> <interface> <source>

Note that you need to run this program as root. Instead of running cargo as root,
build the example normally:

    cd rtnetlink ; cargo build --example add_route_pref_src

Then find the binary in the target directory:

    cd ../target/debug/example ; sudo ./add_route_pref_src <destination>/<prefix_length> <interface> <source>"
    );
}
