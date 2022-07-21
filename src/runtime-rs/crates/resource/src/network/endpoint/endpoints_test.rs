// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(test)]
mod tests {
    use anyhow::Context;
    use netlink_packet_route::MACVLAN_MODE_PRIVATE;
    use scopeguard::defer;

    use std::sync::Arc;

    use crate::network::{
        endpoint::{IPVlanEndpoint, MacVlanEndpoint, VlanEndpoint},
        network_model::{
            self,
            tc_filter_model::{fetch_index, TcFilterModel},
            NetworkModelType, TC_FILTER_NET_MODEL_STR,
        },
        network_pair::{NetworkInterface, NetworkPair, TapInterface},
    };

    // this unit test tests the integrity of MacVlanEndpoint::new()
    #[actix_rt::test]
    async fn test_vlan_construction() {
        let idx = 8193;
        let mac_addr = String::from("02:78:CA:FE:00:04");
        let manual_vlan_iface_name = format!("eth{}", idx);
        let tap_iface_name = format!("tap{}_kata", idx); // create by NetworkPair::new()
        let dummy_name = format!("dummy{}", idx);
        let vlanid = 123;

        if let Ok((conn, handle, _)) =
            rtnetlink::new_connection().context("failed to create netlink connection")
        {
            let thread_handler = tokio::spawn(conn);
            defer!({
                thread_handler.abort();
            });

            if let Ok(()) = handle
                .link()
                .add()
                .dummy(dummy_name.clone())
                .execute()
                .await
                .context("failed to create dummy link")
            {
                let dummy_index = fetch_index(&handle, dummy_name.clone().as_str())
                    .await
                    .expect("failed to get the index of dummy link");

                // since IPVlanEndpoint::new() needs an EXISTING virt_iface (which is created
                // by containerd normally), we have to manually create a virt_iface.
                if let Ok(()) = handle
                    .link()
                    .add()
                    .vlan(manual_vlan_iface_name.clone(), dummy_index, vlanid)
                    .execute()
                    .await
                    .context("failed to create manual veth pair")
                {
                    if let Ok(mut result) = VlanEndpoint::new(&handle, "", idx, 5)
                        .await
                        .context("failed to create new ipvlan endpoint")
                    {
                        let manual = VlanEndpoint {
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
                                    name: manual_vlan_iface_name.clone(),
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
                    let link_index = fetch_index(&handle, manual_vlan_iface_name.as_str())
                        .await
                        .expect("failed to fetch index");
                    assert!(handle.link().del(link_index).execute().await.is_ok());
                    let link_index = fetch_index(&handle, tap_iface_name.as_str())
                        .await
                        .expect("failed to fetch index");
                    assert!(handle.link().del(link_index).execute().await.is_ok());
                    assert!(handle.link().del(dummy_index).execute().await.is_ok());
                }
            }
        }
    }

    // this unit test tests the integrity of VlanEndpoint::new()
    #[actix_rt::test]
    async fn test_macvlan_construction() {
        let idx = 8194;
        let mac_addr = String::from("02:25:CA:FE:00:04");
        let manual_macvlan_iface_name = format!("eth{}", idx);
        let tap_iface_name = format!("tap{}_kata", idx); // create by NetworkPair::new()
        let model_str = TC_FILTER_NET_MODEL_STR;
        let dummy_name = format!("dummy{}", idx);

        if let Ok((conn, handle, _)) =
            rtnetlink::new_connection().context("failed to create netlink connection")
        {
            let thread_handler = tokio::spawn(conn);
            defer!({
                thread_handler.abort();
            });

            if let Ok(()) = handle
                .link()
                .add()
                .dummy(dummy_name.clone())
                .execute()
                .await
                .context("failed to create dummy link")
            {
                let dummy_index = fetch_index(&handle, dummy_name.clone().as_str())
                    .await
                    .expect("failed to get the index of dummy link");

                // the mode here does not matter, could be any of available modes
                if let Ok(()) = handle
                    .link()
                    .add()
                    .macvlan(
                        manual_macvlan_iface_name.clone(),
                        dummy_index,
                        MACVLAN_MODE_PRIVATE,
                    )
                    .execute()
                    .await
                    .context("failed to create manual macvlan pair")
                {
                    // model here does not matter, could be any of supported models
                    if let Ok(mut result) = MacVlanEndpoint::new(
                        &handle,
                        manual_macvlan_iface_name.clone().as_str(),
                        idx,
                        model_str,
                        5,
                    )
                    .await
                    .context("failed to create new macvlan endpoint")
                    {
                        let manual = MacVlanEndpoint {
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
                                    name: manual_macvlan_iface_name.clone(),
                                    hard_addr: mac_addr.clone(),
                                    ..Default::default()
                                },
                                model: network_model::new(model_str)
                                    .expect("failed to create new network model"),
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
                        // using match branch to avoid deriving PartialEq trait
                        // TcFilter model is hard-coded "model_str" variable
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
                    // delete the manually created links
                    let link_index = fetch_index(&handle, manual_macvlan_iface_name.as_str())
                        .await
                        .expect("failed to fetch index");
                    assert!(handle.link().del(link_index).execute().await.is_ok());
                    let link_index = fetch_index(&handle, tap_iface_name.as_str())
                        .await
                        .expect("failed to fetch index");
                    assert!(handle.link().del(link_index).execute().await.is_ok());
                    assert!(handle.link().del(dummy_index).execute().await.is_ok());
                }
            }
        }
    }

    // this unit test tests the integrity of IPVlanEndpoint::new()
    #[actix_rt::test]
    async fn test_ipvlan_construction() {
        let idx = 8192;
        let mac_addr = String::from("02:00:CA:FE:00:04");
        let manual_virt_iface_name = format!("eth{}", idx);
        let tap_iface_name = format!("tap{}_kata", idx); // create by kata

        if let Ok((conn, handle, _)) =
            rtnetlink::new_connection().context("failed to create netlink connection")
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
                .context("failed to create manual veth pair")
            {
                if let Ok(mut result) = IPVlanEndpoint::new(&handle, "", idx, 5)
                    .await
                    .context("failed to create new ipvlan endpoint")
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
                let link_index = fetch_index(&handle, manual_virt_iface_name.as_str())
                    .await
                    .expect("failed to fetch index");
                assert!(handle.link().del(link_index).execute().await.is_ok());
                let link_index = fetch_index(&handle, tap_iface_name.as_str())
                    .await
                    .expect("failed to fetch index");
                assert!(handle.link().del(link_index).execute().await.is_ok());
            }
        }
    }
}
