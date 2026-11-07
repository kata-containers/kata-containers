// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::num::IntErrorKind;

use dan_core::{HostPortConfig, IpAssignment, MacAddress, NetemConfig, ProviderError, Route};
use serde::Deserialize;

const ALLOCATED: &str = "allocated";
const ROUTED: &str = "routed";
const IP_ADDRESS: &str = "ip_address";
const MAC_ADDRESS: &str = "mac_address";
const CIDR: &str = "cidr";
const GATEWAY: &str = "gateway";
const LOGICAL_SWITCH: &str = "logical_switch";
const DEFAULT_ROUTE: &str = "default_route";
const ROUTES: &str = "routes";
const PROVIDER_NETWORK: &str = "provider_network";
const INGRESS_RATE: &str = "ingress_rate";
const EGRESS_RATE: &str = "egress_rate";
const LATENCY: &str = "latency";
const JITTER: &str = "jitter";
const LIMIT: &str = "limit";
const LOSS: &str = "loss";
const MIRROR: &str = "mirror";

pub(crate) fn parse_port_config(
    annotations: &BTreeMap<String, String>,
    provider: &str,
    if_name: &str,
) -> Result<HostPortConfig, ProviderError> {
    let prefix = annotation_provider(annotations, provider, if_name);
    let loss_key = key(&prefix, LOSS);
    let loss_percent = annotations
        .get(&loss_key)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or_default();
    if !loss_percent.is_finite() {
        return Err(ProviderError::InvalidData(format!(
            "Pod annotation `{loss_key}` must be finite"
        )));
    }
    let config = HostPortConfig {
        ingress_rate_mbps: parse_rate(annotations, &key(&prefix, INGRESS_RATE))?,
        egress_rate_mbps: parse_rate(annotations, &key(&prefix, EGRESS_RATE))?,
        netem: NetemConfig {
            latency_ms: parse_netem_integer(annotations, &key(&prefix, LATENCY))?,
            jitter_ms: parse_netem_integer(annotations, &key(&prefix, JITTER))?,
            limit_packets: parse_netem_integer(annotations, &key(&prefix, LIMIT))?,
            // Kube-OVN ignores nonpositive Netem settings.
            loss_percent: loss_percent.max(0.0),
        },
        mirror: annotations.get(&key(provider, MIRROR)).map(String::as_str) == Some("true"),
        ..HostPortConfig::default()
    };
    config.validate()?;
    Ok(config)
}

fn parse_rate(annotations: &BTreeMap<String, String>, field: &str) -> Result<u64, ProviderError> {
    let Some(value) = annotations.get(field).filter(|value| !value.is_empty()) else {
        return Ok(0);
    };
    value.parse().map_err(|error| {
        ProviderError::InvalidData(format!(
            "Pod annotation `{field}` must be a nonnegative integer: {error}"
        ))
    })
}

