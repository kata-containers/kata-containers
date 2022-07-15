// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use scopeguard::defer;

    use std::sync::Arc;

    use crate::network::{
        endpoint::IPVlanEndpoint,
        network_model::{
            tc_filter_model::{fetch_index, TcFilterModel},
            NetworkModelType,
        },
        network_pair::{NetworkInterface, NetworkPair, TapInterface},
    };

    // this unit test tests the integrity of IPVlanEndpoint::new()
    // by comparing the manual constructed object with object constructed by new()
    #[actix_rt::test]
    async fn test_ipvlan_construction() {
        let idx = 8192;
        let mac_addr = String::from("02:00:CA:FE:00:04");
        let manual_virt_iface_name = format!("eth{}", idx);
        let tap_iface_name = format!("tap{}_kata", idx); // create by kata

        if let Ok((conn, handle, _)) =
            rtnetlink::new_connection().context("failed to create new netlink connection")
        {
            let thread_handler = tokio::spawn(conn);
            defer!({
                thread_handler.abort();
            });

            // since IPVlanEndpoint::new() needs an EXISTING virt_iface (which is created
            // by containerd normally), we have to manually create a virt_iface.
            if let Ok(()) = handle
                .link()
                .add()
                .veth("foo".to_string(), manual_virt_iface_name.clone())
                .execute()
                .await
                .context("failed to create virt_iface")
            {
                if let Ok(mut result) = IPVlanEndpoint::new(&handle, "", idx, 5)
                    .await
                    .context("failed to create new IPVlan Endpoint")
                {
                    let manual = IPVlanEndpoint {
                        net_pair: NetworkPair {
                            tap: TapInterface {
                                id: String::from("uniqueTestID_kata"),
                                name: format!("br{}_kata", idx),
                                tap_iface: NetworkInterface {
                                    name: tap_iface_name.clone(),
                                    ..Default::default()
                                },
                            },
                            virt_iface: NetworkInterface {
                                name: manual_virt_iface_name.clone(),
                                hard_addr: mac_addr.clone(),
                                ..Default::default()
                            },
                            model: Arc::new(TcFilterModel::new().unwrap()), // impossible to panic
                            network_qos: false,
                        },
                    };

                    result.net_pair.tap.id = String::from("uniqueTestID_kata");
                    result.net_pair.tap.tap_iface.hard_addr = String::from("");
                    result.net_pair.virt_iface.hard_addr = mac_addr.clone();

                    // check the integrity by compare all variables
                    assert_eq!(manual.net_pair.tap.id, result.net_pair.tap.id);
                    assert_eq!(manual.net_pair.tap.name, result.net_pair.tap.name);
                    assert_eq!(
                        manual.net_pair.tap.tap_iface.name,
                        result.net_pair.tap.tap_iface.name
                    );
                    assert_eq!(
                        manual.net_pair.tap.tap_iface.hard_addr,
                        result.net_pair.tap.tap_iface.hard_addr
                    );
                    assert_eq!(
                        manual.net_pair.tap.tap_iface.addrs,
                        result.net_pair.tap.tap_iface.addrs
                    );
                    assert_eq!(
                        manual.net_pair.virt_iface.name,
                        result.net_pair.virt_iface.name
                    );
                    assert_eq!(
                        manual.net_pair.virt_iface.hard_addr,
                        result.net_pair.virt_iface.hard_addr
                    );
                    assert_eq!(
                        manual.net_pair.virt_iface.addrs,
                        result.net_pair.virt_iface.addrs
                    );
                    // using match branch to avoid deriving PartialEq trait
                    match manual.net_pair.model.model_type() {
                        NetworkModelType::TcFilter => {} // ok
                        _ => unreachable!(),
                    }
                    match result.net_pair.model.model_type() {
                        NetworkModelType::TcFilter => {}
                        _ => unreachable!(),
                    }
                    assert_eq!(manual.net_pair.network_qos, result.net_pair.network_qos);
                }
                if let Ok(link_index) = fetch_index(&handle, manual_virt_iface_name.as_str()).await
                {
                    assert!(handle.link().del(link_index).execute().await.is_ok())
                }
                if let Ok(link_index) = fetch_index(&handle, tap_iface_name.as_str()).await {
                    assert!(handle.link().del(link_index).execute().await.is_ok())
                }
            }
        }
    }
}
