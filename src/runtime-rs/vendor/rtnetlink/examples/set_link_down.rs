// SPDX-License-Identifier: MIT

use futures::stream::TryStreamExt;
use rtnetlink::{new_connection, Error, Handle};
use std::env;

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        usage();
        return Ok(());
    }
    let link_name = &args[1];

    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    set_link_down(handle, link_name.to_string())
        .await
        .map_err(|e| format!("{}", e))
}

async fn set_link_down(handle: Handle, name: String) -> Result<(), Error> {
    let mut links = handle.link().get().match_name(name.clone()).execute();
    if let Some(link) = links.try_next().await? {
        handle
            .link()
            .set(link.header.index)
            .down()
            .execute()
            .await?
    } else {
        println!("no link link {} found", name);
    }
    Ok(())
}

fn usage() {
    eprintln!(
        "usage:
    cargo run --example set_link_down -- <link name>

Note that you need to run this program as root. Instead of running cargo as root,
build the example normally:

    cd netlink-ip ; cargo build --example set_link_down

Then find the binary in the target directory:

    cd ../target/debug/example ; sudo ./set_link_down <link_name>"
    );
}