fn parse_netem_integer(
    annotations: &BTreeMap<String, String>,
    field: &str,
) -> Result<u64, ProviderError> {
    let Some(value) = annotations.get(field).filter(|value| !value.is_empty()) else {
        return Ok(0);
    };
    match value.parse::<i64>() {
        Ok(value) => Ok(value.max(0) as u64),
        Err(error)
            if matches!(
                error.kind(),
                IntErrorKind::PosOverflow | IntErrorKind::NegOverflow
            ) =>
        {
            Err(ProviderError::InvalidData(format!(
                "Pod annotation `{field}` exceeds the supported integer range"
            )))
        }
        // Kube-OVN's Netem parser treats malformed values as disabled.
        Err(_) => Ok(0),
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PodNetworkData {
    pub(crate) mac_address: MacAddress,
    pub(crate) addresses: Vec<IpAssignment>,
    pub(crate) gateways: Vec<IpAddr>,
    pub(crate) default_route: bool,
    pub(crate) routes: Vec<Route>,
    pub(crate) logical_switch: String,
    pub(crate) provider_network: Option<String>,
    pub(crate) append_if_name: bool,
}

#[derive(Deserialize)]
struct RawRoute {
    #[serde(default)]
    dst: String,
    #[serde(default)]
    gw: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum AddressFamily {
    Ipv4,
    Ipv6,
}

pub(crate) fn ensure_ready<'a>(
    annotations: Option<&'a BTreeMap<String, String>>,
    provider: &str,
    if_name: &str,
) -> Result<&'a BTreeMap<String, String>, ProviderError> {
    let annotations = annotations.ok_or_else(|| {
        ProviderError::NotReady("Pod does not have Kube-OVN annotations yet".to_string())
    })?;

    let prefix = annotation_provider(annotations, provider, if_name);
    let allocated_key = key(&prefix, ALLOCATED);
    if annotations.get(&allocated_key).map(String::as_str) != Some("true") {
        return Err(ProviderError::NotReady(format!(
            "Pod annotation `{allocated_key}` is not true"
        )));
    }
    let ip_key = key(&prefix, IP_ADDRESS);
    if annotations
        .get(&ip_key)
        .is_none_or(|value| value.is_empty())
    {
        return Err(ProviderError::NotReady(format!(
            "Pod annotation `{ip_key}` has not been allocated"
        )));
    }
    let routed_key = key(provider, ROUTED);
    if default_route_enabled(annotations, provider, if_name)
        && annotations.get(&routed_key).map(String::as_str) != Some("true")
    {
        return Err(ProviderError::NotReady(format!(
            "Pod annotation `{routed_key}` is not true"
        )));
    }

    Ok(annotations)
}

pub(crate) fn parse(
    annotations: &BTreeMap<String, String>,
    provider: &str,
    if_name: &str,
) -> Result<PodNetworkData, ProviderError> {
    let prefix = annotation_provider(annotations, provider, if_name);
    let ip_key = key(&prefix, IP_ADDRESS);
    let cidr_key = key(&prefix, CIDR);
    let gateway_key = key(&prefix, GATEWAY);
    let ip_addresses = parse_address_map(required(annotations, &ip_key)?, &ip_key)?;
    let cidrs = parse_cidr_map(required(annotations, &cidr_key)?, &cidr_key)?;
    let gateways = parse_address_map(required(annotations, &gateway_key)?, &gateway_key)?;

    let mut addresses = Vec::with_capacity(ip_addresses.len());
    let mut aligned_gateways = Vec::with_capacity(ip_addresses.len());
    for (family, ip_address) in &ip_addresses {
        let (network, prefix_length) = cidrs.get(family).ok_or_else(|| {
            ProviderError::InvalidData(format!(
                "Pod IP `{ip_address}` has no matching CIDR address family"
            ))
        })?;
        if network_address(*ip_address, *prefix_length) != *network {
            return Err(ProviderError::InvalidData(format!(
                "Pod IP `{ip_address}` is outside matching CIDR `{network}/{prefix_length}`"
            )));
        }
        addresses.push(IpAssignment {
            address: *ip_address,
            prefix_length: *prefix_length,
        });
        aligned_gateways.push(*gateways.get(family).ok_or_else(|| {
            ProviderError::InvalidData(format!(
                "Pod IP `{ip_address}` has no matching gateway address family"
            ))
        })?);
    }

    let routes_key = key(provider, ROUTES);
    let routes = parse_routes(
        annotations.get(&routes_key).map(String::as_str),
        &routes_key,
    )?;

    Ok(PodNetworkData {
        mac_address: parse_mac_address(required(annotations, &key(&prefix, MAC_ADDRESS))?)?,
        addresses,
        gateways: aligned_gateways,
        default_route: default_route_enabled(annotations, provider, if_name),
        routes,
        logical_switch: required(annotations, &key(&prefix, LOGICAL_SWITCH))?.to_string(),
        provider_network: optional(annotations, &key(&prefix, PROVIDER_NETWORK)),
        append_if_name: prefix != provider,
    })
}

pub(crate) fn build_routes(
    data: &PodNetworkData,
    gateways: &[IpAddr],
    config_routes: &[Route],
) -> Vec<Route> {
    let mut routes = Vec::with_capacity(
        config_routes.len() + data.routes.len() + usize::from(data.default_route) * gateways.len(),
    );

    if data.default_route {
        for gateway in gateways {
            upsert_route(
                &mut routes,
                Route {
                    destination: unspecified(family(*gateway)),
                    prefix_length: 0,
                    gateway: Some(*gateway),
                },
            );
        }
    }

    // Route precedence: Pod annotations > CNI configuration > automatic defaults.
    for route in config_routes.iter().chain(&data.routes) {
        upsert_route(&mut routes, *route);
    }

    routes
}

pub(crate) fn parse_address_list(value: &str, field: &str) -> Result<Vec<IpAddr>, ProviderError> {
    Ok(parse_address_map(value, field)?.into_values().collect())
}

fn parse_address_map(
    value: &str,
    field: &str,
) -> Result<BTreeMap<AddressFamily, IpAddr>, ProviderError> {
    let mut addresses = BTreeMap::new();
    for raw_address in value.split(',') {
        let address = parse_single_address(raw_address, field)?;
        let family = family(address);
        if addresses.insert(family, address).is_some() {
            return Err(ProviderError::InvalidData(format!(
                "`{field}` contains more than one {} address",
                family_name(family)
            )));
        }
    }

    Ok(addresses)
}

fn required<'a>(
    annotations: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, ProviderError> {
    annotations
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ProviderError::InvalidData(format!("missing Pod annotation `{key}`")))
}

