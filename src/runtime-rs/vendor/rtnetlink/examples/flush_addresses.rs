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

    if let Err(e) = flush_addresses(handle, link_name.to_string()).await {
        eprintln!("{}", e);
    }

    Ok(())
}

async fn flush_addresses(handle: Handle, link: String) -> Result<(), Error> {
    let mut links = handle.link().get().match_name(link.clone()).execute();
    if let Some(link) = links.try_next().await? {
        // We should have received only one message
        assert!(links.try_next().await?.is_none());

        let mut addresses = handle
            .address()
            .get()
            .set_link_index_filter(link.header.index)
            .execute();
        while let Some(addr) = addresses.try_next().await? {
            handle.address().del(addr).execute().await?;
        }
        Ok(())
    } else {
        eprintln!("link {} not found", link);
        Ok(())
    }
}

fn usage() {
    eprintln!(
        "usage:
    cargo run --example flush_addresses -- <link_name>

Note that you need to run this program as root. Instead of running cargo as root,
build the example normally:

    cd rtnetlink ; cargo build --example flush_addresses

Then find the binary in the target directory:

    cd ../target/debug/example ; sudo ./flush_addresses <link_name>"
    );
}
