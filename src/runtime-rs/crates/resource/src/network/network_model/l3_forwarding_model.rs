// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::TryStreamExt;
use netlink_packet_route::address::AddressAttribute;
use netlink_packet_route::neighbour::NeighbourFlags;
use netlink_packet_route::route::{RouteAddress, RouteAttribute, RouteProtocol, RouteScope};
use rtnetlink::{Handle, RouteMessageBuilder};
use scopeguard::defer;

use super::{
    port_forwarding::{configure_port_forwarding, TAP_IPV4_ADDR},
    NetworkModel, NetworkModelType,
};
use crate::network::NetworkPair;

const ROUTE_TABLE_LOCAL: u8 = 255;
const IPV4_HOST_PREFIX: u8 = 32;
const IPV6_HOST_PREFIX: u8 = 128;

#[derive(Debug)]
pub(crate) struct L3ForwardingModel {}

impl L3ForwardingModel {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl NetworkModel for L3ForwardingModel {
    fn model_type(&self) -> NetworkModelType {
        NetworkModelType::L3Forwarding
    }

    async fn add(&self, pair: &NetworkPair) -> Result<()> {
        let (connection, handle, _) = rtnetlink::new_connection().context("new connection")?;
        let thread_handler = tokio::spawn(connection);

        defer!({
            thread_handler.abort();
        });

        let tap_index = fetch_index(&handle, pair.tap.tap_iface.name.as_str())
            .await
            .context("fetch tap by index")?;
        let virt_index = fetch_index(&handle, pair.virt_iface.name.as_str())
            .await
            .context("fetch virt by index")?;

        let (pod_v4_addrs, pod_v6_addrs) =
            workload_addresses(pair.virt_iface.addrs.iter().map(|a| (a.addr, a.perfix_len)))?;
        if pod_v4_addrs.is_empty() && pod_v6_addrs.is_empty() {
            anyhow::bail!("no IP addresses found on virt iface");
        }
        let pod_ipv4 = pod_v4_addrs.first().copied();

        // Check the IPv6 routes before configuring either family, so that a refusal leaves the
        // pod netns as the CNI set it up.
        let ipv6_gateways = if pod_v6_addrs.is_empty() {
            Vec::new()
        } else {
            check_ipv6_link_local(pair.virt_iface.addrs.iter().map(|a| a.addr))
                .context("check virt iface for an ipv6 link-local address")?;
            let gateways = route_gateways(&handle, virt_index)
                .await
                .context("collect ipv6 route gateways")?;
            check_ipv6_routes(&handle, virt_index, &gateways)
                .await
                .context("check ipv6 routes on virt iface")?;
            gateways
        };

        if !pod_v4_addrs.is_empty() {
            configure_ipv4(
                &handle,
                &pair.tap.tap_iface.name,
                &pair.virt_iface.name,
                tap_index,
                virt_index,
                &pod_v4_addrs,
            )
            .await
            .context("configure ipv4 l3 forwarding")?;
        }

        if !pod_v6_addrs.is_empty() {
            configure_ipv6(
                &handle,
                &pair.tap.tap_iface.name,
                &pair.virt_iface.name,
                tap_index,
                virt_index,
                &pod_v6_addrs,
                &ipv6_gateways,
            )
            .await
            .context("configure ipv6 l3 forwarding")?;
        }

        // IPv4 only, deliberately. This exists so `kubectl port-forward` -- which connects to
        // localhost inside the pod netns -- reaches the workload inside the guest. There is no
        // IPv6 equivalent yet; a v6-only pod simply gets no port-forward support rather than
        // failing. Adding one would also need an IPv6 address on the tap to SNAT from.
        configure_port_forwarding(&pair.tap.tap_iface.name, pod_ipv4, TAP_IPV4_ADDR).await;

        Ok(())
    }

