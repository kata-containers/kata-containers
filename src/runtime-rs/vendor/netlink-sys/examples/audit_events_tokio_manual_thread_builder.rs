// SPDX-License-Identifier: MIT

/*
 * This example shows the mimimal manual tokio initialization required to be able
 * to use netlink.
 */

use netlink_sys::{protocols::NETLINK_AUDIT, AsyncSocket, TokioSocket};

fn main() -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .build()
        .unwrap();

    let future = async {
        TokioSocket::new(NETLINK_AUDIT).unwrap();
    };
    rt.handle().block_on(future);
    Ok(())
}
