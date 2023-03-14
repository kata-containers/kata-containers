// SPDX-License-Identifier: MIT

use rtnetlink::new_connection;
use std::net::{Ipv4Addr, Ipv6Addr};

#[tokio::main]
async fn main() -> Result<(), String> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    handle
        .link()
        .add()
        .bond("my-bond".into())
        .mode(1)
        .miimon(100)
        .updelay(100)
        .downdelay(100)
        .min_links(2)
        .arp_ip_target(vec![Ipv4Addr::new(6, 6, 7, 7), Ipv4Addr::new(8, 8, 9, 10)])
        .ns_ip6_target(vec![
            Ipv6Addr::new(0xfd01, 0, 0, 0, 0, 0, 0, 1),
            Ipv6Addr::new(0xfd02, 0, 0, 0, 0, 0, 0, 2),
        ])
        .up()
        .execute()
        .await
        .map_err(|e| format!("{}", e))
}
