// Copyright (c) 2021 Kata Maintainers
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use futures::{future, StreamExt, TryStreamExt};
use ipnetwork::{IpNetwork, Ipv4Network, Ipv6Network};
use netlink_packet_route::link::{LinkAttribute, LinkMessage};
use netlink_packet_route::neighbour::{self, NeighbourFlag};
use netlink_packet_route::route::{RouteFlag, RouteHeader, RouteProtocol, RouteScope, RouteType};
use netlink_packet_route::{
    address::{AddressAttribute, AddressMessage},
    route::RouteMetric,
};
use netlink_packet_route::{
    neighbour::{NeighbourAddress, NeighbourAttribute, NeighbourState},
    route::{RouteAddress, RouteAttribute, RouteMessage},
    AddressFamily,
};
use nix::errno::Errno;
use protocols::types::{ARPNeighbor, IPAddress, IPFamily, Interface, Route};
use rtnetlink::{new_connection, IpVersion};
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ops::Deref;
use std::str::{self, FromStr};

/// Search criteria to use when looking for a link in `find_link`.
pub enum LinkFilter<'a> {
    /// Find by link name.
    Name(&'a str),
    /// Find by link index.
    Index(u32),
    /// Find by MAC address.
    Address(&'a str),
}

impl fmt::Display for LinkFilter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkFilter::Name(name) => write!(f, "Name: {}", name),
            LinkFilter::Index(idx) => write!(f, "Index: {}", idx),
            LinkFilter::Address(addr) => write!(f, "Address: {}", addr),
        }
    }
}

const ALL_RULE_FLAGS: [NeighbourFlag; 8] = [
    NeighbourFlag::Use,
    NeighbourFlag::Own,
    NeighbourFlag::Controller,
    NeighbourFlag::Proxy,
    NeighbourFlag::ExtLearned,
    NeighbourFlag::Offloaded,
    NeighbourFlag::Sticky,
    NeighbourFlag::Router,
];

const ALL_ROUTE_FLAGS: [RouteFlag; 16] = [
    RouteFlag::Dead,
    RouteFlag::Pervasive,
    RouteFlag::Onlink,
    RouteFlag::Offload,
    RouteFlag::Linkdown,
    RouteFlag::Unresolved,
    RouteFlag::Trap,
    RouteFlag::Notify,
    RouteFlag::Cloned,
    RouteFlag::Equalize,
    RouteFlag::Prefix,
    RouteFlag::LookupTable,
    RouteFlag::FibMatch,
    RouteFlag::RtOffload,
    RouteFlag::RtTrap,
    RouteFlag::OffloadFailed,
];

/// A filter to query addresses.
pub enum AddressFilter {
    /// Return addresses that belong to the given interface.
    LinkIndex(u32),
    /// Get addresses with the given prefix.
    #[allow(dead_code)]
    IpAddress(IpAddr),
}

/// A high level wrapper for netlink (and `rtnetlink` crate) for use by the Agent's RPC.
/// It is expected to be consumed by the `AgentService`, so it operates with protobuf
/// structures directly for convenience.
#[derive(Debug)]
pub struct Handle {
    handle: rtnetlink::Handle,
}

impl Handle {
    pub(crate) fn new() -> Result<Handle> {
        let (conn, handle, _) = new_connection()?;
        tokio::spawn(conn);

        Ok(Handle { handle })
    }

