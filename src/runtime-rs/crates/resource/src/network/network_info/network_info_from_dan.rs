// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::{ARPNeighbor, IPAddress, Interface, Route};
use anyhow::Result;
use async_trait::async_trait;
use netlink_packet_route::link::LinkFlags;
use tokio::sync::RwLock;

use super::NetworkInfo;
use crate::network::dan::{DanDevice, Device};
use crate::network::utils::address::{ip_family_from_ip_addr, parse_ip_cidr};

/// Matches the Go runtime's VfioEndpointType.
const VFIO_INTERFACE_TYPE: &str = "vfio";

/// NetworkInfoFromDan is responsible for converting network info in JSON
/// to agent's network info.
#[derive(Debug)]
pub(crate) struct NetworkInfoFromDan {
    interface: Interface,
    routes: Vec<Route>,
    neighs: Vec<ARPNeighbor>,
    /// Only known once the device is hot-plugged, so it is filled in later.
    device_path: RwLock<String>,
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
                    mask: format!("{mask}"),
                })
            })
            .collect();

        let field_type = match dan_device.device {
            Device::Vfio { .. } => VFIO_INTERFACE_TYPE.to_owned(),
            _ => dan_device.network_info.interface.ntype.clone(),
        };

        let interface = Interface {
            device: dan_device.name.clone(),
            name: dan_device.name.clone(),
            ip_addresses,
            mtu: dan_device.network_info.interface.mtu,
            hw_addr: dan_device.guest_mac.clone(),
            device_path: String::default(),
            field_type,
            raw_flags: dan_device.network_info.interface.flags & LinkFlags::Noarp.bits(),
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
                    flags: route.flags,
                    mtu: route.mtu,
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
                            mask: format!("{mask}"),
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
            device_path: RwLock::new(String::default()),
        })
    }
}

#[async_trait]
impl NetworkInfo for NetworkInfoFromDan {
    async fn interface(&self) -> Result<Interface> {
        let mut interface = self.interface.clone();
        interface.device_path = self.device_path.read().await.clone();
        Ok(interface)
    }

    async fn routes(&self) -> Result<Vec<Route>> {
        Ok(self.routes.clone())
    }

    async fn neighs(&self) -> Result<Vec<ARPNeighbor>> {
        Ok(self.neighs.clone())
    }

    async fn set_device_path(&self, path: String) -> Result<()> {
        *self.device_path.write().await = path;
        Ok(())
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
                    flags: 0,
                    mtu: 1450,
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
            device_path: String::default(),
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
            flags: 0,
            mtu: 1450,
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

    #[tokio::test]
    async fn test_network_info_from_dan_vfio() {
        let dan_device = DanDevice {
            name: "eth0".to_owned(),
            guest_mac: "0a:58:0a:0a:00:05".to_owned(),
            device: Device::Vfio {
                pci_device_id: "0000:85:02.5".to_owned(),
            },
            network_info: DanNetworkInfo {
                interface: DanInterface {
                    ip_addresses: vec!["10.10.0.5/24".to_owned()],
                    mtu: 1500,
                    ntype: String::new(),
                    flags: 0,
                },
                routes: vec![],
                neighbors: vec![],
            },
        };

        let network_info = NetworkInfoFromDan::new(&dan_device).await.unwrap();

        let interface = network_info.interface().await.unwrap();
        assert_eq!(interface.field_type, "vfio");
        assert!(interface.device_path.is_empty());

        network_info
            .set_device_path("02/03".to_owned())
            .await
            .unwrap();
        assert_eq!(network_info.interface().await.unwrap().device_path, "02/03");
    }
}
