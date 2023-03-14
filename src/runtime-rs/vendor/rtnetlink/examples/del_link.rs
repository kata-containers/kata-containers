// SPDX-License-Identifier: MIT

use futures::stream::TryStreamExt;
use rtnetlink::{new_connection, Error, Handle};
use std::env;

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        usage();
        return Ok(());
    }
    let link_name = &args[1];
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    if let Err(e) = del_link(handle, link_name.to_string()).await {
        eprintln!("{}", e);
    }

    Ok(())
}

async fn del_link(handle: Handle, name: String) -> Result<(), Error> {
    let mut links = handle.link().get().match_name(name.clone()).execute();
    if let Some(link) = links.try_next().await? {
        handle.link().del(link.header.index).execute().await
    } else {
        eprintln!("link {} not found", name);
        Ok(())
    }
}

fn usage() {
    eprintln!(
        "usage:
    cargo run --example del_link -- <link name>

Note that you need to run this program as root. Instead of running cargo as root,
build the example normally:

    cd rtnetlink ; cargo build --example del_link

Then find the binary in the target directory:

    cd ../target/debug/example ; sudo ./del_link <link_name>"
    );
}
