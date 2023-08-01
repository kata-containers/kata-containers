// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryFrom;

use agent::{ARPNeighbor, IPAddress, IPFamily, Interface, Route};
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::stream::TryStreamExt;
use netlink_packet_route::{
    self, neighbour::NeighbourMessage, nlas::neighbour::Nla, route::RouteMessage,
};

use super::NetworkInfo;
use crate::network::utils::{
    address::{parse_ip, Address},
    link::{self, LinkAttrs},
};

#[derive(Debug)]
pub(crate) struct NetworkInfoFromLink {
    interface: Interface,
    neighs: Vec<ARPNeighbor>,
    routes: Vec<Route>,
}

impl NetworkInfoFromLink {
    pub async fn new(
        handle: &rtnetlink::Handle,
        link: &dyn link::Link,
        addrs: Vec<IPAddress>,
        hw_addr: &str,
    ) -> Result<Self> {
        let attrs = link.attrs();
        let name = &attrs.name;

        Ok(Self {
            interface: Interface {
                device: name.clone(),
                name: name.clone(),
                ip_addresses: addrs.clone(),
                mtu: attrs.mtu as u64,
                hw_addr: hw_addr.to_string(),
                pci_addr: Default::default(),
                field_type: link.r#type().to_string(),
                raw_flags: attrs.flags & libc::IFF_NOARP as u32,
            },
            neighs: handle_neighbors(handle, attrs)
                .await
                .context("handle neighbours")?,
            routes: handle_routes(handle, attrs)
                .await
                .context("handle routes")?,
        })
    }
}

pub async fn handle_addresses(
    handle: &rtnetlink::Handle,
    attrs: &LinkAttrs,
) -> Result<Vec<IPAddress>> {
    let mut addr_msg_list = handle
        .address()
        .get()
        .set_link_index_filter(attrs.index)
        .execute();

    let mut addresses = vec![];
    while let Some(addr_msg) = addr_msg_list
        .try_next()
        .await
        .context("try next address msg")?
    {
        let family = addr_msg.header.family as i32;
        if family != libc::AF_INET && family != libc::AF_INET6 {
            warn!(sl!(), "unsupported ip family {}", family);
            continue;
        }
        let a = Address::try_from(addr_msg).context("get addr from msg")?;
        if a.addr.is_loopback() {
            continue;
        }

        addresses.push(IPAddress {
            family: if a.addr.is_ipv4() {
                IPFamily::V4
            } else {
                IPFamily::V6
            },
            address: a.addr.to_string(),
            mask: a.perfix_len.to_string(),
        });
    }
    Ok(addresses)
}

fn generate_neigh(name: &str, n: &NeighbourMessage) -> Result<ARPNeighbor> {
    let mut neigh = ARPNeighbor {
        device: name.to_string(),
        state: n.header.state as i32,
        ..Default::default()
    };
    for nla in &n.nlas {
        match nla {
            Nla::Destination(addr) => {
                let dest = parse_ip(addr, n.header.family).context("parse ip")?;
                let addr = Some(IPAddress {
                    family: if dest.is_ipv4() {
                        IPFamily::V4
                    } else {
                        IPFamily::V6
                    },
                    address: dest.to_string(),
                    mask: "".to_string(),
                });
                neigh.to_ip_address = addr;
            }
            Nla::LinkLocalAddress(addr) => {
                if addr.len() < 6 {
                    continue;
                }
                let lladdr = format!(
                    "{:<02x}:{:<02x}:{:<02x}:{:<02x}:{:<02x}:{:<02x}",
                    addr[0], addr[1], addr[2], addr[3], addr[4], addr[5]
                );
                neigh.ll_addr = lladdr;
            }
            _ => {
                // skip the unused Nla
            }
        }
    }

    Ok(neigh)
}

async fn handle_neighbors(
    handle: &rtnetlink::Handle,
    attrs: &LinkAttrs,
) -> Result<Vec<ARPNeighbor>> {
    let name = &attrs.name;
    let mut neighs = vec![];
    let mut neigh_msg_list = handle.neighbours().get().execute();
    while let Some(neigh) = neigh_msg_list
        .try_next()
        .await
        .context("try next neigh msg")?
    {
        // get neigh filter with index
        if neigh.header.ifindex == attrs.index {
            neighs.push(generate_neigh(name, &neigh).context("generate neigh")?)
        }
    }
    Ok(neighs)
}

fn generate_route(name: &str, route: &RouteMessage) -> Result<Option<Route>> {
    if route.header.protocol == libc::RTPROT_KERNEL {
        return Ok(None);
    }

    Ok(Some(Route {
        dest: route
            .destination_prefix()
            .map(|(addr, prefix)| format!("{}/{}", addr, prefix))
            .unwrap_or_default(),
        gateway: route.gateway().map(|v| v.to_string()).unwrap_or_default(),
        device: name.to_string(),
        source: route
            .source_prefix()
            .map(|(addr, _)| addr.to_string())
            .unwrap_or_default(),
        scope: route.header.scope as u32,
        family: if route.header.address_family == libc::AF_INET as u8 {
            IPFamily::V4
        } else {
            IPFamily::V6
        },
    }))
}

async fn get_route_from_msg(
    routes: &mut Vec<Route>,
    handle: &rtnetlink::Handle,
    attrs: &LinkAttrs,
    ip_version: rtnetlink::IpVersion,
) -> Result<()> {
    let name = &attrs.name;
    let mut route_msg_list = handle.route().get(ip_version).execute();
    while let Some(route) = route_msg_list.try_next().await? {
        // get route filter with index
        if let Some(index) = route.output_interface() {
            if index == attrs.index {
                if let Some(route) = generate_route(name, &route).context("generate route")? {
                    routes.push(route);
                }
            }
        }
    }
    Ok(())
}

async fn handle_routes(handle: &rtnetlink::Handle, attrs: &LinkAttrs) -> Result<Vec<Route>> {
    let mut routes = vec![];
    get_route_from_msg(&mut routes, handle, attrs, rtnetlink::IpVersion::V4)
        .await
        .context("get ip v4 route")?;
    get_route_from_msg(&mut routes, handle, attrs, rtnetlink::IpVersion::V6)
        .await
        .context("get ip v6 route")?;
    Ok(routes)
}

#[async_trait]
impl NetworkInfo for NetworkInfoFromLink {
    async fn interface(&self) -> Result<Interface> {
        Ok(self.interface.clone())
    }

    async fn routes(&self) -> Result<Vec<Route>> {
        Ok(self.routes.clone())
    }

    async fn neighs(&self) -> Result<Vec<ARPNeighbor>> {
        Ok(self.neighs.clone())
    }
}