    pub async fn update_interface(&mut self, iface: &Interface) -> Result<()> {
        // The reliable way to find link is using hardware address
        // as filter. However, hardware filter might not be supported
        // by netlink, we may have to dump link list and then find the
        // target link. filter using name or family is supported, but
        // we cannot use that to find target link.
        // let's try if hardware address filter works. -_-
        let link = self.find_link(LinkFilter::Address(&iface.hwAddr)).await?;

        // Bring down interface if it is UP
        if link.is_up() {
            self.enable_link(link.index(), false).await?;
        }

        // Get whether the network stack has ipv6 enabled or disabled.
        let supports_ipv6_all = fs::read_to_string("/proc/sys/net/ipv6/conf/all/disable_ipv6")
            .map(|s| s.trim() == "0")
            .unwrap_or(false);
        let supports_ipv6_default =
            fs::read_to_string("/proc/sys/net/ipv6/conf/default/disable_ipv6")
                .map(|s| s.trim() == "0")
                .unwrap_or(false);
        let supports_ipv6 = supports_ipv6_default || supports_ipv6_all;

        // Add new ip addresses from request
        for ip_address in &iface.IPAddresses {
            let ip = IpAddr::from_str(ip_address.address())?;
            let mask = ip_address.mask().parse::<u8>()?;

            let net = IpNetwork::new(ip, mask)?;
            if !net.is_ipv4() && !supports_ipv6 {
                // If we're dealing with an ipv6 address, but the stack does not
                // support ipv6, skip adding it otherwise it will lead to an
                // error at the "CreatePodSandbox" time.
                continue;
            }

            self.add_addresses(link.index(), std::iter::once(net))
                .await?;
        }

        // we need to update the link's interface name, thus we should rename the existed link whose name
        // is the same with the link's request name, otherwise, it would update the link failed with the
        // name conflicted.
        let mut new_link = None;
        if link.name() != iface.name {
            if let Ok(link) = self.find_link(LinkFilter::Name(iface.name.as_str())).await {
                // Bring down interface if it is UP
                if link.is_up() {
                    self.enable_link(link.index(), false).await?;
                }

                // update the existing interface name with a temporary name, otherwise
                // it would failed to udpate this interface with an existing name.
                let mut request = self.handle.link().set(link.index());
                request.message_mut().header = link.header.clone();
                let link_name = link.name();
                let temp_name = link_name.clone() + "_temp";

                request
                    .name(temp_name.clone())
                    .execute()
                    .await
                    .map_err(|err| {
                        anyhow!(
                            "Failed to rename interface {} to {}with error: {}",
                            link_name,
                            temp_name,
                            err
                        )
                    })?;

                new_link = Some(link);
            }
        }

        // Update link
        let link = self.find_link(LinkFilter::Address(&iface.hwAddr)).await?;
        let mut request = self.handle.link().set(link.index());
        request.message_mut().header = link.header.clone();

        request
            .mtu(iface.mtu as _)
            .name(iface.name.clone())
            .arp(iface.raw_flags & libc::IFF_NOARP as u32 == 0)
            .up()
            .execute()
            .await
            .map_err(|err| {
                anyhow!(
                    "Failure in LinkSetRequest for interface {}: {}",
                    iface.name.as_str(),
                    err
                )
            })?;

        // swap the updated iface's name.
        if let Some(nlink) = new_link {
            let mut request = self.handle.link().set(nlink.index());
            request.message_mut().header = nlink.header.clone();

            request
                .name(link.name())
                .up()
                .execute()
                .await
                .map_err(|err| {
                    anyhow!(
                        "Error swapping back interface name {} to {}: {}",
                        nlink.name().as_str(),
                        link.name(),
                        err
                    )
                })?;
        }

        Ok(())
    }

    pub async fn handle_localhost(&self) -> Result<()> {
        let link = self.find_link(LinkFilter::Name("lo")).await?;
        self.enable_link(link.index(), true).await?;
        Ok(())
    }

    /// Retireve available network interfaces.
    pub async fn list_interfaces(&self) -> Result<Vec<Interface>> {
        let mut list = Vec::new();

        let links = self.list_links().await?;

        for link in &links {
            let mut iface = Interface {
                name: link.name(),
                hwAddr: link.address(),
                mtu: link.mtu().unwrap_or(0),
                ..Default::default()
            };

            let ips = self
                .list_addresses(AddressFilter::LinkIndex(link.index()))
                .await?
                .into_iter()
                .map(|p| p.try_into())
                .collect::<Result<Vec<IPAddress>>>()?;

            iface.IPAddresses = ips;

            list.push(iface);
        }

        Ok(list)
    }