    async fn del(&self, _pair: &NetworkPair) -> Result<()> {
        // Nothing to do: every change made by `add()` lives inside the
        // pod netns (link-local addr on tap, /32 and /128 routes via tap,
        // proxy_arp on tap+virt, proxy_ndp on tap+virt, the proxy neighbour
        // entries, the IPv6 pod addresses removed from virt, and the
        // forwarding sysctls) and goes away when the netns is torn down
        // with the sandbox.
        Ok(())
    }
}

/// IPv4: proxy ARP plus a /32 route via the tap for each pod address.
async fn configure_ipv4(
    handle: &Handle,
    tap_name: &str,
    virt_name: &str,
    tap_index: u32,
    virt_index: u32,
    pod_addrs: &[Ipv4Addr],
) -> Result<()> {
    // Enable proxy arp so we can respond to ARP requests using the tap and virt interfaces.
    fs::write(format!("/proc/sys/net/ipv4/conf/{tap_name}/proxy_arp"), "1")
        .context(format!("enable proxy arp on {tap_name}"))?;
    fs::write(
        format!("/proc/sys/net/ipv4/conf/{virt_name}/proxy_arp"),
        "1",
    )
    .context(format!("enable proxy arp on {virt_name}"))?;
    fs::write("/proc/sys/net/ipv4/ip_forward", "1").context("enable ip forward")?;

    // We need the tap interface to have an address different from the pod ip for arp proxying
    // to work. If this didn't have an ip, when the host-side netns gets an arp request it will
    // forward it into the guest. However, it will set the src ip of the arp request to the pod
    // ip. This is a gratuitous ARP request since the request and response address are the same,
    // and the guest will not respond.
    // This also allows for SNATing link local connections from the host.
    // TODO: What if there are multiple tap interfaces? They would get the same address?
    //  Maybe that's fine?
    ignore_eexist(
        handle
            .address()
            .add(tap_index, IpAddr::V4(TAP_IPV4_ADDR), 32)
            .execute()
            .await,
    )
    .context("add link-local address to tap")?;

    // Remove rules in the local route table that have to do with our pod ips.
    let local_query = RouteMessageBuilder::<Ipv4Addr>::new()
        .table_id(ROUTE_TABLE_LOCAL as u32)
        .build();
    let mut local_routes = handle.route().get(local_query).execute();
    while let Some(route) = local_routes.try_next().await? {
        if route.header.table != ROUTE_TABLE_LOCAL {
            continue;
        }
        let mut dst_matches = false;
        let mut oif_matches = false;
        for attr in &route.attributes {
            match attr {
                RouteAttribute::Destination(RouteAddress::Inet(v4)) if pod_addrs.contains(v4) => {
                    dst_matches = true;
                }
                RouteAttribute::Oif(idx) if *idx == virt_index => {
                    oif_matches = true;
                }
                _ => {}
            }
        }
        if dst_matches && oif_matches && route.header.destination_prefix_length == IPV4_HOST_PREFIX
        {
            handle
                .route()
                .del(route)
                .execute()
                .await
                .context("delete local route for pod ip on virt iface")?;
        }
    }

    // Add a route for each pod ip via the tap interface.
    for pod_addr in pod_addrs {
        let route_msg = RouteMessageBuilder::<Ipv4Addr>::new()
            .destination_prefix(*pod_addr, IPV4_HOST_PREFIX)
            .output_interface(tap_index)
            .scope(RouteScope::Link)
            .build();
        ignore_eexist(handle.route().add(route_msg).execute().await)
            .with_context(|| format!("add route for pod ip {pod_addr} via tap"))?;
    }

    Ok(())
}

/// IPv6: the same shape as `configure_ipv4`, with two structural differences.
///
/// `proxy_arp` answers ARP for any address the host has a route for. `proxy_ndp` does not:
/// the kernel answers Neighbour Solicitations only for addresses explicitly present in the
/// neighbour proxy table. So the addresses the guest resolves, its gateways, each need a proxy
/// neighbour entry on the tap -- there is no IPv4 counterpart to this step.
///
/// And the pod addresses are moved off virt, with proxy entries on virt answering for them
/// instead; see below for why IPv4 can keep them.
async fn configure_ipv6(
    handle: &Handle,
    tap_name: &str,
    virt_name: &str,
    tap_index: u32,
    virt_index: u32,
    pod_addrs: &[Ipv6Addr],
    gateways: &[Ipv6Addr],
) -> Result<()> {
    // proxy_ndp on the tap answers the guest for its gateways, and on virt it answers the node
    // and the pod's peers for the pod addresses (both below).
    for iface in [tap_name, virt_name] {
        fs::write(format!("/proc/sys/net/ipv6/conf/{iface}/proxy_ndp"), "1")
            .context(format!("enable proxy ndp on {iface}"))?;
    }
    fs::write("/proc/sys/net/ipv6/conf/all/forwarding", "1").context("enable ipv6 forwarding")?;

    // Answer the guest's Neighbour Solicitations for its gateways.
    //
    // This is where IPv6 departs from the IPv4 path. `proxy_arp` answers ARP for anything the
    // host has a route for, so v4 needs no per-address setup. `proxy_ndp` answers only for
    // addresses explicitly present in the neighbour proxy table, so each address the guest
    // will resolve has to be named. The guest reaches everything through its route gateways,
    // so those are the addresses to proxy -- and proxying them on the tap is what lets the
    // guest's replies leave the sandbox at all.
    //
    // This only works when the guest routes via a gateway rather than treating peers as
    // on-link, which `workload_addresses` and `check_ipv6_routes` have made sure of: on-link
    // peers would need `proxy_arp`'s catch-all behaviour, which NDP cannot express.
    for gateway in gateways {
        ignore_eexist(
            handle
                .neighbours()
                .add(tap_index, IpAddr::V6(*gateway))
                .flags(NeighbourFlags::Proxy)
                .execute()
                .await,
        )
        .with_context(|| format!("add proxy neighbour {gateway} on {tap_name}"))?;
    }

    // Move the pod addresses off virt, and answer for them there with proxy NDP instead.
    //
    // While virt holds a pod address, the pod netns solicits its own neighbours on virt -- the
    // gateway, and with an on-link prefix its peers -- from that address whenever a packet from
    // the guest triggers the solicitation: the kernel reuses a packet's source address if the
    // outgoing interface owns it (ndisc_solicit()). The Neighbour Advertisement then comes back
    // to the pod address, which is routed to the tap like any other traffic for the guest, so
    // the pod netns never learns the neighbour and the guest's traffic stops. Without the
    // address, the solicitations go out from virt's link-local address. This only shows when the
    // pod netns has to resolve a neighbour: the AWS VPC CNI hides it behind a permanent
    // neighbour entry for its gateway. IPv4 does not need this: ARP replies are not routed.
    //
    // Removing the address also removes the routes the kernel keeps for it on virt, and both
    // have to go. With the local-table route left in place, the sandbox would answer for the
    // guest itself: solicitations and echo requests served locally and never reaching the VM. And a
    // /128 address gives virt a main-table /128 route at metric 256 (IP6_RT_PRIO_ADDRCONF),
    // which would outrank the /128 via the tap added below at the default 1024
    // (IP6_RT_PRIO_USER) and send the guest's traffic back out of virt.
    //
    // The proxy entry goes in first, so that the node gets an answer throughout.
    for pod_addr in pod_addrs {
        ignore_eexist(
            handle
                .neighbours()
                .add(virt_index, IpAddr::V6(*pod_addr))
                .flags(NeighbourFlags::Proxy)
                .execute()
                .await,
        )
        .with_context(|| format!("add proxy neighbour {pod_addr} on {virt_name}"))?;
    }
    let mut addresses = handle
        .address()
        .get()
        .set_link_index_filter(virt_index)
        .execute();
    let mut assigned = Vec::new();
    while let Some(address) = addresses.try_next().await? {
        let is_pod_addr = address.attributes.iter().any(|attr| {
            matches!(attr, AddressAttribute::Address(IpAddr::V6(addr)) if pod_addrs.contains(addr))
        });
        if is_pod_addr {
            assigned.push(address);
        }
    }
    for address in assigned {
        handle
            .address()
            .del(address)
            .execute()
            .await
            .with_context(|| format!("remove pod address from {virt_name}"))?;
    }

    // Add a route for each pod ip via the tap interface.
    for pod_addr in pod_addrs {
        let route_msg = RouteMessageBuilder::<Ipv6Addr>::new()
            .destination_prefix(*pod_addr, IPV6_HOST_PREFIX)
            .output_interface(tap_index)
            .scope(RouteScope::Link)
            .build();
        ignore_eexist(handle.route().add(route_msg).execute().await)
            .with_context(|| format!("add route for pod ip {pod_addr} via tap"))?;
    }

    Ok(())
}

/// IPv6 gateways reachable through the virt interface.
///
/// These are the addresses the guest will send Neighbour Solicitations for, so they are the
/// ones that need proxying on the tap.
async fn route_gateways(handle: &Handle, virt_index: u32) -> Result<Vec<Ipv6Addr>> {
    let query = RouteMessageBuilder::<Ipv6Addr>::new().build();
    let mut routes = handle.route().get(query).execute();
    let mut gateways: Vec<Ipv6Addr> = Vec::new();
    while let Some(route) = routes.try_next().await? {
        let mut via_virt = false;
        let mut gateway = None;
        for attr in &route.attributes {
            match attr {
                RouteAttribute::Oif(index) if *index == virt_index => via_virt = true,
                RouteAttribute::Gateway(RouteAddress::Inet6(addr)) => gateway = Some(*addr),
                _ => {}
            }
        }
        if via_virt {
            if let Some(gateway) = gateway {
                if !gateways.contains(&gateway) {
                    gateways.push(gateway);
                }
            }
        }
    }
    Ok(gateways)
}

/// Refuse IPv6 routes on the virt interface that l3forwarding cannot serve.
///
/// The guest gets every non-kernel route of the pod interface, so each would fail silently:
/// - A route without a gateway makes the guest solicit the destinations directly, and proxy NDP
///   only answers for the gateways. The one such route that works is the host route to a
///   gateway, which the AWS VPC CNI installs for fe80::1.
/// - A route learned from a Router Advertisement expires unless further Router Advertisements
///   refresh it, and once IPv6 forwarding is on the pod netns ignores them (accept_ra=1). The
///   guest's copy, which has no lifetime, would then point at a gateway the host no longer
///   routes through.
/// - Any other route that expires fails the same way.
/// - A multipath route keeps its interfaces and gateways inside its next hops, where neither
///   the gateway proxying nor the routes handed to the guest look, so the guest would get no
///   route at all.
async fn check_ipv6_routes(handle: &Handle, virt_index: u32, gateways: &[Ipv6Addr]) -> Result<()> {
    let query = RouteMessageBuilder::<Ipv6Addr>::new().build();
    let mut routes = handle.route().get(query).execute();
    while let Some(route) = routes.try_next().await? {
        // Kernel routes are not handed to the guest.
        if u8::from(route.header.protocol) == libc::RTPROT_KERNEL {
            continue;
        }
        let mut via_virt = false;
        let mut multipath_via_virt = false;
        let mut has_gateway = false;
        let mut expires = 0;
        let mut destination = Ipv6Addr::UNSPECIFIED;
        for attr in &route.attributes {
            match attr {
                RouteAttribute::Oif(index) if *index == virt_index => via_virt = true,
                RouteAttribute::MultiPath(next_hops) => {
                    multipath_via_virt = next_hops
                        .iter()
                        .any(|next_hop| next_hop.interface_index == virt_index)
                }
                RouteAttribute::Gateway(_) => has_gateway = true,
                RouteAttribute::CacheInfo(info) => expires = info.expires,
                RouteAttribute::Destination(RouteAddress::Inet6(addr)) => destination = *addr,
                _ => {}
            }
        }
        let prefix_len = route.header.destination_prefix_length;
        if multipath_via_virt {
            anyhow::bail!(
                "l3forwarding cannot serve the multipath IPv6 route {}/{} on the virt iface: \
                 it only supports routes with a single next hop",
                destination,
                prefix_len
            );
        }
        if !via_virt {
            continue;
        }
        if route.header.protocol == RouteProtocol::Ra {
            anyhow::bail!(
                "l3forwarding cannot keep the IPv6 route {}/{} on the virt iface: it was learned \
                 from a Router Advertisement, and the pod netns stops accepting those once \
                 forwarding is enabled",
                destination,
                prefix_len
            );
        }
        if expires != 0 {
            anyhow::bail!(
                "l3forwarding cannot keep the IPv6 route {}/{} on the virt iface: it expires, \
                 and the guest's copy would not",
                destination,
                prefix_len
            );
        }
        if has_gateway {
            continue;
        }
        if prefix_len != IPV6_HOST_PREFIX || !gateways.contains(&destination) {
            anyhow::bail!(
                "l3forwarding cannot serve the on-link IPv6 route {}/{} on the virt iface: \
                 proxy NDP only answers for gateways",
                destination,
                prefix_len
            );
        }
    }
    Ok(())
}

/// Refuse a virt interface without an IPv6 link-local address.
///
/// `configure_ipv6` moves the pod addresses off virt, and from then on the pod netns solicits its
/// neighbours on virt from virt's link-local address. Without one, the kernel sends no
/// solicitations at all (ndisc_send_ns()), and the guest's traffic stops at the first neighbour
/// the pod netns has to resolve.
fn check_ipv6_link_local(addresses: impl IntoIterator<Item = IpAddr>) -> Result<()> {
    let has_link_local = addresses
        .into_iter()
        .any(|address| matches!(address, IpAddr::V6(v6) if v6.is_unicast_link_local()));
    if !has_link_local {
        anyhow::bail!(
            "l3forwarding needs an IPv6 link-local address on the virt iface: the pod netns \
             solicits its neighbours from it"
        );
    }
    Ok(())
}

/// Split the virt interface addresses into the workload addresses of each family.
///
/// IPv6 link-local addresses are not workload addresses -- every interface has one -- so they
/// are skipped rather than forwarded.
///
/// An IPv6 workload address must be a /128. With an on-link prefix the guest would solicit its
/// same-prefix peers directly, and proxy NDP only answers for the gateways, so that traffic
/// would be dropped while setup reported success. IPv4 has no such limit: proxy ARP answers for
/// anything the pod netns has a route to.
fn workload_addresses(
    addresses: impl IntoIterator<Item = (IpAddr, u8)>,
) -> Result<(Vec<Ipv4Addr>, Vec<Ipv6Addr>)> {
    let mut ipv4_addresses = Vec::new();
    let mut ipv6_addresses = Vec::new();
    for (address, prefix_len) in addresses {
        match address {
            IpAddr::V4(address) => ipv4_addresses.push(address),
            IpAddr::V6(address) if address.is_unicast_link_local() => {}
            IpAddr::V6(address) if prefix_len != IPV6_HOST_PREFIX => anyhow::bail!(
                "l3forwarding needs a /{} for an IPv6 pod address, but the virt iface has {}/{}",
                IPV6_HOST_PREFIX,
                address,
                prefix_len
            ),
            IpAddr::V6(address) => ipv6_addresses.push(address),
        }
    }
    Ok((ipv4_addresses, ipv6_addresses))
}

fn ignore_eexist(result: Result<(), rtnetlink::Error>) -> Result<(), rtnetlink::Error> {
    match result {
        Err(err) if is_eexist(&err) => Ok(()),
        result => result,
    }
}

fn is_eexist(err: &rtnetlink::Error) -> bool {
    match err {
        rtnetlink::Error::NetlinkError(message) => {
            message.code.is_some_and(|code| code.get() == -libc::EEXIST)
        }
        _ => false,
    }
}

pub async fn fetch_index(handle: &Handle, name: &str) -> Result<u32> {
    let link = crate::network::network_pair::get_link_by_name(handle, name)
        .await
        .context("get link by name")?;
    let base = link.attrs();
    Ok(base.index)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTE_TABLE_MAIN: u8 = 254;

    #[test]
    fn splits_addresses_by_family_and_skips_link_local() {
        let (v4, v6) = workload_addresses([
            ("10.244.0.8".parse().unwrap(), 24),
            ("fe80::1234".parse().unwrap(), 64),
            ("fd00::8".parse().unwrap(), 128),
        ])
        .unwrap();

        assert_eq!(v4, vec!["10.244.0.8".parse::<Ipv4Addr>().unwrap()]);
        assert_eq!(v6, vec!["fd00::8".parse::<Ipv6Addr>().unwrap()]);
    }

    #[test]
    fn accepts_ipv6_only_workloads() {
        let (v4, v6) = workload_addresses([
            ("2001:db8::8".parse().unwrap(), 128),
            ("fe80::1".parse().unwrap(), 64),
        ])
        .unwrap();

        assert!(v4.is_empty());
        assert_eq!(v6, vec!["2001:db8::8".parse::<Ipv6Addr>().unwrap()]);
    }

    #[test]
    fn rejects_ipv6_on_link_prefixes() {
        // Proxy NDP cannot answer for a whole prefix, so a /64 has to be refused up front rather
        // than leaving the guest unable to reach its same-prefix peers.
        let err = workload_addresses([
            ("10.244.0.8".parse().unwrap(), 24),
            ("2001:db8::8".parse().unwrap(), 64),
        ])
        .unwrap_err();
        assert!(
            err.to_string().contains("2001:db8::8/64"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn requires_an_ipv6_link_local_address() {
        check_ipv6_link_local(["2001:db8::8".parse().unwrap(), "fe80::8".parse().unwrap()])
            .expect("a link-local address is there");

        // An IPv4 link-local address does not count.
        let err = check_ipv6_link_local([
            "2001:db8::8".parse().unwrap(),
            "169.254.0.8".parse().unwrap(),
        ])
        .unwrap_err();
        assert!(
            err.to_string().contains("link-local"),
            "unexpected error: {}",
            err
        );
    }

    /// Exercises the real netlink/sysctl paths inside a throwaway netns.
    ///
    /// Requires root: creating a netns and veth pair needs CAP_NET_ADMIN.
    #[actix_rt::test]
    async fn ipv6_proxies_the_gateway_and_routes_via_the_tap() {
        test_utils::skip_if_not_root!();

        const TAP: &str = "kata-t-tap";
        const VIRT: &str = "kata-t-virt";
        let pod_addr: Ipv6Addr = "fd00:dead:beef::8".parse().unwrap();

        let ns_name = format!("kata-l3fwd-{}", std::process::id());
        let netns = netns_rs::NetNs::new(&ns_name).expect("create netns");
        let ns_path = netns.path().to_path_buf();
        defer!({
            let _ = std::process::Command::new("ip")
                .args(["netns", "del", &ns_name])
                .status();
        });

        let guard =
            kata_sys_util::netns::NetnsGuard::new(ns_path.to_str().unwrap()).expect("enter netns");

        let (connection, handle, _) = rtnetlink::new_connection().expect("new connection");
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });

        handle
            .link()
            .add(rtnetlink::LinkVeth::new(TAP, VIRT).build())
            .execute()
            .await
            .expect("create veth pair");

        let tap_index = fetch_index(&handle, TAP).await.expect("tap index");
        let virt_index = fetch_index(&handle, VIRT).await.expect("virt index");

        // A link-scope route via a down interface is rejected with ENETDOWN.
        for index in [tap_index, virt_index] {
            handle
                .link()
                .set(rtnetlink::LinkUnspec::new_with_index(index).up().build())
                .execute()
                .await
                .expect("set link up");
        }

        // A /128, as the AWS VPC CNI assigns it. The kernel then adds a main-table /128 route on
        // virt at metric 256, which configure_ipv6 must not let outrank the route via the tap.
        handle
            .address()
            .add(virt_index, IpAddr::V6(pod_addr), IPV6_HOST_PREFIX)
            .execute()
            .await
            .expect("assign pod address");

        // A default route via a gateway, as a CNI would install. The gateway is the address
        // the guest resolves, so it is what must end up proxied on the tap. The AWS VPC CNI also
        // adds a host route to the gateway, an on-link route that must be accepted.
        let gateway: Ipv6Addr = "fe80::1".parse().unwrap();
        handle
            .route()
            .add(
                RouteMessageBuilder::<Ipv6Addr>::new()
                    .destination_prefix(gateway, IPV6_HOST_PREFIX)
                    .output_interface(virt_index)
                    .build(),
            )
            .execute()
            .await
            .expect("add host route to the gateway");
        handle
            .route()
            .add(
                RouteMessageBuilder::<Ipv6Addr>::new()
                    .gateway(gateway)
                    .output_interface(virt_index)
                    .build(),
            )
            .execute()
            .await
            .expect("add default route via gateway");

        let gateways = route_gateways(&handle, virt_index)
            .await
            .expect("collect gateways");
        check_ipv6_routes(&handle, virt_index, &gateways)
            .await
            .expect("the host route to the gateway must be accepted");
        configure_ipv6(
            &handle,
            TAP,
            VIRT,
            tap_index,
            virt_index,
            &[pod_addr],
            &gateways,
        )
        .await
        .expect("configure_ipv6");

        // proxy_ndp on both: the tap answers the guest for its gateway, virt answers the node
        // for the pod address.
        for iface in [TAP, VIRT] {
            let ndp = std::fs::read_to_string(format!("/proc/sys/net/ipv6/conf/{iface}/proxy_ndp"))
                .expect("read proxy_ndp");
            assert_eq!(ndp.trim(), "1", "proxy_ndp not enabled on {}", iface);
        }
        let fwd = std::fs::read_to_string("/proc/sys/net/ipv6/conf/all/forwarding")
            .expect("read forwarding");
        assert_eq!(fwd.trim(), "1", "ipv6 forwarding not enabled");

        // The gateway must be proxied on the tap, which is what lets the guest resolve its next
        // hop, and the pod address on virt, which is what lets the node resolve it. A plain
        // neighbour dump does not include proxies -- `.proxies()` is the equivalent of
        // `ip neigh show proxy`.
        let mut proxied: Vec<(u32, Ipv6Addr)> = Vec::new();
        let mut neighs = handle.neighbours().get().proxies().execute();
        while let Some(n) = neighs.try_next().await.expect("list proxy neighbours") {
            if let Some(dst) = n.attributes.iter().find_map(|a| match a {
                netlink_packet_route::neighbour::NeighbourAttribute::Destination(
                    netlink_packet_route::neighbour::NeighbourAddress::Inet6(v6),
                ) => Some(*v6),
                _ => None,
            }) {
                proxied.push((n.header.ifindex, dst));
            }
        }
        assert!(
            proxied.contains(&(tap_index, gateway)),
            "expected the gateway {} proxied on the tap ({}), got {:?}",
            gateway,
            tap_index,
            proxied
        );
        assert!(
            proxied.contains(&(virt_index, pod_addr)),
            "expected the pod address {} proxied on virt ({}), got {:?}",
            pod_addr,
            virt_index,
            proxied
        );
        assert!(
            !proxied.contains(&(tap_index, pod_addr)),
            "the pod address must not be proxied on the tap, got {:?}",
            proxied
        );

        // The pod address is gone from virt, so the pod netns solicits its own neighbours from
        // virt's link-local address.
        let mut addresses = handle
            .address()
            .get()
            .set_link_index_filter(virt_index)
            .execute();
        while let Some(address) = addresses.try_next().await.expect("list addresses") {
            assert!(
                !address.attributes.iter().any(|a| matches!(
                    a,
                    AddressAttribute::Address(IpAddr::V6(v6)) if *v6 == pod_addr
                )),
                "the pod address {} is still assigned to virt",
                pod_addr
            );
        }

        // The /128 for the pod address goes via the tap, and no /128 for it is left on virt in
        // either the main or the local table.
        let query = RouteMessageBuilder::<Ipv6Addr>::new().build();
        let mut routes = handle.route().get(query).execute();
        let mut pod_routes: Vec<(u8, u32)> = Vec::new();
        while let Some(r) = routes.try_next().await.expect("list routes") {
            if r.header.destination_prefix_length != IPV6_HOST_PREFIX {
                continue;
            }
            let dst_ok = r.attributes.iter().any(|a| {
                matches!(a, RouteAttribute::Destination(RouteAddress::Inet6(v6)) if *v6 == pod_addr)
            });
            let oif = r.attributes.iter().find_map(|a| match a {
                RouteAttribute::Oif(i) => Some(*i),
                _ => None,
            });
            if let (true, Some(oif)) = (dst_ok, oif) {
                pod_routes.push((r.header.table, oif));
            }
        }
        assert!(
            pod_routes.contains(&(ROUTE_TABLE_MAIN, tap_index)),
            "no /128 route for {} via tap, got {:?}",
            pod_addr,
            pod_routes
        );
        assert!(
            !pod_routes.iter().any(|(_, oif)| *oif == virt_index),
            "a /128 route for {} is still on virt, got {:?}",
            pod_addr,
            pod_routes
        );

        drop(guard);
    }

    /// The pod netns resolves the gateway itself, as with any CNI that does not install a
    /// permanent neighbour entry for it, and the guest reaches it both ways.
    ///
    /// A veth stands in for the tap, with a netns behind it configured the way the agent
    /// configures the guest, and another netns plays the node. If the pod address stayed on
    /// virt, the pod netns would solicit the gateway from it and the answer would be routed to
    /// the guest, so neither ping would get through.
    ///
    /// Requires root: creating netns and veth pairs needs CAP_NET_ADMIN.
    #[actix_rt::test]
    async fn ipv6_guest_reaches_a_gateway_the_pod_netns_resolves() {
        test_utils::skip_if_not_root!();

        const VIRT: &str = "kata-g-eth0";
        const TAP: &str = "kata-g-tap";
        let pod_addr = "fd00:dead:beef::8";
        let gateway = "fd00:dead:beef::1";

        let pid = std::process::id();
        let [pod_ns, node_ns, guest_ns] =
            ["pod", "node", "guest"].map(|role| format!("kata-l3fwd-{role}-{pid}"));
        defer!({
            for ns in [&pod_ns, &node_ns, &guest_ns] {
                let _ = std::process::Command::new("ip")
                    .args(["netns", "del", ns])
                    .status();
            }
        });
        // Runs `ip` with a whitespace-separated argument string.
        let ip = |args: String| {
            let status = std::process::Command::new("ip")
                .args(args.split_whitespace())
                .status()
                .expect("run ip");
            assert!(status.success(), "ip {} failed", args);
        };
        let ping = |ns: &str, target: &str| {
            std::process::Command::new("ip")
                .args(format!("netns exec {ns} ping -6 -c 2 -W 2 {target}").split_whitespace())
                .status()
                .expect("run ping")
                .success()
        };

        for ns in [&pod_ns, &node_ns, &guest_ns] {
            ip(format!("netns add {ns}"));
        }
        // The node owns the gateway, on-link with the pod address.
        ip(format!(
            "-n {node_ns} link add gw type veth peer {VIRT} netns {pod_ns}"
        ));
        ip(format!("-n {node_ns} addr add {gateway}/64 dev gw nodad"));
        ip(format!("-n {node_ns} link set gw up"));
        // The pod netns as a /128 CNI leaves it, with no neighbour entries.
        ip(format!(
            "-n {pod_ns} addr add {pod_addr}/128 dev {VIRT} nodad"
        ));
        ip(format!("-n {pod_ns} link set {VIRT} up"));
        ip(format!("-n {pod_ns} -6 route add {gateway}/128 dev {VIRT}"));
        ip(format!(
            "-n {pod_ns} -6 route add default via {gateway} dev {VIRT}"
        ));
        // The guest, behind the stand-in tap, with the same address and routes.
        ip(format!(
            "-n {pod_ns} link add {TAP} type veth peer eth0 netns {guest_ns}"
        ));
        ip(format!("-n {pod_ns} link set {TAP} up"));
        ip(format!(
            "-n {guest_ns} addr add {pod_addr}/128 dev eth0 nodad"
        ));
        ip(format!("-n {guest_ns} link set eth0 up"));
        ip(format!("-n {guest_ns} -6 route add {gateway}/128 dev eth0"));
        ip(format!(
            "-n {guest_ns} -6 route add default via {gateway} dev eth0"
        ));

        {
            let _guard = kata_sys_util::netns::NetnsGuard::new(&format!("/run/netns/{pod_ns}"))
                .expect("enter pod netns");
            let (connection, handle, _) = rtnetlink::new_connection().expect("new connection");
            let thread_handler = tokio::spawn(connection);
            defer!({
                thread_handler.abort();
            });
            let tap_index = fetch_index(&handle, TAP).await.expect("tap index");
            let virt_index = fetch_index(&handle, VIRT).await.expect("virt index");
            let gateways = route_gateways(&handle, virt_index)
                .await
                .expect("collect gateways");
            configure_ipv6(
                &handle,
                TAP,
                VIRT,
                tap_index,
                virt_index,
                &[pod_addr.parse().unwrap()],
                &gateways,
            )
            .await
            .expect("configure_ipv6");
        }

        // Wait for duplicate address detection to finish: until it does, the link-local
        // addresses are tentative and the kernel sends no solicitations from them. A CNI brings
        // the pod interface up long before the sandbox starts.
        for ns in [&pod_ns, &node_ns, &guest_ns] {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                let output = std::process::Command::new("ip")
                    .args(["-n", ns, "-6", "addr", "show", "tentative"])
                    .output()
                    .expect("run ip");
                if output.stdout.is_empty() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "addresses in {} are still tentative",
                    ns
                );
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }

        // The guest goes first, so the pod netns cannot have learned the gateway from the
        // node's own solicitation.
        assert!(
            ping(&guest_ns, gateway),
            "the guest cannot reach its gateway"
        );
        assert!(ping(&node_ns, pod_addr), "the node cannot reach the guest");
    }

    /// A dual-stack pod interface with an on-link IPv6 route that proxy NDP cannot serve is
    /// refused before either family is configured, so the IPv4 side is left as the CNI set it
    /// up too.
    ///
    /// Requires root: creating a netns, a veth pair and a TAP needs CAP_NET_ADMIN.
    #[actix_rt::test]
    async fn on_link_ipv6_route_is_refused_before_anything_is_configured() {
        test_utils::skip_if_not_root!();

        const VIRT: &str = "kata-d-eth0";
        const PEER: &str = "kata-d-peer";
        let pod_v4: Ipv4Addr = "10.0.1.5".parse().unwrap();
        let pod_v6: Ipv6Addr = "fd00:dead:beef::8".parse().unwrap();
        let gateway: Ipv6Addr = "fe80::1".parse().unwrap();
        let on_link: Ipv6Addr = "2001:db8:2::".parse().unwrap();

        let ns_name = format!("kata-l3fwd-dual-{}", std::process::id());
        let netns = netns_rs::NetNs::new(&ns_name).expect("create netns");
        let ns_path = netns.path().to_path_buf();
        defer!({
            let _ = std::process::Command::new("ip")
                .args(["netns", "del", &ns_name])
                .status();
        });
        let _guard =
            kata_sys_util::netns::NetnsGuard::new(ns_path.to_str().unwrap()).expect("enter netns");
        let (connection, handle, _) = rtnetlink::new_connection().expect("new connection");
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });

