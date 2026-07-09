// Copyright (c) 2026 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::net::{IpAddr, Ipv4Addr};

use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::TryStreamExt;
use netlink_packet_route::route::{RouteAddress, RouteAttribute, RouteScope};
use rtnetlink::{Handle, RouteMessageBuilder};
use scopeguard::defer;

use super::{
    port_forwarding::{configure_port_forwarding, TAP_IPV4_ADDR},
    NetworkModel, NetworkModelType,
};
use crate::network::NetworkPair;

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

        let pod_addrs =
            ipv4_workload_addresses(pair.virt_iface.addrs.iter().map(|address| address.addr))?;
        if pod_addrs.is_empty() {
            anyhow::bail!("no IP addresses found on virt iface");
        }
        let pod_ipv4 = pod_addrs.first().copied();

        // Enable proxy arp so we can respond to ARP requests using the tap and virt interfaces.
        fs::write(
            format!(
                "/proc/sys/net/ipv4/conf/{}/proxy_arp",
                pair.tap.tap_iface.name
            ),
            "1",
        )
        .context("enable proxy arp")?;
        fs::write(
            format!("/proc/sys/net/ipv4/conf/{}/proxy_arp", pair.virt_iface.name),
            "1",
        )
        .context("enable proxy arp")?;
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
        const ROUTE_TABLE_LOCAL: u8 = 255;
        let local_query = RouteMessageBuilder::<std::net::Ipv4Addr>::new()
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
                    RouteAttribute::Destination(RouteAddress::Inet(v4))
                        if pod_addrs.contains(v4) =>
                    {
                        dst_matches = true;
                    }
                    RouteAttribute::Oif(idx) if *idx == virt_index => {
                        oif_matches = true;
                    }
                    _ => {}
                }
            }
            if dst_matches && oif_matches && route.header.destination_prefix_length == 32 {
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
            let route_msg = RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                .destination_prefix(pod_addr, 32)
                .output_interface(tap_index)
                .scope(RouteScope::Link)
                .build();
            ignore_eexist(handle.route().add(route_msg).execute().await)
                .with_context(|| format!("add route for pod ip {pod_addr} via tap"))?;
        }

        configure_port_forwarding(&pair.tap.tap_iface.name, pod_ipv4, TAP_IPV4_ADDR).await;

        Ok(())
    }

    async fn del(&self, _pair: &NetworkPair) -> Result<()> {
        // Nothing to do: every resource added by `add()` lives inside the
        // pod netns (link-local addr on tap, /32 route via tap, proxy_arp
        // on tap+virt, ip_forward sysctl) and is destroyed when the netns
        // is torn down with the sandbox.
        Ok(())
    }
}

fn ipv4_workload_addresses(addresses: impl IntoIterator<Item = IpAddr>) -> Result<Vec<Ipv4Addr>> {
    let mut ipv4_addresses = Vec::new();
    for address in addresses {
        match address {
            IpAddr::V4(address) => ipv4_addresses.push(address),
            IpAddr::V6(address) if address.is_unicast_link_local() => {}
            IpAddr::V6(address) => anyhow::bail!(
                "l3forwarding does not support IPv6 yet, but the virt iface has {}",
                address
            ),
        }
    }
    Ok(ipv4_addresses)
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

    #[test]
    fn ignores_ipv6_link_local_addresses() {
        let addresses =
            ipv4_workload_addresses(["10.244.0.8".parse().unwrap(), "fe80::1234".parse().unwrap()])
                .unwrap();

        assert_eq!(addresses, vec!["10.244.0.8".parse::<Ipv4Addr>().unwrap()]);
    }

    #[test]
    fn rejects_non_link_local_ipv6_addresses() {
        let error = ipv4_workload_addresses(["fd00::8".parse().unwrap()]).unwrap_err();

        assert_eq!(
            error.to_string(),
            "l3forwarding does not support IPv6 yet, but the virt iface has fd00::8"
        );
    }
}
