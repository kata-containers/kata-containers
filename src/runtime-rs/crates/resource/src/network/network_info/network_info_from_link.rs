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
    route::{RouteAddress, RouteAttribute, RouteMessage, RouteMetric},
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

    /// Drops the neighbours collected from the pod netns, so none are handed to the guest.
    pub(crate) fn without_neighbours(mut self) -> Self {
        self.neighs.clear();
        self
    }

    /// Hands the guest each IPv6 address that is not link-local as a /128, plus a host route to
    /// each IPv6 gateway that is not link-local.
    ///
    /// l3forwarding answers the guest's Neighbour Solicitations only for its gateways, so the
    /// guest has to send everything through one. With the CNI's on-link prefix it would solicit
    /// its same-prefix peers directly and get no answer. Without the prefix the gateway is no
    /// longer on-link either, hence the host routes; a link-local gateway stays reachable through
    /// the guest's own fe80::/64.
    pub(crate) fn with_ipv6_host_addresses(mut self) -> Self {
        let is_global_v6 = |address: &str| {
            address
                .parse::<Ipv6Addr>()
                .is_ok_and(|address| !address.is_unicast_link_local())
        };
        for address in &mut self.interface.ip_addresses {
            if address.family == IPFamily::V6 && is_global_v6(&address.address) {
                address.mask = "128".to_string();
            }
        }
        let mut host_routes: Vec<Route> = Vec::new();
        for route in &self.routes {
            if route.family != IPFamily::V6 || !is_global_v6(&route.gateway) {
                continue;
            }
            let dest = format!("{}/128", route.gateway);
            let known = |r: &Route| r.dest == dest && r.device == route.device;
            if !self.routes.iter().any(known) && !host_routes.iter().any(known) {
                host_routes.push(Route {
                    dest,
                    device: route.device.clone(),
                    family: IPFamily::V6,
                    ..Default::default()
                });
            }
        }
        self.routes.extend(host_routes);
        self
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

    fn address(family: IPFamily, address: &str, mask: &str) -> IPAddress {
        IPAddress {
            family,
            address: address.to_string(),
            mask: mask.to_string(),
        }
    }

    fn route(family: IPFamily, dest: &str, gateway: &str) -> Route {
        Route {
            dest: dest.to_string(),
            gateway: gateway.to_string(),
            device: "eth0".to_string(),
            family,
            ..Default::default()
        }
    }

    fn network_info(ip_addresses: Vec<IPAddress>, routes: Vec<Route>) -> NetworkInfoFromLink {
        NetworkInfoFromLink {
            interface: Interface {
                name: "eth0".to_string(),
                ip_addresses,
                ..Default::default()
            },
            device_path: Default::default(),
            neighs: vec![],
            routes,
        }
    }

    #[test]
    fn ipv6_host_addresses_for_an_on_link_pod() {
        // A dual-stack pod as the bridge CNI sets it up.
        let info = network_info(
            vec![
                address(IPFamily::V4, "10.244.0.5", "24"),
                address(IPFamily::V6, "fd00:10:244::5", "64"),
                address(IPFamily::V6, "fe80::5", "64"),
            ],
            vec![
                route(IPFamily::V4, "", "10.244.0.1"),
                route(IPFamily::V6, "", "fd00:10:244::1"),
                route(IPFamily::V6, "fd00:10:96::/112", "fd00:10:244::1"),
            ],
        )
        .with_ipv6_host_addresses();

        // Only the IPv6 address that is not link-local becomes a /128.
        assert_eq!(
            info.interface.ip_addresses,
            vec![
                address(IPFamily::V4, "10.244.0.5", "24"),
                address(IPFamily::V6, "fd00:10:244::5", "128"),
                address(IPFamily::V6, "fe80::5", "64"),
            ]
        );
        // One host route for the gateway, however many routes use it, and none for IPv4.
        assert_eq!(
            info.routes,
            vec![
                route(IPFamily::V4, "", "10.244.0.1"),
                route(IPFamily::V6, "", "fd00:10:244::1"),
                route(IPFamily::V6, "fd00:10:96::/112", "fd00:10:244::1"),
                route(IPFamily::V6, "fd00:10:244::1/128", ""),
            ]
        );
    }

    #[test]
    fn ipv6_host_addresses_keep_a_link_local_gateway_and_existing_host_routes() {
        // The AWS VPC CNI: a /128, a link-local gateway and its host route.
        let vpc_cni_routes = vec![
            route(IPFamily::V6, "fe80::1/128", ""),
            route(IPFamily::V6, "", "fe80::1"),
        ];
        let info = network_info(
            vec![address(IPFamily::V6, "2a05:d014::5", "128")],
            vpc_cni_routes.clone(),
        )
        .with_ipv6_host_addresses();
        assert_eq!(info.routes, vpc_cni_routes);

        // A global gateway that already has its host route gets no second one.
        let routes = vec![
            route(IPFamily::V6, "2001:db8::1/128", ""),
            route(IPFamily::V6, "", "2001:db8::1"),
        ];
        let info = network_info(vec![], routes.clone()).with_ipv6_host_addresses();
        assert_eq!(info.routes, routes);
    }

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
}
