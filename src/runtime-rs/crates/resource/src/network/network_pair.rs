// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{convert::TryFrom, sync::Arc};

use anyhow::{anyhow, Context, Result};
use futures::stream::TryStreamExt;

use super::{
    network_model,
    utils::{self, address::Address, link},
};

const TAP_SUFFIX: &str = "_kata";

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct NetInterworkingModel(u32);

#[derive(Default, Debug, Clone)]
pub struct NetworkInterface {
    pub name: String,
    pub hard_addr: String,
    pub addrs: Vec<Address>,
}

#[derive(Default, Debug)]
pub struct TapInterface {
    pub id: String,
    pub name: String,
    pub tap_iface: NetworkInterface,
}
#[derive(Debug)]
pub struct NetworkPair {
    pub tap: TapInterface,
    pub virt_iface: NetworkInterface,
    pub model: Arc<dyn network_model::NetworkModel>,
    pub network_qos: bool,
}

impl NetworkPair {
    pub(crate) async fn new(
        handle: &rtnetlink::Handle,
        idx: u32,
        name: &str,
        model: &str,
        queues: usize,
    ) -> Result<Self> {
        let unique_id = kata_sys_util::rand::UUID::new();
        let model = network_model::new(model).context("new network model")?;
        let tap_iface_name = format!("tap{}{}", idx, TAP_SUFFIX);
        let virt_iface_name = format!("eth{}", idx);
        let tap_link = create_link(handle, &tap_iface_name, queues)
            .await
            .context("create link")?;

        let virt_link = get_link_by_name(handle, virt_iface_name.clone().as_str())
            .await
            .context("get link by name")?;

        let mut virt_addr_msg_list = handle
            .address()
            .get()
            .set_link_index_filter(virt_link.attrs().index)
            .execute();

        let mut virt_address = vec![];
        while let Some(addr_msg) = virt_addr_msg_list.try_next().await? {
            let addr = Address::try_from(addr_msg).context("get address from msg")?;
            virt_address.push(addr);
        }

        // Save the veth MAC address to the TAP so that it can later be used
        // to build the hypervisor command line. This MAC address has to be
        // the one inside the VM in order to avoid any firewall issues. The
        // bridge created by the network plugin on the host actually expects
        // to see traffic from this MAC address and not another one.
        let tap_hard_addr =
            utils::get_mac_addr(&virt_link.attrs().hardware_addr).context("get mac addr")?;

        // Save the TAP Mac address to the virt_iface so that it can later updated
        // the guest's gateway IP's mac as this TAP device. This MAC address has
        // to be inside the VM in order to the network reach to the gateway.
        let virt_hard_addr =
            utils::get_mac_addr(&tap_link.attrs().hardware_addr).context("get mac addr")?;

        handle
            .link()
            .set(tap_link.attrs().index)
            .mtu(virt_link.attrs().mtu)
            .execute()
            .await
            .context("set link mtu")?;

        handle
            .link()
            .set(tap_link.attrs().index)
            .up()
            .execute()
            .await
            .context("set link up")?;

        let mut net_pair = NetworkPair {
            tap: TapInterface {
                id: String::from(&unique_id),
                name: format!("br{}{}", idx, TAP_SUFFIX),
                tap_iface: NetworkInterface {
                    name: tap_iface_name,
                    hard_addr: tap_hard_addr,
                    ..Default::default()
                },
            },
            virt_iface: NetworkInterface {
                name: virt_iface_name,
                hard_addr: virt_hard_addr,
                addrs: virt_address,
            },
            model,
            network_qos: false,
        };

        if !name.is_empty() {
            net_pair.virt_iface.name = String::from(name);
        }

        Ok(net_pair)
    }

    pub(crate) async fn add_network_model(&self) -> Result<()> {
        let model = self.model.clone();
        model.add(self).await.context("add")?;
        Ok(())
    }

    pub(crate) async fn del_network_model(&self) -> Result<()> {
        let model = self.model.clone();
        model.del(self).await.context("del")?;
        Ok(())
    }
}

