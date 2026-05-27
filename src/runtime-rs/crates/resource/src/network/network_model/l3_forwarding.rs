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
//   - Replace remaining .unwrap()s with proper error handling.
//   - Implement del() (see TODO inside it).
//   - Unhardcode "tap0_kata" / "eth0" in the proxy_arp sysctl writes.
//   - IPv6 support (currently bails on non-Ipv4 pod addresses).
//   - Document the cilium interaction: this model is incompatible with
//     kube-proxy-replacement=true (socketLB / per-veth tc-eBPF service DNAT
//     either bypass or get confused by kata's networking). Deployments must
//     run real kube-proxy with cilium's KPR set to "false" or "partial".
//   - Decide whether the ingress qdisc on virt_index is still needed; it was
//     inherited from the tcfilter model and isn't used by this datapath.
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

        handle
            .qdisc()
            .add(tap_index as i32)
            .ingress()
            .execute()
            .await
            .context("add tap ingress")?;

        handle
            .qdisc()
            .add(virt_index as i32)
            .ingress()
            .execute()
            .await
            .context("add virt ingress")?;

        // TODO: don't hardcode interface names; use pair.tap.tap_iface.name and
        // pair.virt_iface.name. Consider per-neighbour proxy entries via
        // handle.neighbours().add() with NeighbourFlags::Proxy instead of the
        // global per-interface sysctl.
        fs::write("/proc/sys/net/ipv4/conf/tap0_kata/proxy_arp", "1")
            .context("enable proxy arp")?;
        fs::write("/proc/sys/net/ipv4/conf/eth0/proxy_arp", "1")
            .context("enable proxy arp")?;
        fs::write("/proc/sys/net/ipv4/ip_forward", "1")
            .context("enable ip forward")?;


        // NOTE: we intentionally filter by tap_index (which has no addresses to
        // strip). The CNI-assigned pod IP stays on the virt iface (eth0) inside
        // the pod netns so cilium still recognises the endpoint and its tc-eBPF
        // programs (service DNAT, policy, identity) keep working. L3 forwarding
        // is achieved purely via routing + proxy_arp; we don't need to remove
        // the pod IP from eth0.
        let mut addrs = handle.address().get().set_link_index_filter(tap_index).execute();
        while let Some(addr) = addrs.try_next().await? {
            handle.address().del(addr).execute().await?;
        }
        // add a link local address
        let link_local_addr = Ipv4Addr::new(169, 254, 0, 1);
        handle
            .address()
            .add(tap_index, IpAddr::V4(link_local_addr), 32)
            .execute()
            .await?;

        let pod_addr = pair.virt_iface.addrs.first().unwrap().addr; // TODO: handle missing addr instead of panicking; also handle multiple addrs / IPv6

        // Remove the auto-created `local <pod_ip> dev eth0` entry from the
        // local routing table. The CNI-assigned pod IP stays on eth0 (so cilium
        // still recognises the endpoint), but the kernel auto-installed a
        // matching `local` entry on eth0 which makes proxy_arp on tap0_kata
        // reject ARP replies (the kernel treats the guest's own pod IP as a
        // "martian" source when it arrives via tap0_kata while also being
        // configured as local on eth0). Deleting just the local-table entry
        // (not the address itself) is enough to make proxy_arp work while
        // preserving the address on eth0 for cilium.
        let pod_addr_v4 = match pod_addr {
            IpAddr::V4(v4) => v4,
            _ => anyhow::bail!("unsupported pod address type"),
        };
        // RT_TABLE_LOCAL = 255 (RouteHeader has no constant for it in this crate version).
        const RT_TABLE_LOCAL: u8 = 255;
        let local_query = RouteMessageBuilder::<std::net::Ipv4Addr>::new()
            .table_id(RT_TABLE_LOCAL as u32)
            .build();
        let mut local_routes = handle.route().get(local_query).execute();
        while let Some(route) = local_routes.try_next().await? {
            if route.header.table != RT_TABLE_LOCAL {
                continue;
            }
            let mut dst_matches = false;
            let mut oif_matches = false;
            for attr in &route.attributes {
                match attr {
                    RouteAttribute::Destination(RouteAddress::Inet(v4))
                        if *v4 == pod_addr_v4 =>
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
        let route_msg = RouteMessageBuilder::<std::net::Ipv4Addr>::new()
            .destination_prefix(pod_addr_v4, 32)
            .output_interface(tap_index)
            .scope(RouteScope::Link)
            .build();
        handle.route().add(route_msg).execute().await?;

        Ok(())
    }

    async fn del(&self, pair: &NetworkPair) -> Result<()> {
        // TODO: implement full teardown. Currently only removes the virt
        // ingress qdisc. Should also:
        //   - remove the tap ingress qdisc
        //   - remove the link-local 169.254.0.1/32 address from the tap
        //   - remove the /32 route for the pod IP via the tap
        //   - revert proxy_arp on tap and virt (track previous values)
        //   - revert ip_forward (only if we changed it; track previous value)
        // Skipping these is mostly fine today because the pod netns is torn
        // down with the sandbox, but the host-level ip_forward sysctl leaks.
        let (connection, handle, _) = rtnetlink::new_connection().context("new connection")?;
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });
        let virt_index = fetch_index(&handle, &pair.virt_iface.name).await?;
        handle.qdisc().del(virt_index as i32).execute().await?;
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
