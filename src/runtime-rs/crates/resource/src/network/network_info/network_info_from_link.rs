// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    convert::TryFrom,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use agent::{ARPNeighbor, IPAddress, IPFamily, Interface, Route};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures::stream::TryStreamExt;
use netlink_packet_route::{
    self,
    neighbour::{NeighbourAddress, NeighbourAttribute, NeighbourMessage},
    route::{RouteAddress, RouteAttribute, RouteMessage, RouteMetric},
};
use rtnetlink::{IpVersion, RouteMessageBuilder};

use super::NetworkInfo;
use crate::network::utils::{
    address::Address,
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
                device_path: Default::default(),
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
        let family = u8::from(addr_msg.header.family) as i32;
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
        state: u16::from(n.header.state) as i32,
        ..Default::default()
    };
    for nla in &n.attributes {
        match nla {
            NeighbourAttribute::Destination(addr) => {
                let dest = match addr {
                    NeighbourAddress::Inet6(ipv6_addr) => ipv6_addr.to_canonical(),
                    NeighbourAddress::Inet(ipv4_addr) => IpAddr::from(*ipv4_addr),
                    _ => return Err(anyhow!("invalid address")),
                };
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
            NeighbourAttribute::LinkLocalAddress(addr) => {
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

fn generate_route(name: &str, route_msg: &RouteMessage) -> Result<Option<Route>> {
    if u8::from(route_msg.header.protocol) == libc::RTPROT_KERNEL {
        return Ok(None);
    }

    let mut route = Route {
        scope: u8::from(route_msg.header.scope) as u32,
        device: name.to_string(),
        family: if u8::from(route_msg.header.address_family) == libc::AF_INET as u8 {
            IPFamily::V4
        } else {
            IPFamily::V6
        },
        flags: route_msg.header.flags.bits(),
        ..Default::default()
    };

    for nla in &route_msg.attributes {
        match nla {
            RouteAttribute::Destination(d) => {
                let dest = parse_route_addr(d)?;
                route.dest = dest.to_string();
            }
            RouteAttribute::Gateway(g) => {
                let dest = parse_route_addr(g)?;

                route.gateway = dest.to_string();
            }
            RouteAttribute::Source(s) => {
                let dest = parse_route_addr(s)?;

                route.source = dest.to_string();
            }
            RouteAttribute::Metrics(metrics) => {
                for m in metrics {
                    if let RouteMetric::Mtu(mtu) = m {
                        route.mtu = *mtu;
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Some(route))
}

async fn get_route_from_msg(
    routes: &mut Vec<Route>,
    handle: &rtnetlink::Handle,
    attrs: &LinkAttrs,
    ip_version: IpVersion,
) -> Result<()> {
    let name = &attrs.name;
    let route_message = match ip_version {
        IpVersion::V4 => RouteMessageBuilder::<Ipv4Addr>::new().build(),
        IpVersion::V6 => RouteMessageBuilder::<Ipv6Addr>::new().build(),
    };
    let mut route_msg_list = handle.route().get(route_message).execute();
    while let Some(route_msg) = route_msg_list.try_next().await? {
        // get route filter with index
        for attr in &route_msg.attributes {
            if let RouteAttribute::Oif(index) = attr {
                if *index == attrs.index {
                    if let Some(route) =
                        generate_route(name, &route_msg).context("generate route")?
                    {
                        routes.push(route);
                    }
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

fn parse_route_addr(ra: &RouteAddress) -> Result<IpAddr> {
    let ipaddr = match ra {
        RouteAddress::Inet6(ipv6_addr) => ipv6_addr.to_canonical(),
        RouteAddress::Inet(ipv4_addr) => IpAddr::from(*ipv4_addr),
        _ => return Err(anyhow!("got invalid route address")),
    };

    Ok(ipaddr)
}