    async fn find_link(&self, filter: LinkFilter<'_>) -> Result<Link> {
        let request = self.handle.link().get();

        let filtered = match filter {
            LinkFilter::Name(name) => request.match_name(name.to_owned()),
            LinkFilter::Index(index) => request.match_index(index),
            _ => request, // Post filters
        };

        let mut stream = filtered.execute();

        let next = if let LinkFilter::Address(addr) = filter {
            use LinkAttribute as Nla;

            let mac_addr = parse_mac_address(addr)
                .with_context(|| format!("Failed to parse MAC address: {}", addr))?;

            // Hardware filter might not be supported by netlink,
            // we may have to dump link list and then find the target link.
            stream
                .try_filter(|f| {
                    let result = f.attributes.iter().any(|n| match n {
                        Nla::Address(data) => data.eq(&mac_addr),
                        _ => false,
                    });

                    future::ready(result)
                })
                .try_next()
                .await?
        } else {
            stream.try_next().await?
        };

        next.map(|msg| msg.into())
            .ok_or_else(|| anyhow!("Link not found ({})", filter))
    }

    async fn list_links(&self) -> Result<Vec<Link>> {
        let result = self
            .handle
            .link()
            .get()
            .execute()
            .try_filter_map(|msg| future::ready(Ok(Some(msg.into())))) // Don't filter, just map
            .try_collect::<Vec<Link>>()
            .await?;
        Ok(result)
    }

    pub async fn enable_link(&self, link_index: u32, up: bool) -> Result<()> {
        let link_req = self.handle.link().set(link_index);
        let set_req = if up { link_req.up() } else { link_req.down() };
        set_req.execute().await?;
        Ok(())
    }

    async fn query_routes(&self, ip_version: Option<IpVersion>) -> Result<Vec<RouteMessage>> {
        let list = if let Some(ip_version) = ip_version {
            self.handle
                .route()
                .get(ip_version)
                .execute()
                .try_collect()
                .await?
        } else {
            // These queries must be executed sequentially, otherwise
            // it'll throw "Device or resource busy (os error 16)"
            let routes4 = self
                .handle
                .route()
                .get(IpVersion::V4)
                .execute()
                .try_collect::<Vec<_>>()
                .await
                .with_context(|| "Failed to query IP v4 routes")?;

            let routes6 = self
                .handle
                .route()
                .get(IpVersion::V6)
                .execute()
                .try_collect::<Vec<_>>()
                .await
                .with_context(|| "Failed to query IP v6 routes")?;

            [routes4, routes6].concat()
        };

        Ok(list)
    }

    pub async fn list_routes(&self) -> Result<Vec<Route>> {
        let mut result = Vec::new();

        for msg in self.query_routes(None).await? {
            // Ignore non-main tables
            if msg.header.table != RouteHeader::RT_TABLE_MAIN {
                continue;
            }

            let mut route = Route {
                scope: u8::from(msg.header.scope) as u32,
                ..Default::default()
            };

            for attribute in &msg.attributes {
                if let RouteAttribute::Destination(dest) = attribute {
                    if let Ok(dest) = parse_route_addr(dest) {
                        route.dest = format!("{}/{}", dest, msg.header.destination_prefix_length);
                    }
                }

                if let RouteAttribute::Source(src) = attribute {
                    if let Ok(src) = parse_route_addr(src) {
                        route.source = format!("{}/{}", src, msg.header.source_prefix_length)
                    }
                }

                if let RouteAttribute::Gateway(g) = attribute {
                    if let Ok(addr) = parse_route_addr(g) {
                        // For gateway, destination is 0.0.0.0
                        if addr.is_ipv4() {
                            route.dest = String::from("0.0.0.0");
                        } else {
                            route.dest = String::from("::1");
                        }
                    }

                    route.gateway = parse_route_addr(g)
                        .map(|g| g.to_string())
                        .unwrap_or_default();
                }

                if let RouteAttribute::Metrics(metrics) = attribute {
                    for m in metrics {
                        if let RouteMetric::Mtu(mtu) = m {
                            route.mtu = *mtu;
                            break;
                        }
                    }
                }

                if let RouteAttribute::Oif(index) = attribute {
                    route.device = self.find_link(LinkFilter::Index(*index)).await?.name();
                }
            }

            if !route.dest.is_empty() {
                result.push(route);
            }
        }

        Ok(result)
    }

