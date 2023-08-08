// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::{ARPNeighbor, IPAddress, Interface, Route};
use anyhow::Result;
use async_trait::async_trait;
use netlink_packet_route::IFF_NOARP;

use super::NetworkInfo;
use crate::network::dan::DanDevice;
use crate::network::utils::address::{ip_family_from_ip_addr, parse_ip_cidr};

/// NetworkInfoFromDan is responsible for converting network info in JSON
/// to agent's network info.
#[derive(Debug)]
pub(crate) struct NetworkInfoFromDan {
    interface: Interface,
    routes: Vec<Route>,
    neighs: Vec<ARPNeighbor>,
}

impl NetworkInfoFromDan {
    pub async fn new(dan_device: &DanDevice) -> Result<Self> {
        let ip_addresses = dan_device
            .network_info
            .interface
            .ip_addresses
            .iter()
            .filter_map(|addr| {
                let (ipaddr, mask) = match parse_ip_cidr(addr) {
                    Ok(ip_cidr) => (ip_cidr.0, ip_cidr.1),
                    Err(_) => return None,
                };
                // Skip if it is a loopback address
                if ipaddr.is_loopback() {
                    return None;
                }

                Some(IPAddress {
                    family: ip_family_from_ip_addr(&ipaddr),
                    address: ipaddr.to_string(),
                    mask: format!("{}", mask),
                })
            })
            .collect();

        let interface = Interface {
            device: dan_device.name.clone(),
            name: dan_device.name.clone(),
            ip_addresses,
            mtu: dan_device.network_info.interface.mtu,
            hw_addr: dan_device.guest_mac.clone(),
            pci_addr: String::default(),
            field_type: dan_device.network_info.interface.ntype.clone(),
            raw_flags: dan_device.network_info.interface.flags & IFF_NOARP,
        };

        let routes = dan_device
            .network_info
            .routes
            .iter()
            .filter_map(|route| {
                let family = match route.ip_family() {
                    Ok(family) => family,
                    Err(_) => return None,
                };
                Some(Route {
                    dest: route.dest.clone(),
                    gateway: route.gateway.clone(),
                    device: dan_device.name.clone(),
                    source: route.source.clone(),
                    scope: route.scope,
                    family,
                })
            })
            .collect();

        let neighs = dan_device
            .network_info
            .neighbors
            .iter()
            .map(|neigh| {
                let to_ip_address = neigh.ip_address.as_ref().and_then(|ip_address| {
                    parse_ip_cidr(ip_address)
                        .ok()
                        .map(|(ipaddr, mask)| IPAddress {
                            family: ip_family_from_ip_addr(&ipaddr),
                            address: ipaddr.to_string(),
                            mask: format!("{}", mask),
                        })
                });

                ARPNeighbor {
                    to_ip_address,
                    device: dan_device.name.clone(),
                    ll_addr: neigh.hardware_addr.clone(),
                    state: neigh.state as i32,
                    flags: neigh.flags as i32,
                }
            })
            .collect();

        Ok(Self {
            interface,
            routes,
            neighs,
        })
    }
}

#[async_trait]
impl NetworkInfo for NetworkInfoFromDan {
    async fn interface(&self) -> Result<Interface> {
        Ok(self.interface.clone())
    }

    async fn routes(&self) -> Result<Vec<Route>> {
        Ok(self.routes.clone())
    }

    async fn neighs(&self) -> Result<Vec<ARPNeighbor>> {
        Ok(self.neighs.clone())
    }
}

#[cfg(test)]
mod tests {
    use agent::{ARPNeighbor, IPAddress, IPFamily, Interface, Route};

    use super::NetworkInfoFromDan;
    use crate::network::dan::{
        ARPNeighbor as DanARPNeighbor, DanDevice, Device, Interface as DanInterface,
        NetworkInfo as DanNetworkInfo, Route as DanRoute,
    };
    use crate::network::NetworkInfo;

    #[tokio::test]
    async fn test_network_info_from_dan() {
        let dan_device = DanDevice {
            name: "eth0".to_owned(),
            guest_mac: "xx:xx:xx:xx:xx".to_owned(),
            device: Device::HostTap {
                tap_name: "tap0".to_owned(),
                queue_num: 0,
                queue_size: 0,
            },
            network_info: DanNetworkInfo {
                interface: DanInterface {
                    ip_addresses: vec!["192.168.0.1/24".to_owned()],
                    mtu: 1500,
                    ntype: "tuntap".to_owned(),
                    flags: 0,
                },
                routes: vec![DanRoute {
                    dest: "172.18.0.0/16".to_owned(),
                    source: "172.18.0.1".to_owned(),
                    gateway: "172.18.31.1".to_owned(),
                    scope: 0,
                }],
                neighbors: vec![DanARPNeighbor {
                    ip_address: Some("192.168.0.3/16".to_owned()),
                    hardware_addr: "yy:yy:yy:yy:yy".to_owned(),
                    state: 0,
                    flags: 0,
                }],
            },
        };

        let network_info = NetworkInfoFromDan::new(&dan_device).await.unwrap();

        let interface = Interface {
            device: "eth0".to_owned(),
            name: "eth0".to_owned(),
            ip_addresses: vec![IPAddress {
                family: IPFamily::V4,
                address: "192.168.0.1".to_owned(),
                mask: "24".to_owned(),
            }],
            mtu: 1500,
            hw_addr: "xx:xx:xx:xx:xx".to_owned(),
            pci_addr: String::default(),
            field_type: "tuntap".to_owned(),
            raw_flags: 0,
        };
        assert_eq!(interface, network_info.interface().await.unwrap());

        let routes = vec![Route {
            dest: "172.18.0.0/16".to_owned(),
            gateway: "172.18.31.1".to_owned(),
            device: "eth0".to_owned(),
            source: "172.18.0.1".to_owned(),
            scope: 0,
            family: IPFamily::V4,
        }];
        assert_eq!(routes, network_info.routes().await.unwrap());

        let neighbors = vec![ARPNeighbor {
            to_ip_address: Some(IPAddress {
                family: IPFamily::V4,
                address: "192.168.0.3".to_owned(),
                mask: "16".to_owned(),
            }),
            device: "eth0".to_owned(),
            ll_addr: "yy:yy:yy:yy:yy".to_owned(),
            state: 0,
            flags: 0,
        }];
        assert_eq!(neighbors, network_info.neighs().await.unwrap());
    }
}