        handle
            .link()
            .add(rtnetlink::LinkVeth::new(VIRT, PEER).build())
            .execute()
            .await
            .expect("create veth pair");
        let virt_index = fetch_index(&handle, VIRT).await.expect("virt index");
        let peer_index = fetch_index(&handle, PEER).await.expect("peer index");
        for index in [virt_index, peer_index] {
            handle
                .link()
                .set(rtnetlink::LinkUnspec::new_with_index(index).up().build())
                .execute()
                .await
                .expect("set link up");
        }
        handle
            .address()
            .add(virt_index, IpAddr::V4(pod_v4), IPV4_HOST_PREFIX)
            .execute()
            .await
            .expect("assign pod IPv4 address");
        handle
            .address()
            .add(virt_index, IpAddr::V6(pod_v6), IPV6_HOST_PREFIX)
            .execute()
            .await
            .expect("assign pod IPv6 address");
        for (destination, prefix_len) in [(gateway, IPV6_HOST_PREFIX), (on_link, 64)] {
            handle
                .route()
                .add(
                    RouteMessageBuilder::<Ipv6Addr>::new()
                        .destination_prefix(destination, prefix_len)
                        .output_interface(virt_index)
                        .build(),
                )
                .execute()
                .await
                .expect("add on-link route");
        }
        handle
            .route()
            .add(
                RouteMessageBuilder::<Ipv6Addr>::new()
                    .gateway(gateway)
                    .output_interface(virt_index)
                    .build(),
            )
            .execute()
            .await
            .expect("add default route via gateway");

