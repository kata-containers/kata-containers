// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    convert::TryFrom,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::Arc,
};

use agent::{ARPNeighbor, IPAddress, IPFamily, Interface, Route};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures::stream::TryStreamExt;
use netlink_packet_route::{
    self,
    neighbour::{NeighbourAddress, NeighbourAttribute, NeighbourMessage},
    route::{RouteAddress, RouteAttribute, RouteMessage, RouteMetric, RouteType},
};
use rtnetlink::{IpVersion, RouteMessageBuilder};
use tokio::sync::RwLock;

use super::NetworkInfo;
use crate::network::utils::{
    address::Address,
    link::{self, LinkAttrs},
};

#[derive(Debug)]
pub(crate) struct NetworkInfoFromLink {
    interface: Interface,
    device_path: Arc<RwLock<String>>,
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
            device_path: Arc::new(RwLock::new(String::new())),
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
            NeighbourAttribute::LinkLayerAddress(addr) => {
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

fn route_mtu(route_msg: &RouteMessage) -> u32 {
    route_msg
        .attributes
        .iter()
        .find_map(|nla| match nla {
            RouteAttribute::Metrics(metrics) => metrics.iter().find_map(|m| match m {
                RouteMetric::Mtu(mtu) => Some(*mtu),
                _ => None,
            }),
            _ => None,
        })
        .unwrap_or(0)
}

// The guest kernel recreates kernel routes when the agent adds the interface addresses,
// so they are normally skipped. Forward one only when a CNI attached an MTU to it and the
// agent will put it back in place: the agent installs main-table unicast routes with the
// default priority and replaces the guest route with the same prefix and priority.
fn forward_kernel_route(route_msg: &RouteMessage) -> bool {
    let table = route_msg
        .attributes
        .iter()
        .find_map(|nla| match nla {
            RouteAttribute::Table(table) => Some(*table),
            _ => None,
        })
        .unwrap_or(route_msg.header.table as u32);
    let priority = route_msg
        .attributes
        .iter()
        .find_map(|nla| match nla {
            RouteAttribute::Priority(priority) => Some(*priority),
            _ => None,
        })
        .unwrap_or(0);

    table == libc::RT_TABLE_MAIN as u32
        && route_msg.header.kind == RouteType::Unicast
        && priority == 0
        && route_mtu(route_msg) != 0
}

fn generate_route(name: &str, route_msg: &RouteMessage) -> Result<Option<Route>> {
    let is_kernel_route = u8::from(route_msg.header.protocol) == libc::RTPROT_KERNEL;
    if is_kernel_route && !forward_kernel_route(route_msg) {
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
                let dest_prefix_len = route_msg.header.destination_prefix_length;

                route.dest = format!("{}/{}", dest, dest_prefix_len);
            }
            RouteAttribute::Gateway(g) => {
                let dest = parse_route_addr(g)?;

                route.gateway = dest.to_string();
            }
            RouteAttribute::Source(s) => {
                let dest = parse_route_addr(s)?;

                route.source = dest.to_string();
            }
            // Keep the preferred source of a forwarded kernel route; the agent installs
            // a source with a /0 prefix as the preferred source address.
            RouteAttribute::PrefSource(s) if is_kernel_route => {
                route.source = format!("{}/0", parse_route_addr(s)?);
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
        let mut iface = self.interface.clone();
        iface.device_path = self.device_path.read().await.clone();
        Ok(iface)
    }

    async fn routes(&self) -> Result<Vec<Route>> {
        Ok(self.routes.clone())
    }

    async fn neighs(&self) -> Result<Vec<ARPNeighbor>> {
        Ok(self.neighs.clone())
    }

    async fn set_device_path(&self, path: String) -> Result<()> {
        *self.device_path.write().await = path;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use netlink_packet_route::route::{RouteProtocol, RouteScope};

    #[test]
    fn generate_route_preserves_destination_prefix_length() {
        let route_msg = RouteMessageBuilder::<Ipv4Addr>::new()
            .destination_prefix(Ipv4Addr::new(10, 244, 0, 0), 16)
            .build();

        let route = generate_route("eth0", &route_msg)
            .expect("route should be valid")
            .expect("route should not be filtered");

        assert_eq!(route.dest, "10.244.0.0/16");
    }

    fn kernel_link_route(mtu: Option<u32>) -> RouteMessage {
        let mut route_msg = RouteMessageBuilder::<Ipv4Addr>::new()
            .destination_prefix(Ipv4Addr::new(10, 0, 0, 0), 23)
            .protocol(RouteProtocol::Kernel)
            .scope(RouteScope::Link)
            .build();
        route_msg
            .attributes
            .push(RouteAttribute::PrefSource(RouteAddress::Inet(
                Ipv4Addr::new(10, 0, 0, 20),
            )));
        if let Some(mtu) = mtu {
            route_msg
                .attributes
                .push(RouteAttribute::Metrics(vec![RouteMetric::Mtu(mtu)]));
        }
        route_msg
    }

    fn is_skipped(route_msg: &RouteMessage) -> bool {
        generate_route("eth0", route_msg)
            .expect("route should be valid")
            .is_none()
    }

    #[test]
    fn generate_route_skips_kernel_route_without_mtu() {
        assert!(is_skipped(&kernel_link_route(None)));
    }

    #[test]
    fn generate_route_keeps_kernel_route_with_mtu() {
        let route = generate_route("eth0", &kernel_link_route(Some(8700)))
            .expect("route should be valid")
            .expect("kernel route with an MTU should not be filtered");

        assert_eq!(route.dest, "10.0.0.0/23");
        assert_eq!(route.mtu, 8700);
        assert_eq!(route.scope, libc::RT_SCOPE_LINK as u32);
        assert_eq!(route.source, "10.0.0.20/0");
    }

    #[test]
    fn generate_route_skips_kernel_route_outside_main_table() {
        let mut route_msg = kernel_link_route(Some(8700));
        route_msg.header.table = libc::RT_TABLE_LOCAL;
        assert!(is_skipped(&route_msg));

        let mut route_msg = kernel_link_route(Some(8700));
        route_msg.attributes.push(RouteAttribute::Table(1000));
        assert!(is_skipped(&route_msg));
    }

    #[test]
    fn generate_route_skips_non_unicast_kernel_route() {
        let mut route_msg = kernel_link_route(Some(8700));
        route_msg.header.kind = RouteType::Local;
        assert!(is_skipped(&route_msg));
    }

    #[test]
    fn generate_route_skips_kernel_route_with_priority() {
        let mut route_msg = kernel_link_route(Some(8700));
        route_msg.attributes.push(RouteAttribute::Priority(256));
        assert!(is_skipped(&route_msg));
    }

    #[test]
    fn generate_route_ignores_preferred_source_of_other_routes() {
        let mut route_msg = kernel_link_route(Some(8700));
        route_msg.header.protocol = RouteProtocol::Boot;

        let route = generate_route("eth0", &route_msg)
            .expect("route should be valid")
            .expect("route should not be filtered");

        assert!(route.source.is_empty());
    }
}
