// Copyright (c) 2021 Kata Maintainers
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use futures::{future, StreamExt, TryStreamExt};
use ipnetwork::{IpNetwork, Ipv4Network, Ipv6Network};
use protobuf::RepeatedField;
use protocols::types::{ARPNeighbor, IPAddress, IPFamily, Interface, Route};
use rtnetlink::{new_connection, packet, IpVersion};
use std::convert::{TryFrom, TryInto};
use std::fmt;
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
        // by netlink, we may have to dump link list and the find the
        // target link. filter using name or family is supported, but
        // we cannot use that to find target link.
        // let's try if hardware address filter works. -_-
        let link = self.find_link(LinkFilter::Address(&iface.hwAddr)).await?;

        // Bring down interface if it is UP
        if link.is_up() {
            self.enable_link(link.index(), false).await?;
        }

        // Delete all addresses associated with the link
        let addresses = self
            .list_addresses(AddressFilter::LinkIndex(link.index()))
            .await?;
        self.delete_addresses(addresses).await?;

        // Add new ip addresses from request
        for ip_address in &iface.IPAddresses {
            let ip = IpAddr::from_str(&ip_address.get_address())?;
            let mask = u8::from_str_radix(ip_address.get_mask(), 10)?;

            self.add_addresses(link.index(), std::iter::once(IpNetwork::new(ip, mask)?))
                .await?;
        }

        // Update link
        let mut request = self.handle.link().set(link.index());
        request.message_mut().header = link.header.clone();

        request
            .mtu(iface.mtu as _)
            .name(iface.name.clone())
            .arp(iface.raw_flags & libc::IFF_NOARP as u32 == 0)
            .up()
            .execute()
            .await?;

        Ok(())
    }

    pub async fn handle_localhost(&self) -> Result<()> {
        let link = self.find_link(LinkFilter::Name("lo")).await?;
        self.enable_link(link.index(), true).await?;
        Ok(())
    }

    pub async fn update_routes<I>(&mut self, list: I) -> Result<()>
    where
        I: IntoIterator<Item = Route>,
    {
        let old_routes = self
            .query_routes(None)
            .await
            .with_context(|| "Failed to query old routes")?;

        self.delete_routes(old_routes)
            .await
            .with_context(|| "Failed to delete old routes")?;

        self.add_routes(list)
            .await
            .with_context(|| "Failed to add new routes")?;

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

            iface.IPAddresses = RepeatedField::from_vec(ips);

            list.push(iface);
        }

        Ok(list)
    }

    async fn find_link(&self, filter: LinkFilter<'_>) -> Result<Link> {
        let request = self.handle.link().get();

        let filtered = match filter {
            LinkFilter::Name(name) => request.set_name_filter(name.to_owned()),
            LinkFilter::Index(index) => request.match_index(index),
            _ => request, // Post filters
        };

        let mut stream = filtered.execute();

        let next = if let LinkFilter::Address(addr) = filter {
            use packet::link::nlas::Nla;

            let mac_addr = parse_mac_address(addr)
                .with_context(|| format!("Failed to parse MAC address: {}", addr))?;

            // Hardware filter might not be supported by netlink,
            // we may have to dump link list and the find the target link.
            stream
                .try_filter(|f| {
                    let result = f.nlas.iter().any(|n| match n {
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

    async fn query_routes(
        &self,
        ip_version: Option<IpVersion>,
    ) -> Result<Vec<packet::RouteMessage>> {
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
            if msg.header.table != packet::constants::RT_TABLE_MAIN {
                continue;
            }

            let mut route = Route {
                scope: msg.header.scope as _,
                ..Default::default()
            };

            if let Some((ip, mask)) = msg.destination_prefix() {
                route.dest = format!("{}/{}", ip, mask);
            }

            if let Some((ip, mask)) = msg.source_prefix() {
                route.source = format!("{}/{}", ip, mask);
            }

            if let Some(addr) = msg.gateway() {
                route.gateway = addr.to_string();

                // For gateway, destination is 0.0.0.0
                route.dest = if addr.is_ipv4() {
                    String::from("0.0.0.0")
                } else {
                    String::from("::1")
                }
            }

            if let Some(index) = msg.output_interface() {
                route.device = self.find_link(LinkFilter::Index(index)).await?.name();
            }

            result.push(route);
        }

        Ok(result)
    }

    /// Adds a list of routes from iterable object `I`.
    /// It can accept both a collection of routes or a single item (via `iter::once()`).
    /// It'll also take care of proper order when adding routes (gateways first, everything else after).
    async fn add_routes<I>(&mut self, list: I) -> Result<()>
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
            let is_v6 = is_ipv6(route.get_gateway()) || is_ipv6(route.get_dest());

            const MAIN_TABLE: u8 = packet::constants::RT_TABLE_MAIN;
            const UNICAST: u8 = packet::constants::RTN_UNICAST;
            const BOOT_PROT: u8 = packet::constants::RTPROT_BOOT;

            let scope = route.scope as u8;

            use packet::nlas::route::Nla;

            // Build a common indeterminate ip request
            let request = self
                .handle
                .route()
                .add()
                .table(MAIN_TABLE)
                .kind(UNICAST)
                .protocol(BOOT_PROT)
                .scope(scope);

            // `rtnetlink` offers a separate request builders for different IP versions (IP v4 and v6).
            // This if branch is a bit clumsy because it does almost the same.
            if is_v6 {
                let dest_addr = if !route.dest.is_empty() {
                    Ipv6Network::from_str(&route.dest)?
                } else {
                    Ipv6Network::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 0)?
                };

                // Build IP v6 request
                let mut request = request
                    .v6()
                    .destination_prefix(dest_addr.ip(), dest_addr.prefix())
                    .output_interface(link.index());

                if !route.source.is_empty() {
                    let network = Ipv6Network::from_str(&route.source)?;
                    if network.prefix() > 0 {
                        request = request.source_prefix(network.ip(), network.prefix());
                    } else {
                        request
                            .message_mut()
                            .nlas
                            .push(Nla::PrefSource(network.ip().octets().to_vec()));
                    }
                }

                if !route.gateway.is_empty() {
                    let ip = Ipv6Addr::from_str(&route.gateway)?;
                    request = request.gateway(ip);
                }

                request.execute().await.with_context(|| {
                    format!(
                        "Failed to add IP v6 route (src: {}, dst: {}, gtw: {})",
                        route.get_source(),
                        route.get_dest(),
                        route.get_gateway()
                    )
                })?;
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
                    .output_interface(link.index());

                if !route.source.is_empty() {
                    let network = Ipv4Network::from_str(&route.source)?;
                    if network.prefix() > 0 {
                        request = request.source_prefix(network.ip(), network.prefix());
                    } else {
                        request
                            .message_mut()
                            .nlas
                            .push(Nla::PrefSource(network.ip().octets().to_vec()));
                    }
                }

                if !route.gateway.is_empty() {
                    let ip = Ipv4Addr::from_str(&route.gateway)?;
                    request = request.gateway(ip);
                }

                request.execute().await?;
            }
        }

        Ok(())
    }

    async fn delete_routes<I>(&mut self, routes: I) -> Result<()>
    where
        I: IntoIterator<Item = packet::RouteMessage>,
    {
        for route in routes.into_iter() {
            if route.header.protocol == packet::constants::RTPROT_KERNEL {
                continue;
            }

            let index = if let Some(index) = route.output_interface() {
                index
            } else {
                continue;
            };

            let link = self.find_link(LinkFilter::Index(index)).await?;

            let name = link.name();
            if name.contains("lo") || name.contains("::1") {
                continue;
            }

            self.handle.route().del(route).execute().await?;
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

    async fn add_addresses<I>(&mut self, index: u32, list: I) -> Result<()>
    where
        I: IntoIterator<Item = IpNetwork>,
    {
        for net in list.into_iter() {
            self.handle
                .address()
                .add(index, net.ip(), net.prefix())
                .execute()
                .await
                .map_err(|err| anyhow!("Failed to add address {}: {:?}", net.ip(), err))?;
        }

        Ok(())
    }

    async fn delete_addresses<I>(&mut self, list: I) -> Result<()>
    where
        I: IntoIterator<Item = Address>,
    {
        for addr in list.into_iter() {
            self.handle.address().del(addr.0).execute().await?;
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
                    neigh.get_toIPAddress().get_address(),
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
            .ok_or(nix::Error::Sys(nix::errno::Errno::EINVAL))?;

        let ip = IpAddr::from_str(&ip_address)
            .map_err(|e| anyhow!("Failed to parse IP {}: {:?}", ip_address, e))?;

        // Import rtnetlink objects that make sense only for this function
        use packet::constants::{NDA_UNSPEC, NLM_F_ACK, NLM_F_CREATE, NLM_F_EXCL, NLM_F_REQUEST};
        use packet::neighbour::{NeighbourHeader, NeighbourMessage};
        use packet::nlas::neighbour::Nla;
        use packet::{NetlinkMessage, NetlinkPayload, RtnlMessage};
        use rtnetlink::Error;

        const IFA_F_PERMANENT: u16 = 0x80; // See https://github.com/little-dude/netlink/blob/0185b2952505e271805902bf175fee6ea86c42b8/netlink-packet-route/src/rtnl/constants.rs#L770

        let link = self.find_link(LinkFilter::Name(&neigh.device)).await?;

        let message = NeighbourMessage {
            header: NeighbourHeader {
                family: match ip {
                    IpAddr::V4(_) => packet::AF_INET,
                    IpAddr::V6(_) => packet::AF_INET6,
                } as u8,
                ifindex: link.index(),
                state: if neigh.state != 0 {
                    neigh.state as u16
                } else {
                    IFA_F_PERMANENT
                },
                flags: neigh.flags as u8,
                ntype: NDA_UNSPEC as u8,
            },
            nlas: {
                let mut nlas = vec![Nla::Destination(match ip {
                    IpAddr::V4(v4) => v4.octets().to_vec(),
                    IpAddr::V6(v6) => v6.octets().to_vec(),
                })];

                if !neigh.lladdr.is_empty() {
                    nlas.push(Nla::LinkLocalAddress(
                        parse_mac_address(&neigh.lladdr)?.to_vec(),
                    ));
                }

                nlas
            },
        };

        // Send request and ACK
        let mut req = NetlinkMessage::from(RtnlMessage::NewNeighbour(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK | NLM_F_EXCL | NLM_F_CREATE;

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

fn is_ipv6(str: &str) -> bool {
    Ipv6Addr::from_str(str).is_ok()
}

fn parse_mac_address(addr: &str) -> Result<[u8; 6]> {
    let mut split = addr.splitn(6, ':');

    // Parse single Mac address block
    let mut parse_next = || -> Result<u8> {
        let v = u8::from_str_radix(
            split
                .next()
                .ok_or(nix::Error::Sys(nix::errno::Errno::EINVAL))?,
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
struct Link(packet::LinkMessage);

impl Link {
    /// If name.
    fn name(&self) -> String {
        use packet::nlas::link::Nla;
        self.nlas
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
        use packet::nlas::link::Nla;
        self.nlas
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
        self.header.flags & packet::rtnl::constants::IFF_UP > 0
    }

    fn index(&self) -> u32 {
        self.header.index
    }

    fn mtu(&self) -> Option<u64> {
        use packet::nlas::link::Nla;
        self.nlas.iter().find_map(|n| {
            if let Nla::Mtu(mtu) = n {
                Some(*mtu as u64)
            } else {
                None
            }
        })
    }
}

impl From<packet::LinkMessage> for Link {
    fn from(msg: packet::LinkMessage) -> Self {
        Link(msg)
    }
}

impl Deref for Link {
    type Target = packet::LinkMessage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct Address(packet::AddressMessage);

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
            family,
            address,
            mask,
            ..Default::default()
        })
    }
}

impl Address {
    fn is_ipv6(&self) -> bool {
        self.0.header.family == packet::constants::AF_INET6 as u8
    }

    #[allow(dead_code)]
    fn prefix(&self) -> u8 {
        self.0.header.prefix_len
    }

    fn address(&self) -> String {
        use packet::nlas::address::Nla;
        self.0
            .nlas
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

    fn local(&self) -> String {
        use packet::nlas::address::Nla;
        self.0
            .nlas
            .iter()
            .find_map(|n| {
                if let Nla::Local(data) = n {
                    format_address(data).ok()
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skip_if_not_root;
    use rtnetlink::packet;
    use std::iter;
    use std::process::Command;

    #[tokio::test]
    async fn find_link_by_name() {
        let message = Handle::new()
            .expect("Failed to create netlink handle")
            .find_link(LinkFilter::Name("lo"))
            .await
            .expect("Loopback not found");

        assert_ne!(message.header, packet::LinkHeader::default());
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
            assert_ne!(addr.0.header, packet::AddressHeader::default());
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
    async fn add_delete_addresses() {
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

            // Delete it
            handle
                .delete_addresses(iter::once(result.unwrap()))
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

    #[test]
    fn check_ipv6() {
        assert!(is_ipv6("::1"));
        assert!(is_ipv6("2001:0:3238:DFE1:63::FEFB"));

        assert!(!is_ipv6(""));
        assert!(!is_ipv6("127.0.0.1"));
        assert!(!is_ipv6("10.10.10.10"));
    }

    fn clean_env_for_test_add_one_arp_neighbor(dummy_name: &str, ip: &str) {
        // ip link delete dummy
        Command::new("ip")
            .args(&["link", "delete", dummy_name])
            .output()
            .expect("prepare: failed to delete dummy");

        // ip neigh del dev dummy ip
        Command::new("ip")
            .args(&["neigh", "del", dummy_name, ip])
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
            .args(&["link", "add", dummy_name, "type", "dummy"])
            .output()
            .expect("failed to add dummy interface");

        // ip addr add 192.168.0.2/16 dev dummy
        Command::new("ip")
            .args(&["addr", "add", "192.168.0.2/16", "dev", dummy_name])
            .output()
            .expect("failed to add ip for dummy");

        // ip link set dummy up;
        Command::new("ip")
            .args(&["link", "set", dummy_name, "up"])
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
            .args(&["neigh", "show", "dev", dummy_name, to_ip])
            .output()
            .expect("failed to show neigh")
            .stdout;

        let stdout = std::str::from_utf8(&stdout).expect("failed to conveert stdout");
        assert_eq!(stdout, format!("{} lladdr {} PERMANENT\n", to_ip, mac));

        clean_env_for_test_add_one_arp_neighbor(dummy_name, to_ip);
    }
}