pub async fn create_link(
    handle: &rtnetlink::Handle,
    name: &str,
    queues: usize,
) -> Result<Box<dyn link::Link>> {
    link::create_link(name, link::LinkType::Tap, queues)?;

    let link = get_link_by_name(handle, name)
        .await
        .context("get link by name")?;

    let base = link.attrs();
    if base.master_index != 0 {
        handle
            .link()
            .set(base.index)
            .master(base.master_index)
            .execute()
            .await
            .context("set index")?;
    }
    Ok(link)
}

pub async fn get_link_by_name(
    handle: &rtnetlink::Handle,
    name: &str,
) -> Result<Box<dyn link::Link>> {
    let mut link_msg_list = handle.link().get().match_name(name.to_string()).execute();
    let msg = if let Some(msg) = link_msg_list.try_next().await? {
        msg
    } else {
        return Err(anyhow!("failed to find link by name {}", name));
    };

    Ok(link::get_link_from_message(msg))
}

#[cfg(test)]
mod tests {
    use scopeguard::defer;

    use super::*;
    use crate::network::network_model::TC_FILTER_NET_MODEL_STR;
    use test_utils::skip_if_not_root;
    use utils::link::net_test_utils::delete_link;

    // this ut tests create_link() and get_link_by_name()
    #[actix_rt::test]
    async fn test_utils() {
        skip_if_not_root!();

        if let Ok((conn, handle, _)) =
            rtnetlink::new_connection().context("failed to create netlink connection")
        {
            let thread_handler = tokio::spawn(conn);
            defer!({
                thread_handler.abort();
            });

            assert!(create_link(&handle, "kata_test_1", 2).await.is_ok());
            assert!(create_link(&handle, "kata_test_2", 3).await.is_ok());
            assert!(create_link(&handle, "kata_test_3", 4).await.is_ok());

            assert!(get_link_by_name(&handle, "kata_test_1").await.is_ok());
            assert!(get_link_by_name(&handle, "kata_test_2").await.is_ok());
            assert!(get_link_by_name(&handle, "kata_test_3").await.is_ok());

            assert!(delete_link(&handle, "kata_test_1").await.is_ok());
            assert!(delete_link(&handle, "kata_test_2").await.is_ok());
            assert!(delete_link(&handle, "kata_test_3").await.is_ok());

            assert!(get_link_by_name(&handle, "kata_test_1").await.is_err());
            assert!(get_link_by_name(&handle, "kata_test_2").await.is_err());
            assert!(get_link_by_name(&handle, "kata_test_3").await.is_err());
        }
    }

    #[actix_rt::test]
    async fn test_network_pair() {
        let idx = 123456;
        let virt_iface_name = format!("eth{}", idx);
        let tap_name = format!("tap{}{}", idx, TAP_SUFFIX);
        let queues = 2;
        let model = TC_FILTER_NET_MODEL_STR;

        skip_if_not_root!();

        if let Ok((conn, handle, _)) =
            rtnetlink::new_connection().context("failed to create netlink connection")
        {
            let thread_handler = tokio::spawn(conn);
            defer!({
                thread_handler.abort();
            });
            // the network pair has not been created
            assert!(get_link_by_name(&handle, virt_iface_name.as_str())
                .await
                .is_err());

            // mock containerd to create one end of the network pair
            assert!(create_link(&handle, virt_iface_name.as_str(), queues)
                .await
                .is_ok());

            if let Ok(_pair) = NetworkPair::new(&handle, idx, "", model, queues).await {
                // the pair is created, we can find the two ends of network pair
                assert!(get_link_by_name(&handle, virt_iface_name.as_str())
                    .await
                    .is_ok());
                assert!(get_link_by_name(&handle, tap_name.as_str()).await.is_ok());

                //delete the link created in test
                assert!(delete_link(&handle, virt_iface_name.as_str()).await.is_ok());
                assert!(delete_link(&handle, tap_name.as_str()).await.is_ok());
            }
        }
    }
}
