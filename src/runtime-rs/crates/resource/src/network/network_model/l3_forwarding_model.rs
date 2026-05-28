// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
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

// TODO(l3-forwarding): outstanding work for production-readiness
//   - IPv6 support (currently bails on non-Ipv4 pod addresses).
use super::{NetworkModel, NetworkModelType};
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

        // Enable proxy arp so we can respond to ARP requests using the tap and eth0 interfaces.
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

        // Delete all the addresses on the tap interface, then add a link-local address so we can
        // receive l3 traffic on the tap.
        let mut addrs = handle
            .address()
            .get()
            .set_link_index_filter(tap_index)
            .execute();
        while let Some(addr) = addrs.try_next().await? {
            handle.address().del(addr).execute().await?;
        }
        let link_local_addr = Ipv4Addr::new(169, 254, 0, 1);
        handle
            .address()
            .add(tap_index, IpAddr::V4(link_local_addr), 32)
            .execute()
            .await?;

        // TODO support ip6.
        // In most cases, this will only have one address.
        let pod_addrs = pair
            .virt_iface
            .addrs
            .iter()
            .filter_map(|addr| match addr.addr {
                IpAddr::V4(v4) => Some(v4),
                IpAddr::V6(_) => None,
            })
            .collect::<Vec<_>>();
        if pod_addrs.is_empty() {
            anyhow::bail!("no ipv4 addresses found on virt iface");
        }

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
            if dst_matches && oif_matches {
                handle
                    .route()
                    .del(route)
                    .execute()
                    .await
                    .context("delete local route for pod ip on virt iface")?;
            }
        }

        // Add a route
        for pod_addr_v4 in pod_addrs {
            let route_msg = RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                .destination_prefix(pod_addr_v4, 32)
                .output_interface(tap_index)
                .scope(RouteScope::Link)
                .build();
            handle.route().add(route_msg).execute().await?;
        }

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

pub async fn fetch_index(handle: &Handle, name: &str) -> Result<u32> {
    let link = crate::network::network_pair::get_link_by_name(handle, name)
        .await
        .context("get link by name")?;
    let base = link.attrs();
    Ok(base.index)
}
