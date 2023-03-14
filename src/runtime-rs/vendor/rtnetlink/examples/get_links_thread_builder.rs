// SPDX-License-Identifier: MIT

use futures::stream::TryStreamExt;

use rtnetlink::{
    new_connection,
    packet::rtnl::{
        constants::{AF_BRIDGE, RTEXT_FILTER_BRVLAN},
        link::nlas::Nla,
    },
    Error,
    Handle,
};

async fn do_it(rt: &tokio::runtime::Runtime) -> Result<(), ()> {
    env_logger::init();
    let (connection, handle, _) = new_connection().unwrap();
    rt.spawn(connection);

    // Fetch a link by its index
    let index = 1;
    println!("*** retrieving link with index {} ***", index);
    if let Err(e) = get_link_by_index(handle.clone(), index).await {
        eprintln!("{}", e);
    }

    // Fetch a link by its name
    let name = "lo";
    println!("*** retrieving link named \"{}\" ***", name);
    if let Err(e) = get_link_by_name(handle.clone(), name.to_string()).await {
        eprintln!("{}", e);
    }

    // Dump all the links and print their index and name
    println!("*** dumping links ***");
    if let Err(e) = dump_links(handle.clone()).await {
        eprintln!("{}", e);
    }

    // Dump all the bridge vlan information
    if let Err(e) = dump_bridge_filter_info(handle.clone()).await {
        eprintln!("{}", e);
    }

    Ok(())
}

async fn get_link_by_index(handle: Handle, index: u32) -> Result<(), Error> {
    let mut links = handle.link().get().match_index(index).execute();
    let msg = if let Some(msg) = links.try_next().await? {
        msg
    } else {
        eprintln!("no link with index {} found", index);
        return Ok(());
    };
    // We should have received only one message
    assert!(links.try_next().await?.is_none());

    for nla in msg.nlas.into_iter() {
        if let Nla::IfName(name) = nla {
            println!("found link with index {} (name = {})", index, name);
            return Ok(());
        }
    }
    eprintln!(
        "found link with index {}, but this link does not have a name",
        index
    );
    Ok(())
}

async fn get_link_by_name(handle: Handle, name: String) -> Result<(), Error> {
    let mut links = handle.link().get().match_name(name.clone()).execute();
    if (links.try_next().await?).is_some() {
        println!("found link {}", name);
        // We should only have one link with that name
        assert!(links.try_next().await?.is_none());
    } else {
        println!("no link link {} found", name);
    }
    Ok(())
}

async fn dump_links(handle: Handle) -> Result<(), Error> {
    let mut links = handle.link().get().execute();
    'outer: while let Some(msg) = links.try_next().await? {
        for nla in msg.nlas.into_iter() {
            if let Nla::IfName(name) = nla {
                println!("found link {} ({})", msg.header.index, name);
                continue 'outer;
            }
        }
        eprintln!("found link {}, but the link has no name", msg.header.index);
    }
    Ok(())
}

async fn dump_bridge_filter_info(handle: Handle) -> Result<(), Error> {
    let mut links = handle
        .link()
        .get()
        .set_filter_mask(AF_BRIDGE as u8, RTEXT_FILTER_BRVLAN)
        .execute();
    'outer: while let Some(msg) = links.try_next().await? {
        for nla in msg.nlas.into_iter() {
            if let Nla::AfSpecBridge(data) = nla {
                println!(
                    "found interface {} with AfSpecBridge data {:?})",
                    msg.header.index, data
                );
                continue 'outer;
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .build()
        .unwrap();

    let future = do_it(&rt);
    println!("blocking in main");
    rt.handle().block_on(future).unwrap();
    Ok(())
}