        let pair = crate::network::NetworkPair::new(
            &handle,
            0,
            VIRT,
            crate::network::network_model::L3_FORWARDING_NET_MODEL_STR,
            1,
        )
        .await
        .expect("new network pair");
        let tap = pair.tap.tap_iface.name.clone();
        let err = pair
            .add_network_model()
            .await
            .expect_err("an on-link /64 must be refused");
        assert!(
            format!("{:#}", err).contains("2001:db8:2::/64"),
            "unexpected error: {:#}",
            err
        );

        // Neither family was configured: the IPv4 local route is still on virt, nothing routes
        // the pod address via the tap, and the tap has neither proxy ARP nor proxy NDP.
        let tap_index = fetch_index(&handle, &tap).await.expect("tap index");
        let mut local_on_virt = false;
        let mut via_tap = false;
        let mut routes = handle
            .route()
            .get(RouteMessageBuilder::<Ipv4Addr>::new().build())
            .execute();
        while let Some(route) = routes.try_next().await.expect("list routes") {
            let to_pod = route.attributes.iter().any(|a| {
                matches!(a, RouteAttribute::Destination(RouteAddress::Inet(v4)) if *v4 == pod_v4)
            });
            let oif = route.attributes.iter().find_map(|a| match a {
                RouteAttribute::Oif(index) => Some(*index),
                _ => None,
            });
            if to_pod && route.header.table == ROUTE_TABLE_LOCAL && oif == Some(virt_index) {
                local_on_virt = true;
            }
            if to_pod && oif == Some(tap_index) {
                via_tap = true;
            }
        }
        assert!(
            local_on_virt,
            "the IPv4 local route for the pod address was deleted"
        );
        assert!(!via_tap, "the pod IPv4 address was routed via the tap");
        for (family, knob) in [("ipv4", "proxy_arp"), ("ipv6", "proxy_ndp")] {
            let value =
                std::fs::read_to_string(format!("/proc/sys/net/{family}/conf/{tap}/{knob}"))
                    .expect("read tap sysctl");
            assert_eq!(value.trim(), "0", "{} was enabled on the tap", knob);
        }
    }

    /// A default route learned from a Router Advertisement is refused: once forwarding is on,
    /// the pod netns stops accepting the Router Advertisements that keep it alive.
    ///
    /// Requires root: creating a netns and veth pair needs CAP_NET_ADMIN.
    #[actix_rt::test]
    async fn ipv6_route_from_a_router_advertisement_is_refused() {
        test_utils::skip_if_not_root!();

        const VIRT: &str = "kata-r-eth0";
        const PEER: &str = "kata-r-peer";
        let pod_addr: Ipv6Addr = "fd00:dead:beef::8".parse().unwrap();
        let gateway: Ipv6Addr = "fe80::1".parse().unwrap();

        let ns_name = format!("kata-l3fwd-ra-{}", std::process::id());
        let netns = netns_rs::NetNs::new(&ns_name).expect("create netns");
        let ns_path = netns.path().to_path_buf();
        defer!({
            let _ = std::process::Command::new("ip")
                .args(["netns", "del", &ns_name])
                .status();
        });
        let _guard =
            kata_sys_util::netns::NetnsGuard::new(ns_path.to_str().unwrap()).expect("enter netns");
        let (connection, handle, _) = rtnetlink::new_connection().expect("new connection");
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });

        handle
            .link()
            .add(rtnetlink::LinkVeth::new(VIRT, PEER).build())
            .execute()
            .await
            .expect("create veth pair");
        let virt_index = fetch_index(&handle, VIRT).await.expect("virt index");
        let peer_index = fetch_index(&handle, PEER).await.expect("peer index");
        for index in [virt_index, peer_index] {
            handle
                .link()
                .set(rtnetlink::LinkUnspec::new_with_index(index).up().build())
                .execute()
                .await
                .expect("set link up");
        }
        handle
            .address()
            .add(virt_index, IpAddr::V6(pod_addr), IPV6_HOST_PREFIX)
            .execute()
            .await
            .expect("assign pod address");
        handle
            .route()
            .add(
                RouteMessageBuilder::<Ipv6Addr>::new()
                    .gateway(gateway)
                    .output_interface(virt_index)
                    .protocol(RouteProtocol::Ra)
                    .build(),
            )
            .execute()
            .await
            .expect("add default route learned from a Router Advertisement");

        let gateways = route_gateways(&handle, virt_index)
            .await
            .expect("collect gateways");
        let err = check_ipv6_routes(&handle, virt_index, &gateways)
            .await
            .expect_err("a Router Advertisement route must be refused");
        assert!(
            format!("{:#}", err).contains("Router Advertisement"),
            "unexpected error: {:#}",
            err
        );
    }

    /// A pod interface without an IPv6 link-local address is refused before anything is
    /// configured: once the pod address is moved off it, the pod netns could not solicit its
    /// neighbours from any address.
    ///
    /// Requires root: creating a netns, a veth pair and a TAP needs CAP_NET_ADMIN.
    #[actix_rt::test]
    async fn ipv6_pod_interface_without_link_local_is_refused() {
        test_utils::skip_if_not_root!();

        const VIRT: &str = "kata-l-eth0";
        let ns_name = format!("kata-l3fwd-noll-{}", std::process::id());
        defer!({
            let _ = std::process::Command::new("ip")
                .args(["netns", "del", &ns_name])
                .status();
        });
        // A /128 with a host route to the gateway and a default route via it, as the AWS VPC
        // CNI sets it up, but with no link-local address on the pod interface.
        for args in [
            format!("netns add {ns_name}"),
            format!("-n {ns_name} link add {VIRT} type veth peer kata-l-peer"),
            format!("-n {ns_name} link set {VIRT} addrgenmode none"),
            format!("-n {ns_name} link set kata-l-peer up"),
            format!("-n {ns_name} link set {VIRT} up"),
            format!("-n {ns_name} addr add fd00:dead:beef::8/128 dev {VIRT} nodad"),
            format!("-n {ns_name} -6 route add fe80::1/128 dev {VIRT}"),
            format!("-n {ns_name} -6 route add default via fe80::1 dev {VIRT}"),
        ] {
            let status = std::process::Command::new("ip")
                .args(args.split_whitespace())
                .status()
                .expect("run ip");
            assert!(status.success(), "ip {} failed", args);
        }

        let _guard = kata_sys_util::netns::NetnsGuard::new(&format!("/run/netns/{ns_name}"))
            .expect("enter netns");
        let (connection, handle, _) = rtnetlink::new_connection().expect("new connection");
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });
        let pair = crate::network::NetworkPair::new(
            &handle,
            0,
            VIRT,
            crate::network::network_model::L3_FORWARDING_NET_MODEL_STR,
            1,
        )
        .await
        .expect("new network pair");
        let err = pair
            .add_network_model()
            .await
            .expect_err("a pod interface without a link-local address must be refused");
        assert!(
            format!("{:#}", err).contains("link-local"),
            "unexpected error: {:#}",
            err
        );
        // Nothing was configured: the pod address is still on the pod interface.
        let output = std::process::Command::new("ip")
            .args(["-n", &ns_name, "-6", "addr", "show", "dev", VIRT])
            .output()
            .expect("run ip");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("fd00:dead:beef::8/128"),
            "the pod address was moved off the pod interface"
        );
    }

    /// A default route that expires is refused: the guest's copy of it would not.
    ///
    /// Requires root: creating a netns and veth pair needs CAP_NET_ADMIN.
    #[actix_rt::test]
    async fn expiring_ipv6_route_is_refused() {
        test_utils::skip_if_not_root!();

        const VIRT: &str = "kata-x-eth0";
        const PEER: &str = "kata-x-peer";
        let pod_addr: Ipv6Addr = "fd00:dead:beef::8".parse().unwrap();
        let gateway: Ipv6Addr = "fe80::1".parse().unwrap();

        let ns_name = format!("kata-l3fwd-expiring-{}", std::process::id());
        let netns = netns_rs::NetNs::new(&ns_name).expect("create netns");
        let ns_path = netns.path().to_path_buf();
        defer!({
            let _ = std::process::Command::new("ip")
                .args(["netns", "del", &ns_name])
                .status();
        });
        let _guard =
            kata_sys_util::netns::NetnsGuard::new(ns_path.to_str().unwrap()).expect("enter netns");
        let (connection, handle, _) = rtnetlink::new_connection().expect("new connection");
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });

        handle
            .link()
            .add(rtnetlink::LinkVeth::new(VIRT, PEER).build())
            .execute()
            .await
            .expect("create veth pair");
        let virt_index = fetch_index(&handle, VIRT).await.expect("virt index");
        let peer_index = fetch_index(&handle, PEER).await.expect("peer index");
        for index in [virt_index, peer_index] {
            handle
                .link()
                .set(rtnetlink::LinkUnspec::new_with_index(index).up().build())
                .execute()
                .await
                .expect("set link up");
        }
        handle
            .address()
            .add(virt_index, IpAddr::V6(pod_addr), IPV6_HOST_PREFIX)
            .execute()
            .await
            .expect("assign pod address");
        let mut route = RouteMessageBuilder::<Ipv6Addr>::new()
            .gateway(gateway)
            .output_interface(virt_index)
            .build();
        route.attributes.push(RouteAttribute::Expires(600));
        handle
            .route()
            .add(route)
            .execute()
            .await
            .expect("add expiring default route");

        let gateways = route_gateways(&handle, virt_index)
            .await
            .expect("collect gateways");
        let err = check_ipv6_routes(&handle, virt_index, &gateways)
            .await
            .expect_err("an expiring route must be refused");
        assert!(
            format!("{:#}", err).contains("it expires"),
            "unexpected error: {:#}",
            err
        );
    }

    /// An ECMP default route is refused: the kernel reports its interfaces and gateways only
    /// inside RTA_MULTIPATH, so the gateways would not be proxied and the guest would get no
    /// default route.
    ///
    /// Requires root: creating a netns and veth pair needs CAP_NET_ADMIN.
    #[actix_rt::test]
    async fn multipath_ipv6_route_is_refused() {
        test_utils::skip_if_not_root!();

        const VIRT: &str = "kata-m-eth0";
        const PEER: &str = "kata-m-peer";
        let pod_addr: Ipv6Addr = "fd00:dead:beef::8".parse().unwrap();
        let gateways: [Ipv6Addr; 2] = ["fe80::1".parse().unwrap(), "fe80::2".parse().unwrap()];

        let ns_name = format!("kata-l3fwd-ecmp-{}", std::process::id());
        let netns = netns_rs::NetNs::new(&ns_name).expect("create netns");
        let ns_path = netns.path().to_path_buf();
        defer!({
            let _ = std::process::Command::new("ip")
                .args(["netns", "del", &ns_name])
                .status();
        });
        let _guard =
            kata_sys_util::netns::NetnsGuard::new(ns_path.to_str().unwrap()).expect("enter netns");
        let (connection, handle, _) = rtnetlink::new_connection().expect("new connection");
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });

        handle
            .link()
            .add(rtnetlink::LinkVeth::new(VIRT, PEER).build())
            .execute()
            .await
            .expect("create veth pair");
        let virt_index = fetch_index(&handle, VIRT).await.expect("virt index");
        let peer_index = fetch_index(&handle, PEER).await.expect("peer index");
        for index in [virt_index, peer_index] {
            handle
                .link()
                .set(rtnetlink::LinkUnspec::new_with_index(index).up().build())
                .execute()
                .await
                .expect("set link up");
        }
        handle
            .address()
            .add(virt_index, IpAddr::V6(pod_addr), IPV6_HOST_PREFIX)
            .execute()
            .await
            .expect("assign pod address");
        let next_hops = gateways
            .iter()
            .map(|gateway| {
                let mut next_hop = netlink_packet_route::route::RouteNextHop::default();
                next_hop.interface_index = virt_index;
                next_hop
                    .attributes
                    .push(RouteAttribute::Gateway(RouteAddress::Inet6(*gateway)));
                next_hop
            })
            .collect();
        handle
            .route()
            .add(
                RouteMessageBuilder::<Ipv6Addr>::new()
                    .multipath(next_hops)
                    .build(),
            )
            .execute()
            .await
            .expect("add ECMP default route");

        let gateways = route_gateways(&handle, virt_index)
            .await
            .expect("collect gateways");
        let err = check_ipv6_routes(&handle, virt_index, &gateways)
            .await
            .expect_err("a multipath route must be refused");
        assert!(
            format!("{:#}", err).contains("multipath IPv6 route ::/0"),
            "unexpected error: {:#}",
            err
        );
    }
}