    /// Add a list of routes from iterable object `I`.
    /// If the route existed, then replace it with the latest.
    /// It can accept both a collection of routes or a single item (via `iter::once()`).
    /// It'll also take care of proper order when adding routes (gateways first, everything else after).
    pub async fn update_routes<I>(&mut self, list: I) -> Result<()>
    where
        I: IntoIterator<Item = Route>,
    {
        // Split the list so we add routes with no gateway first.
        // Note: `partition_in_place` is a better fit here, since it reorders things inplace (instead of
        // allocating two separate collections), however it's not yet in stable Rust.
        let (a, b): (Vec<Route>, Vec<Route>) = list.into_iter().partition(|p| p.gateway.is_empty());
        let list = a.iter().chain(&b);

        for route in list {
            let link = self.find_link(LinkFilter::Name(&route.device)).await?;

            const MAIN_TABLE: u32 = libc::RT_TABLE_MAIN as u32;
            let uni_cast: RouteType = RouteType::from(libc::RTN_UNICAST);
            let boot_prot: RouteProtocol = RouteProtocol::from(libc::RTPROT_BOOT);

            let scope = RouteScope::from(route.scope as u8);

            use RouteAttribute as Nla;

            // Build a common indeterminate ip request
            let mut request = self
                .handle
                .route()
                .add()
                .table_id(MAIN_TABLE)
                .kind(uni_cast)
                .protocol(boot_prot)
                .scope(scope);

            let message = request.message_mut();

            // calculate the Flag vec from the u32 flags
            let mut got: u32 = 0;
            let mut flags = Vec::new();
            for flag in ALL_ROUTE_FLAGS {
                if (route.flags & (u32::from(flag))) > 0 {
                    flags.push(flag);
                    got += u32::from(flag);
                }
            }
            if got != route.flags {
                flags.push(RouteFlag::Other(route.flags - got));
            }

            message.header.flags = flags;

            if route.mtu != 0 {
                let route_metrics = vec![RouteMetric::Mtu(route.mtu)];
                message
                    .attributes
                    .push(RouteAttribute::Metrics(route_metrics));
            }

            // `rtnetlink` offers a separate request builders for different IP versions (IP v4 and v6).
            // This if branch is a bit clumsy because it does almost the same.
            if route.family() == IPFamily::v6 {
                let dest_addr = if !route.dest.is_empty() {
                    Ipv6Network::from_str(&route.dest)?
                } else {
                    Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 0)?
                };

                // Build IP v6 request
                let mut request = request
                    .v6()
                    .destination_prefix(dest_addr.ip(), dest_addr.prefix())
                    .output_interface(link.index())
                    .replace();

                if !route.source.is_empty() {
                    let network = Ipv6Network::from_str(&route.source)?;
                    if network.prefix() > 0 {
                        request = request.source_prefix(network.ip(), network.prefix());
                    } else {
                        request
                            .message_mut()
                            .attributes
                            .push(Nla::PrefSource(RouteAddress::from(network.ip())));
                    }
                }

                if !route.gateway.is_empty() {
                    let ip = Ipv6Addr::from_str(&route.gateway)?;
                    request = request.gateway(ip);
                }

                if let Err(rtnetlink::Error::NetlinkError(message)) = request.execute().await {
                    if let Some(code) = message.code {
                        if Errno::from_i32(code.get()) != Errno::EEXIST {
                            return Err(anyhow!(
                                "Failed to add IP v6 route (src: {}, dst: {}, gtw: {},Err: {})",
                                route.source(),
                                route.dest(),
                                route.gateway(),
                                message
                            ));
                        }
                    }
                }
            } else {
                let dest_addr = if !route.dest.is_empty() {
                    Ipv4Network::from_str(&route.dest)?
                } else {
                    Ipv4Network::new(Ipv4Addr::new(0, 0, 0, 0), 0)?
                };

                // Build IP v4 request
                let mut request = request
                    .v4()
                    .destination_prefix(dest_addr.ip(), dest_addr.prefix())
                    .output_interface(link.index())
                    .replace();

                if !route.source.is_empty() {
                    let network = Ipv4Network::from_str(&route.source)?;
                    if network.prefix() > 0 {
                        request = request.source_prefix(network.ip(), network.prefix());
                    } else {
                        request
                            .message_mut()
                            .attributes
                            .push(RouteAttribute::PrefSource(RouteAddress::from(network.ip())));
                    }
                }

                if !route.gateway.is_empty() {
                    let ip = Ipv4Addr::from_str(&route.gateway)?;
                    request = request.gateway(ip);
                }

                if let Err(rtnetlink::Error::NetlinkError(message)) = request.execute().await {
                    if let Some(code) = message.code {
                        if Errno::from_i32(code.get()) != Errno::EEXIST {
                            return Err(anyhow!(
                                "Failed to add IP v4 route (src: {}, dst: {}, gtw: {},Err: {})",
                                route.source(),
                                route.dest(),
                                route.gateway(),
                                message
                            ));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn list_addresses<F>(&self, filter: F) -> Result<Vec<Address>>
    where
        F: Into<Option<AddressFilter>>,
    {
        let mut request = self.handle.address().get();

        if let Some(filter) = filter.into() {
            request = match filter {
                AddressFilter::LinkIndex(index) => request.set_link_index_filter(index),
                AddressFilter::IpAddress(addr) => request.set_address_filter(addr),
            };
        };

        let list = request
            .execute()
            .try_filter_map(|msg| future::ready(Ok(Some(Address(msg))))) // Map message to `Address`
            .try_collect()
            .await?;
        Ok(list)
    }

    // add the addresses to the specified interface, if the addresses existed,
    // replace it with the latest one.
    async fn add_addresses<I>(&mut self, index: u32, list: I) -> Result<()>
    where
        I: IntoIterator<Item = IpNetwork>,
    {
        for net in list.into_iter() {
            self.handle
                .address()
                .add(index, net.ip(), net.prefix())
                .replace()
                .execute()
                .await
                .map_err(|err| anyhow!("Failed to add address {}: {:?}", net.ip(), err))?;
        }

        Ok(())
    }

    pub async fn add_arp_neighbors<I>(&mut self, list: I) -> Result<()>
    where
        I: IntoIterator<Item = ARPNeighbor>,
    {
        for neigh in list.into_iter() {
            self.add_arp_neighbor(&neigh).await.map_err(|err| {
                anyhow!(
                    "Failed to add ARP neighbor {}: {:?}",
                    neigh.toIPAddress().address(),
                    err
                )
            })?;
        }

        Ok(())
    }

    /// Adds an ARP neighbor.
    /// TODO: `rtnetlink` has no neighbours API, remove this after https://github.com/little-dude/netlink/pull/135
    async fn add_arp_neighbor(&mut self, neigh: &ARPNeighbor) -> Result<()> {
        let ip_address = neigh
            .toIPAddress
            .as_ref()
            .map(|to| to.address.as_str()) // Extract address field
            .and_then(|addr| if addr.is_empty() { None } else { Some(addr) }) // Make sure it's not empty
            .ok_or_else(|| anyhow!("Unable to determine ip address of ARP neighbor"))?;

        let ip = IpAddr::from_str(ip_address)
            .map_err(|e| anyhow!("Failed to parse IP {}: {:?}", ip_address, e))?;

        // Import rtnetlink objects that make sense only for this function
        use libc::{NDA_UNSPEC, NLM_F_ACK, NLM_F_CREATE, NLM_F_REPLACE, NLM_F_REQUEST};
        use neighbour::{NeighbourHeader, NeighbourMessage};
        use netlink_packet_core::{NetlinkMessage, NetlinkPayload};
        use netlink_packet_route::RouteNetlinkMessage as RtnlMessage;
        use rtnetlink::Error;

        const IFA_F_PERMANENT: u16 = 0x80; // See https://github.com/little-dude/netlink/blob/0185b2952505e271805902bf175fee6ea86c42b8/netlink-packet-route/src/rtnl/constants.rs#L770
        let state = if neigh.state != 0 {
            neigh.state as u16
        } else {
            IFA_F_PERMANENT
        };

        let link = self.find_link(LinkFilter::Name(&neigh.device)).await?;

        let mut flags = Vec::new();
        for flag in ALL_RULE_FLAGS {
            if (neigh.flags as u8 & (u8::from(flag))) > 0 {
                flags.push(flag);
            }
        }

        let mut message = NeighbourMessage::default();

        message.header = NeighbourHeader {
            family: match ip {
                IpAddr::V4(_) => AddressFamily::Inet,
                IpAddr::V6(_) => AddressFamily::Inet6,
            },
            ifindex: link.index(),
            state: NeighbourState::from(state),
            flags,
            kind: RouteType::from(NDA_UNSPEC as u8),
        };

        let mut nlas = vec![NeighbourAttribute::Destination(match ip {
            IpAddr::V4(ipv4_addr) => NeighbourAddress::from(ipv4_addr),
            IpAddr::V6(ipv6_addr) => NeighbourAddress::from(ipv6_addr),
        })];

        if !neigh.lladdr.is_empty() {
            nlas.push(NeighbourAttribute::LinkLocalAddress(
                parse_mac_address(&neigh.lladdr)?.to_vec(),
            ));
        }

        message.attributes = nlas;

        // Send request and ACK
        let mut req = NetlinkMessage::from(RtnlMessage::NewNeighbour(message));
        req.header.flags = (NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_REPLACE) as u16;

        let mut response = self.handle.request(req)?;
        while let Some(message) = response.next().await {
            if let NetlinkPayload::Error(err) = message.payload {
                return Err(anyhow!(Error::NetlinkError(err)));
            }
        }

        Ok(())
    }
}

fn format_address(data: &[u8]) -> Result<String> {
    match data.len() {
        4 => {
            // IP v4
            Ok(format!("{}.{}.{}.{}", data[0], data[1], data[2], data[3]))
        }
        6 => {
            // Mac address
            Ok(format!(
                "{:0>2X}:{:0>2X}:{:0>2X}:{:0>2X}:{:0>2X}:{:0>2X}",
                data[0], data[1], data[2], data[3], data[4], data[5]
            ))
        }
        16 => {
            // IP v6
            let octets = <[u8; 16]>::try_from(data)?;
            Ok(Ipv6Addr::from(octets).to_string())
        }
        _ => Err(anyhow!("Unsupported address length: {}", data.len())),
    }
}

fn parse_mac_address(addr: &str) -> Result<[u8; 6]> {
    let mut split = addr.splitn(6, ':');

    // Parse single Mac address block
    let mut parse_next = || -> Result<u8> {
        let v = u8::from_str_radix(
            split
                .next()
                .ok_or_else(|| anyhow!("Invalid MAC address {}", addr))?,
            16,
        )?;
        Ok(v)
    };

    // Parse all 6 blocks
    let arr = [
        parse_next()?,
        parse_next()?,
        parse_next()?,
        parse_next()?,
        parse_next()?,
        parse_next()?,
    ];

    Ok(arr)
}

/// Wraps external type with the local one, so we can implement various extensions and type conversions.
struct Link(LinkMessage);

impl Link {
    /// If name.
    fn name(&self) -> String {
        use LinkAttribute as Nla;
        self.attributes
            .iter()
            .find_map(|n| {
                if let Nla::IfName(name) = n {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    /// Extract Mac address.
    fn address(&self) -> String {
        use LinkAttribute as Nla;
        self.attributes
            .iter()
            .find_map(|n| {
                if let Nla::Address(data) = n {
                    format_address(data).ok()
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    /// Returns whether the link is UP
    fn is_up(&self) -> bool {
        let mut flags: u32 = 0;
        for flag in &self.header.flags {
            flags += u32::from(*flag);
        }

        flags as i32 & libc::IFF_UP > 0
    }

    fn index(&self) -> u32 {
        self.header.index
    }

    fn mtu(&self) -> Option<u64> {
        use LinkAttribute as Nla;
        self.attributes.iter().find_map(|n| {
            if let Nla::Mtu(mtu) = n {
                Some(*mtu as u64)
            } else {
                None
            }
        })
    }
}

impl From<LinkMessage> for Link {
    fn from(msg: LinkMessage) -> Self {
        Link(msg)
    }
}

impl Deref for Link {
    type Target = LinkMessage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct Address(AddressMessage);

impl TryFrom<Address> for IPAddress {
    type Error = anyhow::Error;

    fn try_from(value: Address) -> Result<Self, Self::Error> {
        let family = if value.is_ipv6() {
            IPFamily::v4
        } else {
            IPFamily::v6
        };

        let mut address = value.address();
        if address.is_empty() {
            address = value.local();
        }

        let mask = format!("{}", value.0.header.prefix_len);

        Ok(IPAddress {
            family: family.into(),
            address,
            mask,
            ..Default::default()
        })
    }
}

impl Address {
    fn is_ipv6(&self) -> bool {
        u8::from(self.0.header.family) == libc::AF_INET6 as u8
    }

    #[allow(dead_code)]
    fn prefix(&self) -> u8 {
        self.0.header.prefix_len
    }

    fn address(&self) -> String {
        use AddressAttribute as Nla;
        self.0
            .attributes
            .iter()
            .find_map(|n| {
                if let Nla::Address(data) = n {
                    Some(data.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }

    fn local(&self) -> String {
        use AddressAttribute as Nla;
        self.0
            .attributes
            .iter()
            .find_map(|n| {
                if let Nla::Local(data) = n {
                    Some(data.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default()
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
    use netlink_packet_route::address::AddressHeader;
    use netlink_packet_route::link::LinkHeader;
    use std::iter;
    use std::process::Command;
    use test_utils::skip_if_not_root;

    #[tokio::test]
    async fn find_link_by_name() {
        let message = Handle::new()
            .expect("Failed to create netlink handle")
            .find_link(LinkFilter::Name("lo"))
            .await
            .expect("Loopback not found");

        assert_ne!(message.header, LinkHeader::default());
        assert_eq!(message.name(), "lo");
    }

    #[tokio::test]
    async fn find_link_by_addr() {
        let handle = Handle::new().unwrap();

        let list = handle.list_links().await.unwrap();
        let link = list.first().expect("At least one link required");

        let result = handle
            .find_link(LinkFilter::Address(&link.address()))
            .await
            .expect("Failed to query link by address");

        assert_eq!(result.header.index, link.header.index);
    }

    #[tokio::test]
    async fn link_up() {
        skip_if_not_root!();

        let handle = Handle::new().unwrap();
        let link = handle.find_link(LinkFilter::Name("lo")).await.unwrap();

        handle
            .enable_link(link.header.index, true)
            .await
            .expect("Failed to bring link up");

        assert!(handle
            .find_link(LinkFilter::Name("lo"))
            .await
            .unwrap()
            .is_up());
    }

    #[tokio::test]
    async fn link_ext() {
        let lo = Handle::new()
            .unwrap()
            .find_link(LinkFilter::Name("lo"))
            .await
            .unwrap();

        assert_eq!(lo.name(), "lo");
        assert_ne!(lo.address().len(), 0);
    }

    #[tokio::test]
    async fn list_routes() {
        let all = Handle::new()
            .unwrap()
            .list_routes()
            .await
            .expect("Failed to list routes");

        assert_ne!(all.len(), 0);

        for r in &all {
            assert_ne!(r.device.len(), 0);
        }
    }

    #[tokio::test]
    async fn list_addresses() {
        let list = Handle::new()
            .unwrap()
            .list_addresses(None)
            .await
            .expect("Failed to list addresses");

        assert_ne!(list.len(), 0);
        for addr in &list {
            assert_ne!(addr.0.header, AddressHeader::default());
        }
    }

    #[tokio::test]
    async fn list_interfaces() {
        let list = Handle::new()
            .unwrap()
            .list_interfaces()
            .await
            .expect("Failed to list interfaces");

        for iface in &list {
            assert_ne!(iface.name.len(), 0);
            assert_ne!(iface.mtu, 0);

            for ip in &iface.IPAddresses {
                assert_ne!(ip.mask.len(), 0);
                assert_ne!(ip.address.len(), 0);
            }
        }
    }

    #[tokio::test]
    async fn add_update_addresses() {
        skip_if_not_root!();

        let list = vec![
            IpNetwork::from_str("169.254.1.1/31").unwrap(),
            IpNetwork::from_str("2001:db8:85a3::8a2e:370:7334/128").unwrap(),
        ];

        let mut handle = Handle::new().unwrap();
        let lo = handle.find_link(LinkFilter::Name("lo")).await.unwrap();

        for network in list {
            handle
                .add_addresses(lo.index(), iter::once(network))
                .await
                .expect("Failed to add IP");

            // Make sure the address is there
            let result = handle
                .list_addresses(AddressFilter::LinkIndex(lo.index()))
                .await
                .unwrap()
                .into_iter()
                .find(|p| {
                    p.prefix() == network.prefix() && p.address() == network.ip().to_string()
                });

            assert!(result.is_some());

            // Update it
            handle
                .add_addresses(lo.index(), iter::once(network))
                .await
                .expect("Failed to delete address");
        }
    }

    #[test]
    fn format_addr() {
        let buf = [1u8, 2u8, 3u8, 4u8];
        let addr = format_address(&buf).unwrap();
        assert_eq!(addr, "1.2.3.4");

        let buf = [1u8, 2u8, 3u8, 4u8, 5u8, 10u8];
        let addr = format_address(&buf).unwrap();
        assert_eq!(addr, "01:02:03:04:05:0A");
    }

    #[test]
    fn parse_mac() {
        let bytes = parse_mac_address("AB:0C:DE:12:34:56").expect("Failed to parse mac address");
        assert_eq!(bytes, [0xAB, 0x0C, 0xDE, 0x12, 0x34, 0x56]);
    }

    fn clean_env_for_test_add_one_arp_neighbor(dummy_name: &str, ip: &str) {
        // ip link delete dummy
        Command::new("ip")
            .args(["link", "delete", dummy_name])
            .output()
            .expect("prepare: failed to delete dummy");

        // ip neigh del dev dummy ip
        Command::new("ip")
            .args(["neigh", "del", dummy_name, ip])
            .output()
            .expect("prepare: failed to delete neigh");
    }

    fn prepare_env_for_test_add_one_arp_neighbor(dummy_name: &str, ip: &str) {
        clean_env_for_test_add_one_arp_neighbor(dummy_name, ip);
        // modprobe dummy
        Command::new("modprobe")
            .arg("dummy")
            .output()
            .expect("failed to run modprobe dummy");

        // ip link add dummy type dummy
        Command::new("ip")
            .args(["link", "add", dummy_name, "type", "dummy"])
            .output()
            .expect("failed to add dummy interface");

        // ip addr add 192.168.0.2/16 dev dummy
        Command::new("ip")
            .args(["addr", "add", "192.168.0.2/16", "dev", dummy_name])
            .output()
            .expect("failed to add ip for dummy");

        // ip link set dummy up;
        Command::new("ip")
            .args(["link", "set", dummy_name, "up"])
            .output()
            .expect("failed to up dummy");
    }

    #[tokio::test]
    async fn test_add_one_arp_neighbor() {
        skip_if_not_root!();

        let mac = "6a:92:3a:59:70:aa";
        let to_ip = "169.254.1.1";
        let dummy_name = "dummy_for_arp";

        prepare_env_for_test_add_one_arp_neighbor(dummy_name, to_ip);

        let mut ip_address = IPAddress::new();
        ip_address.set_address(to_ip.to_string());

        let mut neigh = ARPNeighbor::new();
        neigh.set_toIPAddress(ip_address);
        neigh.set_device(dummy_name.to_string());
        neigh.set_lladdr(mac.to_string());
        neigh.set_state(0x80);

        Handle::new()
            .unwrap()
            .add_arp_neighbor(&neigh)
            .await
            .expect("Failed to add ARP neighbor");

        // ip neigh show dev dummy ip
        let stdout = Command::new("ip")
            .args(["neigh", "show", "dev", dummy_name, to_ip])
            .output()
            .expect("failed to show neigh")
            .stdout;

        let stdout = std::str::from_utf8(&stdout).expect("failed to convert stdout");
        assert_eq!(stdout.trim(), format!("{} lladdr {} PERMANENT", to_ip, mac));

        clean_env_for_test_add_one_arp_neighbor(dummy_name, to_ip);
    }
}
