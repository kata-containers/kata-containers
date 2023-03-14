// SPDX-License-Identifier: MIT

use futures::stream::TryStreamExt;
use rtnetlink::{new_connection, Error, Handle, IpVersion};

#[tokio::main]
async fn main() -> Result<(), ()> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    println!("dumping routes for IPv4");
    if let Err(e) = dump_addresses(handle.clone(), IpVersion::V4).await {
        eprintln!("{}", e);
    }
    println!();

    println!("dumping routes for IPv6");
    if let Err(e) = dump_addresses(handle.clone(), IpVersion::V6).await {
        eprintln!("{}", e);
    }
    println!();

    Ok(())
}

async fn dump_addresses(handle: Handle, ip_version: IpVersion) -> Result<(), Error> {
    let mut routes = handle.route().get(ip_version).execute();
    while let Some(route) = routes.try_next().await? {
        println!("{:?}", route);
    }
    Ok(())
}
