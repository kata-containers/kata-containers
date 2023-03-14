// SPDX-License-Identifier: MIT

use rtnetlink::new_connection;

#[tokio::main]
async fn main() -> Result<(), String> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);
    handle
        .link()
        .add()
        .veth("veth-rs-1".into(), "veth-rs-2".into())
        .execute()
        .await
        .map_err(|e| format!("{}", e))
}