fn optional(annotations: &BTreeMap<String, String>, key: &str) -> Option<String> {
    annotations
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn key(provider: &str, field: &str) -> String {
    format!("{provider}.kubernetes.io/{field}")
}

// Match Kube-OVN: the presence of the interface-specific IP annotation selects
// that prefix for all interface-specific fields, without individual fallbacks.
fn annotation_provider(
    annotations: &BTreeMap<String, String>,
    provider: &str,
    if_name: &str,
) -> String {
    let with_interface = format!("{provider}.{if_name}");
    if annotations.contains_key(&key(&with_interface, IP_ADDRESS)) {
        with_interface
    } else {
        provider.to_string()
    }
}

fn default_route_enabled(
    annotations: &BTreeMap<String, String>,
    provider: &str,
    if_name: &str,
) -> bool {
    match annotations
        .get(&key(provider, DEFAULT_ROUTE))
        .map(String::as_str)
    {
        Some("true") => true,
        Some("false") => false,
        _ => if_name == "eth0",
    }
}

fn parse_mac_address(value: &str) -> Result<MacAddress, ProviderError> {
    let mut bytes = [0_u8; 6];
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != bytes.len() {
        return Err(ProviderError::InvalidData(format!(
            "invalid MAC address `{value}`"
        )));
    }
    for (byte, part) in bytes.iter_mut().zip(parts) {
        if part.len() != 2 {
            return Err(ProviderError::InvalidData(format!(
                "invalid MAC address `{value}`"
            )));
        }
        *byte = u8::from_str_radix(part, 16).map_err(|error| {
            ProviderError::InvalidData(format!("invalid MAC address `{value}`: {error}"))
        })?;
    }

    Ok(MacAddress(bytes))
}

fn parse_routes(value: Option<&str>, field: &str) -> Result<Vec<Route>, ProviderError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(Vec::new());
    };

    let raw_routes: Vec<RawRoute> = serde_json::from_str(value).map_err(|error| {
        ProviderError::InvalidData(format!("invalid Pod annotation `{field}`: {error}"))
    })?;
    parse_raw_routes(raw_routes, field)
}

pub(crate) fn parse_config_routes(
    value: Option<serde_json::Value>,
) -> Result<Vec<Route>, ProviderError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(Vec::new());
    };
    let raw_routes: Vec<RawRoute> = serde_json::from_value(value).map_err(|error| {
        ProviderError::InvalidData(format!("invalid Kube-OVN CNI `routes`: {error}"))
    })?;
    parse_raw_routes(raw_routes, "Kube-OVN CNI routes")
}

