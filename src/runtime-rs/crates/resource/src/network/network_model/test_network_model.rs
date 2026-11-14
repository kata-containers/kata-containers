// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use anyhow::{Context, Result};
    use rtnetlink::{Handle, LinkUnspec, LinkVeth};
    use scopeguard::defer;
    use test_utils::skip_if_not_root;

    use crate::network::{
        network_model::{
            l3_forwarding_model::fetch_index, L3_FORWARDING_NET_MODEL_STR, TC_FILTER_NET_MODEL_STR,
        },
        network_pair::NetworkPair,
    };

    /// Build a veth pair, give the virt end an address, and drive a network model through
    /// `add` and `del`.
    ///
    /// The models operate on real links: `NetworkPair::new` looks the virt interface up by
    /// name, and the l3forwarding model refuses an interface with no addresses. So the pair
    /// has to exist, be up, and be addressed before the model runs.
    async fn exercise_model(handle: &Handle, model: &str, idx: u32, virt_name: &str) -> Result<()> {
        let peer_name = format!("{}-peer", virt_name);

        handle
            .link()
            .add(LinkVeth::new(virt_name, &peer_name).build())
            .execute()
            .await
            .context("create veth pair")?;

        for name in [virt_name, peer_name.as_str()] {
            let index = fetch_index(handle, name)
                .await
                .with_context(|| format!("fetch index of {}", name))?;
            handle
                .link()
                .set(LinkUnspec::new_with_index(index).up().build())
                .execute()
                .await
                .with_context(|| format!("set {} up", name))?;
        }

        let virt_index = fetch_index(handle, virt_name)
            .await
            .context("fetch virt index")?;
        handle
            .address()
            .add(virt_index, "10.244.0.8".parse::<IpAddr>().unwrap(), 24)
            .execute()
            .await
            .context("assign address to virt iface")?;

        let net_pair = NetworkPair::new(handle, idx, virt_name, model, 2)
            .await
            .context("new network pair")?;

        net_pair
            .add_network_model()
            .await
            .context("add network model")?;
        net_pair
            .del_network_model()
            .await
            .context("del network model")?;

        Ok(())
    }

    /// Run `exercise_model` inside a throwaway network namespace, so the test does not create
    /// veth and tap devices on the host network.
    async fn run_in_netns(model: &str, idx: u32, virt_name: &str, ns_suffix: &str) -> Result<()> {
        let ns_name = format!("kata-{}-{}", ns_suffix, std::process::id());
        let netns = scopeguard::guard(
            netns_rs::NetNs::new(&ns_name).context("create netns")?,
            |netns| {
                let _ = netns.remove();
            },
        );

        let guard = kata_sys_util::netns::NetnsGuard::new(
            netns.path().to_str().context("netns path is not utf-8")?,
        )
        .context("enter netns")?;

        let (connection, handle, _) = rtnetlink::new_connection().context("new connection")?;
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });

        let result = exercise_model(&handle, model, idx, virt_name).await;

        drop(guard);
        result
    }

    #[actix_rt::test]
    async fn test_tc_redirect_network() {
        skip_if_not_root!();
        run_in_netns(TC_FILTER_NET_MODEL_STR, 1, "tcvirt", "tcfilter")
            .await
            .expect("tc_filter network model");
    }

    #[actix_rt::test]
    async fn test_l3_redirect_network() {
        skip_if_not_root!();
        run_in_netns(L3_FORWARDING_NET_MODEL_STR, 2, "l3virt", "l3fwd")
            .await
            .expect("l3forwarding network model");
    }
}
