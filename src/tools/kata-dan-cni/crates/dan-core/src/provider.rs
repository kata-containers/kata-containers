// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PodIdentity {
    pub namespace: String,
    pub name: String,
    pub uid: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSelection {
    /// Name of the CNI network configuration for this attachment.
    pub network_name: String,
    pub if_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacAddress(pub [u8; 6]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Route {
    pub destination: IpAddr,
    pub prefix_length: u8,
    pub gateway: Option<IpAddr>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IpAssignment {
    pub address: IpAddr,
    pub prefix_length: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkAllocation {
    pub mac_address: MacAddress,
    pub addresses: Vec<IpAssignment>,
    pub routes: Vec<Route>,
    pub mtu: u32,
}

impl NetworkAllocation {
    /// Validate the allocation before creating host resources.
    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.addresses.is_empty() || self.mtu == 0 {
            return Err(ProviderError::InvalidData(
                "network allocation requires IP addresses and a positive MTU".into(),
            ));
        }
        if self.mac_address.0 == [0; 6] || self.mac_address.0[0] & 1 != 0 {
            return Err(ProviderError::InvalidData(
                "network allocation requires a unicast MAC address".into(),
            ));
        }
        for ip in &self.addresses {
            if ip.prefix_length > maximum_prefix(ip.address) {
                return Err(ProviderError::InvalidData(
                    "IP prefix length is out of range".into(),
                ));
            }
            if ip.address.is_ipv6() && self.mtu < 1280 {
                return Err(ProviderError::InvalidData(
                    "IPv6 requires an MTU of at least 1280".into(),
                ));
            }
        }
        for route in &self.routes {
            if route.prefix_length > maximum_prefix(route.destination)
                || route
                    .gateway
                    .is_some_and(|gw| gw.is_ipv4() != route.destination.is_ipv4())
                || !self
                    .addresses
                    .iter()
                    .any(|ip| ip.address.is_ipv4() == route.destination.is_ipv4())
            {
                return Err(ProviderError::InvalidData(
                    "route prefix or address family does not match the allocation".into(),
                ));
            }
        }
        Ok(())
    }
}

const fn maximum_prefix(address: IpAddr) -> u8 {
    if address.is_ipv4() {
        32
    } else {
        128
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalPort {
    pub name: String,
    pub host_binding: HostPortBinding,
}

/// Host-side properties needed to bind an endpoint to a logical port.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostPortBinding {
    pub encapsulation_ip: Option<IpAddr>,
}

/// Host-side traffic settings, separate from the Guest's IP configuration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HostPortConfig {
    pub pod_name: String,
    pub pod_namespace: String,
    pub ingress_rate_mbps: u64,
    pub egress_rate_mbps: u64,
    pub netem: NetemConfig,
    pub mirror: bool,
}

impl HostPortConfig {
    pub fn validate(&self) -> Result<(), ProviderError> {
        for (name, value, multiplier) in [
            ("ingress rate", self.ingress_rate_mbps, 1_000_000),
            ("egress rate", self.egress_rate_mbps, 1_000_000),
            ("latency", self.netem.latency_ms, 1_000),
            ("jitter", self.netem.jitter_ms, 1_000),
            ("packet limit", self.netem.limit_packets, 1),
        ] {
            if value > i64::MAX as u64 / multiplier {
                return Err(ProviderError::InvalidData(format!(
                    "host port {name} exceeds the supported range after unit conversion"
                )));
            }
        }
        if !self.netem.loss_percent.is_finite() || !(0.0..=100.0).contains(&self.netem.loss_percent)
        {
            return Err(ProviderError::InvalidData(
                "host port packet loss must be finite and between 0 and 100 percent".into(),
            ));
        }
        Ok(())
    }

    pub fn has_qos(&self) -> bool {
        self.ingress_rate_mbps != 0 || self.egress_rate_mbps != 0 || !self.netem.is_empty()
    }

    /// Pod identity alone does not request any host traffic policy.
    pub fn is_empty(&self) -> bool {
        !self.has_qos() && !self.mirror
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetemConfig {
    pub latency_ms: u64,
    pub jitter_ms: u64,
    pub limit_packets: u64,
    pub loss_percent: f64,
}

impl NetemConfig {
    pub fn is_empty(&self) -> bool {
        self.latency_ms == 0
            && self.jitter_ms == 0
            && self.limit_packets == 0
            && self.loss_percent == 0.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderError {
    NotReady(String),
    InvalidData(String),
    RequestFailed(String),
}

impl Display for ProviderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotReady(message) => write!(formatter, "provider data is not ready: {message}"),
            Self::InvalidData(message) => write!(formatter, "invalid provider data: {message}"),
            Self::RequestFailed(message) => write!(formatter, "provider request failed: {message}"),
        }
    }
}

impl Error for ProviderError {}

pub type ProviderFuture<'a, T = (NetworkAllocation, LogicalPort)> =
    Pin<Box<dyn Future<Output = Result<T, ProviderError>> + Send + 'a>>;

/// Read-only resolver for an existing allocation and logical port.
/// Callers must validate the allocation before creating host resources.
pub trait Provider: Send + Sync {
    fn resolve<'a>(
        &'a self,
        identity: &'a PodIdentity,
        selection: &'a NetworkSelection,
    ) -> ProviderFuture<'a>;

    fn resolve_with_port_config<'a>(
        &'a self,
        identity: &'a PodIdentity,
        selection: &'a NetworkSelection,
    ) -> ProviderFuture<'a, (NetworkAllocation, LogicalPort, HostPortConfig)> {
        Box::pin(async move {
            let (allocation, port) = self.resolve(identity, selection).await?;
            Ok((allocation, port, HostPortConfig::default()))
        })
    }

    /// Check that the selected logical port and settings still match the Provider.
    fn verify_port_config<'a>(
        &'a self,
        _identity: &'a PodIdentity,
        _selection: &'a NetworkSelection,
        _expected_port: &'a LogicalPort,
        _expected: &'a HostPortConfig,
    ) -> ProviderFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        BackendCapabilities, IpAssignment, MacAddress, NetworkAllocation, ProviderError, Route,
    };
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    fn ipv4_allocation() -> NetworkAllocation {
        NetworkAllocation {
            mac_address: MacAddress([0x02, 0, 0, 0, 0, 2]),
            addresses: vec![IpAssignment {
                address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
                prefix_length: 24,
            }],
            routes: vec![Route {
                destination: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                prefix_length: 0,
                gateway: Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))),
            }],
            mtu: 1500,
        }
    }

    #[test]
    fn allocation_capabilities() {
        let mut allocation = ipv4_allocation();
        allocation.addresses.push(IpAssignment {
            address: "fd00::2".parse().expect("IPv6 fixture"),
            prefix_length: 64,
        });
        allocation.routes.push(Route {
            destination: IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            prefix_length: 0,
            gateway: Some("fd00::1".parse().expect("IPv6 gateway")),
        });
        assert_eq!(allocation.validate(), Ok(()));
        assert!(BackendCapabilities {
            ipv4: true,
            ipv6: true
        }
        .validate(&allocation)
        .is_ok());
        for capabilities in [
            BackendCapabilities {
                ipv4: true,
                ipv6: false,
            },
            BackendCapabilities {
                ipv4: false,
                ipv6: true,
            },
        ] {
            assert!(capabilities.validate(&allocation).is_err());
        }
        allocation.mtu = 1280;
        assert!(allocation.validate().is_ok());
        allocation.mtu = 1279;
        assert!(matches!(
            allocation.validate(),
            Err(ProviderError::InvalidData(_))
        ));
    }

    #[test]
    fn reject_invalid_allocation() {
        let mutations: &[fn(&mut NetworkAllocation)] = &[
            |allocation| allocation.addresses.clear(),
            |allocation| allocation.mtu = 0,
            |allocation| allocation.mac_address = MacAddress([0; 6]),
            |allocation| allocation.mac_address = MacAddress([1, 0, 0, 0, 0, 1]),
            |allocation| allocation.addresses[0].prefix_length = 33,
            |allocation| allocation.routes[0].prefix_length = 33,
            |allocation| allocation.routes[0].gateway = Some(IpAddr::V6(Ipv6Addr::LOCALHOST)),
            |allocation| {
                allocation.routes.push(Route {
                    destination: IpAddr::V6(Ipv6Addr::UNSPECIFIED),
                    prefix_length: 0,
                    gateway: None,
                })
            },
            |allocation| {
                allocation.addresses.push(IpAssignment {
                    address: IpAddr::V6(Ipv6Addr::LOCALHOST),
                    prefix_length: 129,
                })
            },
        ];
        assert!(ipv4_allocation().validate().is_ok());
        for (index, mutate) in mutations.iter().enumerate() {
            let mut allocation = ipv4_allocation();
            mutate(&mut allocation);
            assert!(
                matches!(allocation.validate(), Err(ProviderError::InvalidData(_))),
                "mutation {index}"
            );
        }
    }
}