fn parse_raw_routes(raw_routes: Vec<RawRoute>, field: &str) -> Result<Vec<Route>, ProviderError> {
    let mut routes = Vec::with_capacity(raw_routes.len());

    for raw_route in raw_routes {
        let gateway = if raw_route.gw.trim().is_empty() {
            None
        } else {
            Some(parse_single_address(raw_route.gw.trim(), field)?)
        };

        let (destination, prefix_length) = if raw_route.dst.trim().is_empty() {
            let gateway = gateway.ok_or_else(|| {
                ProviderError::InvalidData(format!(
                    "a route in `{field}` has neither destination nor gateway"
                ))
            })?;
            (unspecified(family(gateway)), 0)
        } else {
            parse_cidr(raw_route.dst.trim(), field)?
        };

        upsert_route(
            &mut routes,
            Route {
                destination,
                prefix_length,
                gateway,
            },
        );
    }

    Ok(routes)
}

fn parse_cidr_map(
    value: &str,
    field: &str,
) -> Result<BTreeMap<AddressFamily, (IpAddr, u8)>, ProviderError> {
    let mut cidrs = BTreeMap::new();
    for raw_cidr in value.split(',') {
        let cidr = parse_cidr(raw_cidr.trim(), field)?;
        let family = family(cidr.0);
        if cidrs.insert(family, cidr).is_some() {
            return Err(ProviderError::InvalidData(format!(
                "`{field}` contains more than one {} CIDR",
                family_name(family)
            )));
        }
    }

    Ok(cidrs)
}

fn parse_cidr(value: &str, field: &str) -> Result<(IpAddr, u8), ProviderError> {
    let (address, prefix_length) = value.split_once('/').ok_or_else(|| {
        ProviderError::InvalidData(format!("invalid IP CIDR in `{field}`: missing prefix"))
    })?;
    let address = parse_single_address(address, field)?;
    let prefix_length = prefix_length.parse::<u8>().map_err(|error| {
        ProviderError::InvalidData(format!("invalid prefix length in `{field}`: {error}"))
    })?;
    let maximum = match address {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };
    if prefix_length > maximum {
        return Err(ProviderError::InvalidData(format!(
            "{} prefix length in `{field}` is greater than {maximum}",
            family_name(family(address))
        )));
    }

    Ok((network_address(address, prefix_length), prefix_length))
}

fn parse_single_address(value: &str, field: &str) -> Result<IpAddr, ProviderError> {
    value.trim().parse().map_err(|error| {
        ProviderError::InvalidData(format!("invalid IP address in `{field}`: {error}"))
    })
}

fn network_address(address: IpAddr, prefix_length: u8) -> IpAddr {
    match address {
        IpAddr::V4(address) => {
            let mask = u32::MAX
                .checked_shl(32 - u32::from(prefix_length))
                .unwrap_or(0);
            IpAddr::V4(Ipv4Addr::from(u32::from(address) & mask))
        }
        IpAddr::V6(address) => {
            let mask = u128::MAX
                .checked_shl(128 - u32::from(prefix_length))
                .unwrap_or(0);
            IpAddr::V6(Ipv6Addr::from(u128::from(address) & mask))
        }
    }
}

fn family(address: IpAddr) -> AddressFamily {
    match address {
        IpAddr::V4(_) => AddressFamily::Ipv4,
        IpAddr::V6(_) => AddressFamily::Ipv6,
    }
}

fn unspecified(family: AddressFamily) -> IpAddr {
    match family {
        AddressFamily::Ipv4 => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        AddressFamily::Ipv6 => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    }
}

fn family_name(family: AddressFamily) -> &'static str {
    match family {
        AddressFamily::Ipv4 => "IPv4",
        AddressFamily::Ipv6 => "IPv6",
    }
}

fn upsert_route(routes: &mut Vec<Route>, route: Route) {
    if let Some(index) = routes.iter().position(|current| {
        current.destination == route.destination && current.prefix_length == route.prefix_length
    }) {
        routes.remove(index);
    }
    routes.push(route);
}
