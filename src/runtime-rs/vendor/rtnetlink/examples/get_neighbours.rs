// SPDX-License-Identifier: MIT

use futures::stream::TryStreamExt;
use rtnetlink::{new_connection, Error, Handle, IpVersion};

#[tokio::main]
async fn main() -> Result<(), ()> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    println!("dumping neighbours");
    if let Err(e) = dump_neighbours(handle.clone()).await {
        eprintln!("{}", e);
    }
    println!();

    Ok(())
}

async fn dump_neighbours(handle: Handle) -> Result<(), Error> {
    let mut neighbours = handle
        .neighbours()
        .get()
        .set_family(IpVersion::V4)
        .execute();
    while let Some(route) = neighbours.try_next().await? {
        println!("{:?}", route);
    }
    Ok(())
}
