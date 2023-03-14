// SPDX-License-Identifier: MIT

use futures::stream::TryStreamExt;
use rtnetlink::{new_connection, Error, Handle};

#[tokio::main]
async fn main() -> Result<(), ()> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    let link = "lo".to_string();
    println!("dumping address for link \"{}\"", link);

    if let Err(e) = dump_addresses(handle, link).await {
        eprintln!("{}", e);
    }

    Ok(())
}

async fn dump_addresses(handle: Handle, link: String) -> Result<(), Error> {
    let mut links = handle.link().get().match_name(link.clone()).execute();
    if let Some(link) = links.try_next().await? {
        let mut addresses = handle
            .address()
            .get()
            .set_link_index_filter(link.header.index)
            .execute();
        while let Some(msg) = addresses.try_next().await? {
            println!("{:?}", msg);
        }
        Ok(())
    } else {
        eprintln!("link {} not found", link);
        Ok(())
    }
}
