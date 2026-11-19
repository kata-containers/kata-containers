// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::net::IpAddr;

    use anyhow::{ensure, Context, Result};
    use futures::TryStreamExt;
    use netlink_packet_route::tc::TcHandle;
    use rtnetlink::{Handle, LinkUnspec, LinkVeth};
    use scopeguard::defer;
    use test_utils::skip_if_not_root;

    use crate::network::{
        network_model::{
            l3_forwarding_model::fetch_index, L3_FORWARDING_NET_MODEL_STR, TC_FILTER_NET_MODEL_STR,
        },
        network_pair::NetworkPair,
    };

    /// Build a veth pair, bring both ends up, give the virt end an address, and return the
    /// virt end's index.
    ///
    /// The models operate on real links: `NetworkPair::new` looks the virt interface up by
    /// name, and the l3forwarding model refuses an interface with no addresses. So the pair
    /// has to exist, be up, and be addressed before the model runs.
    async fn create_virt(handle: &Handle, virt_name: &str) -> Result<u32> {
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

        Ok(virt_index)
    }

    /// Drive a network model through `add` and `del` on a fresh veth pair.
    async fn exercise_model(handle: &Handle, model: &str, idx: u32, virt_name: &str) -> Result<()> {
        let virt_index = create_virt(handle, virt_name).await?;

        let net_pair = NetworkPair::new(handle, idx, virt_name, model, 2)
            .await
            .context("new network pair")?;

        // tc_filter is the only model that installs qdiscs, so it is the only one whose `del`
        // has something to undo on the virt interface.
        let tc_filter = model == TC_FILTER_NET_MODEL_STR;

        net_pair
            .add_network_model()
            .await
            .context("add network model")?;
        if tc_filter {
            ensure!(
                has_ingress_qdisc(handle, virt_index).await?,
                "add did not install an ingress qdisc on {}",
                virt_name
            );
        }

        net_pair
            .del_network_model()
            .await
            .context("del network model")?;
        if tc_filter {
            ensure!(
                !has_ingress_qdisc(handle, virt_index).await?,
                "del left the ingress qdisc on {}",
                virt_name
            );
        }

        // A detach that fails after `del` is retried, and the retry calls `del` again. It
        // must succeed, and must not take an ingress qdisc somebody added in between.
        if tc_filter {
            add_ingress_qdisc(handle, virt_index).await?;
        }
        net_pair
            .del_network_model()
            .await
            .context("repeat del network model")?;
        if tc_filter {
            ensure!(
                has_ingress_qdisc(handle, virt_index).await?,
                "repeat del removed an ingress qdisc on {} added after the model's was gone",
                virt_name
            );
        }

        Ok(())
    }

    /// Remove tc_filter's ingress qdisc behind the model's back, then `del`. The model still
    /// counts the qdisc as its own, so `del` asks the kernel to delete one that is gone, and
    /// must take the answer as "already done".
    async fn del_after_qdisc_gone(handle: &Handle, idx: u32, virt_name: &str) -> Result<()> {
        let virt_index = create_virt(handle, virt_name).await?;

        let net_pair = NetworkPair::new(handle, idx, virt_name, TC_FILTER_NET_MODEL_STR, 2)
            .await
            .context("new network pair")?;
        net_pair
            .add_network_model()
            .await
            .context("add network model")?;

        remove_ingress_qdisc(handle, virt_index).await?;
        net_pair
            .del_network_model()
            .await
            .context("del network model after the qdisc was removed")?;

        Ok(())
    }

    /// Delete the virt interface after tc_filter attached, as a CNI that tears the pod network
    /// down first does. The qdisc went with the device, so `del` has nothing to undo and must
    /// succeed. It must also stop counting the qdisc as its own: a repeated `del` must leave
    /// alone an ingress qdisc on a new interface of the same name.
    async fn del_after_virt_gone(handle: &Handle, idx: u32, virt_name: &str) -> Result<()> {
        let virt_index = create_virt(handle, virt_name).await?;

        let net_pair = NetworkPair::new(handle, idx, virt_name, TC_FILTER_NET_MODEL_STR, 2)
            .await
            .context("new network pair")?;
        net_pair
            .add_network_model()
            .await
            .context("add network model")?;

        handle
            .link()
            .del(virt_index)
            .execute()
            .await
            .context("delete virt iface")?;
        net_pair
            .del_network_model()
            .await
            .context("del network model after the virt iface was removed")?;

        let virt_index = create_virt(handle, virt_name).await?;
        add_ingress_qdisc(handle, virt_index).await?;
        net_pair
            .del_network_model()
            .await
            .context("repeat del network model")?;
        ensure!(
            has_ingress_qdisc(handle, virt_index).await?,
            "repeat del removed an ingress qdisc on {} that the model did not add",
            virt_name
        );

        Ok(())
    }

    /// Put an ingress qdisc on the virt interface before tc_filter attaches, as a CNI or a
    /// user might. The attach must fail rather than take it over, and the `del` that the
    /// failed attach's cleanup runs must leave it in place.
    async fn keep_foreign_qdisc(handle: &Handle, idx: u32, virt_name: &str) -> Result<()> {
        let virt_index = create_virt(handle, virt_name).await?;
        add_ingress_qdisc(handle, virt_index).await?;

        let net_pair = NetworkPair::new(handle, idx, virt_name, TC_FILTER_NET_MODEL_STR, 2)
            .await
            .context("new network pair")?;

        ensure!(
            net_pair.add_network_model().await.is_err(),
            "add succeeded although {} already had an ingress qdisc",
            virt_name
        );
        net_pair
            .del_network_model()
            .await
            .context("del network model")?;
        ensure!(
            has_ingress_qdisc(handle, virt_index).await?,
            "del removed an ingress qdisc on {} that the model did not add",
            virt_name
        );

        Ok(())
    }

    /// Add an ingress qdisc to the interface the way somebody other than the model would.
    async fn add_ingress_qdisc(handle: &Handle, index: u32) -> Result<()> {
        handle
            .qdisc()
            .add(index as i32)
            .ingress()
            .execute()
            .await
            .context("add foreign ingress qdisc")
    }

    /// Remove the interface's ingress qdisc without going through the model.
    async fn remove_ingress_qdisc(handle: &Handle, index: u32) -> Result<()> {
        let mut req = handle.qdisc().del(index as i32);
        req.message_mut().header.parent = TcHandle::INGRESS;
        req.execute().await.context("remove ingress qdisc")
    }

    /// Whether the interface has an ingress qdisc. A qdisc dump covers every interface in the
    /// namespace, so filter on the interface index and the ingress parent here.
    async fn has_ingress_qdisc(handle: &Handle, index: u32) -> Result<bool> {
        let mut qdiscs = handle.qdisc().get().execute();
        while let Some(qdisc) = qdiscs.try_next().await.context("dump qdiscs")? {
            if qdisc.header.index == index as i32 && qdisc.header.parent == TcHandle::INGRESS {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Run a test scenario inside a throwaway network namespace, so the test does not create
    /// veth and tap devices on the host network.
    async fn run_in_netns<F, Fut>(ns_suffix: &str, scenario: F) -> Result<()>
    where
        F: FnOnce(Handle) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
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

        let result = scenario(handle).await;

        drop(guard);
        result
    }

    #[actix_rt::test]
    async fn test_tc_redirect_network() {
        skip_if_not_root!();
        run_in_netns("tcfilter", |handle| async move {
            exercise_model(&handle, TC_FILTER_NET_MODEL_STR, 1, "tcvirt").await
        })
        .await
        .expect("tc_filter network model");
    }

    #[actix_rt::test]
    async fn test_tc_keeps_foreign_ingress_qdisc() {
        skip_if_not_root!();
        run_in_netns("tcforeign", |handle| async move {
            keep_foreign_qdisc(&handle, 3, "tcforeign").await
        })
        .await
        .expect("tc_filter with a foreign ingress qdisc");
    }

    #[actix_rt::test]
    async fn test_tc_del_after_qdisc_gone() {
        skip_if_not_root!();
        run_in_netns("tcgone", |handle| async move {
            del_after_qdisc_gone(&handle, 4, "tcgone").await
        })
        .await
        .expect("tc_filter del after its qdisc was removed");
    }

    #[actix_rt::test]
    async fn test_tc_del_after_virt_gone() {
        skip_if_not_root!();
        run_in_netns("tcnovirt", |handle| async move {
            del_after_virt_gone(&handle, 5, "tcnovirt").await
        })
        .await
        .expect("tc_filter del after the virt interface was removed");
    }

    #[actix_rt::test]
    async fn test_l3_redirect_network() {
        skip_if_not_root!();
        run_in_netns("l3fwd", |handle| async move {
            exercise_model(&handle, L3_FORWARDING_NET_MODEL_STR, 2, "l3virt").await
        })
        .await
        .expect("l3forwarding network model");
    }
}
