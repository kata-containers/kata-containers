// SPDX-License-Identifier: MIT

use futures::stream::TryStreamExt;
use rtnetlink::{
    new_connection,
    packet::{
        rtnl::link::nlas::{Nla, Prop},
        LinkMessage,
    },
    Error,
    Handle,
};
use std::env;

#[tokio::main]
async fn main() -> Result<(), ()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        usage();
        return Ok(());
    }

    let link_name = &args[1];
    let action = &args[2];
    let alt_ifnames = &args[3..].iter().map(String::as_str).collect();

    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    match action.as_str() {
        "add" => {
            if let Err(e) = add_property_alt_ifnames(link_name, alt_ifnames, handle.clone()).await {
                eprintln!("{}", e);
            }
        }

        "del" => {
            if let Err(e) = del_property_alt_ifnames(link_name, alt_ifnames, handle.clone()).await {
                eprintln!("{}", e);
            }
        }

        "show" => {
            if let Err(e) = show_property_alt_ifnames(link_name, handle.clone()).await {
                eprintln!("{}", e);
            }
        }

        _ => panic!("Unknown action {:?}", action),
    }

    Ok(())
}

async fn show_property_alt_ifnames(link_name: &str, handle: Handle) -> Result<(), Error> {
    for nla in get_link(link_name, handle).await?.nlas.into_iter() {
        if let Nla::PropList(ref prop_list) = nla {
            for prop in prop_list {
                if let Prop::AltIfName(altname) = prop {
                    println!("altname: {}", altname);
                }
            }
        }
    }

    Ok(())
}

async fn add_property_alt_ifnames(
    link_name: &str,
    alt_ifnames: &Vec<&str>,
    handle: Handle,
) -> Result<(), Error> {
    let link_index = get_link_index(link_name, handle.clone()).await?;

    handle
        .link()
        .property_add(link_index)
        .alt_ifname(alt_ifnames)
        .execute()
        .await?;

    Ok(())
}

async fn del_property_alt_ifnames(
    link_name: &str,
    alt_ifnames: &Vec<&str>,
    handle: Handle,
) -> Result<(), Error> {
    let link_index = get_link_index(link_name, handle.clone()).await?;

    handle
        .link()
        .property_del(link_index)
        .alt_ifname(alt_ifnames)
        .execute()
        .await?;

    Ok(())
}

async fn get_link(link_name: &str, handle: Handle) -> Result<LinkMessage, Error> {
    let mut links = handle
        .link()
        .get()
        .match_name(link_name.to_string())
        .execute();

    match links.try_next().await? {
        Some(msg) => Ok(msg),
        _ => {
            eprintln!("Interface {} not found", link_name);
            Err(Error::RequestFailed)
        }
    }
}

async fn get_link_index(link_name: &str, handle: Handle) -> Result<u32, Error> {
    Ok(get_link(link_name, handle.clone()).await?.header.index)
}

fn usage() {
    eprintln!(
        "usage:
    cargo run --example property_altname -- <link_name> [add | del | show] ALTNAME [ALTNAME ...]

Note that you need to run this program as root for add and del. Instead of running cargo as root,
build the example normally:

    cd rtnetlink ; cargo build --example property_altname

Then find the binary in the target directory:

    cd ../target/debug/example ; sudo ./property_altname <link_name> <ip_address>"
    );
}
