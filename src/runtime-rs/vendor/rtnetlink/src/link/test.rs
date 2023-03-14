// SPDX-License-Identifier: MIT

use futures::stream::TryStreamExt;
use tokio::runtime::Runtime;

use crate::{
    new_connection,
    packet::rtnl::link::{
        nlas::{Info, InfoKind, Nla},
        LinkMessage,
    },
    Error,
    LinkHandle,
};

const IFACE_NAME: &str = "wg142"; // rand?

#[test]
fn create_get_delete_wg() {
    let rt = Runtime::new().unwrap();
    let handle = rt.block_on(_create_wg());
    assert!(handle.is_ok());
    let mut handle = handle.unwrap();
    let msg = rt.block_on(_get_wg(&mut handle));
    assert!(msg.is_ok());
    let msg = msg.unwrap();
    assert!(has_nla(
        &msg,
        &Nla::Info(vec![Info::Kind(InfoKind::Wireguard)])
    ));
    rt.block_on(_del_wg(&mut handle, msg.header.index)).unwrap();
}

fn has_nla(msg: &LinkMessage, nla: &Nla) -> bool {
    msg.nlas.iter().any(|x| x == nla)
}

async fn _create_wg() -> Result<LinkHandle, Error> {
    let (conn, handle, _) = new_connection().unwrap();
    tokio::spawn(conn);
    let link_handle = handle.link();
    let mut req = link_handle.add();
    let mutator = req.message_mut();
    let info = Nla::Info(vec![Info::Kind(InfoKind::Wireguard)]);
    mutator.nlas.push(info);
    mutator.nlas.push(Nla::IfName(IFACE_NAME.to_owned()));
    req.execute().await?;
    Ok(link_handle)
}

async fn _get_wg(handle: &mut LinkHandle) -> Result<LinkMessage, Error> {
    let mut links = handle.get().match_name(IFACE_NAME.to_owned()).execute();
    let msg = links.try_next().await?;
    msg.ok_or(Error::RequestFailed)
}

async fn _del_wg(handle: &mut LinkHandle, index: u32) -> Result<(), Error> {
    handle.del(index).execute().await
}
