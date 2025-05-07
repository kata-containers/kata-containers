// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use agent::IPFamily;
use anyhow::{anyhow, Context, Result};
use netlink_packet_route::address::AddressAttribute;
use netlink_packet_route::address::AddressMessage;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Address {
    pub addr: IpAddr,
    pub label: String,
    pub flags: u32,
    pub scope: u8,
    pub perfix_len: u8,
    pub peer: IpAddr,
    pub broadcast: IpAddr,
    pub prefered_lft: u32,
    pub valid_ltf: u32,
}

impl TryFrom<AddressMessage> for Address {
    type Error = anyhow::Error;
    fn try_from(msg: AddressMessage) -> Result<Self> {
        let AddressMessage {
            header, attributes, ..
        } = msg;
        let mut addr = Address {
            addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            peer: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            broadcast: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            label: String::default(),
            flags: 0,
            scope: u8::from(header.scope),
            perfix_len: header.prefix_len,
            prefered_lft: 0,
            valid_ltf: 0,
        };

        for nla in attributes.into_iter() {
            match nla {
                AddressAttribute::Address(a) => {
                    addr.addr = a;
                }
                AddressAttribute::Broadcast(b) => {
                    addr.broadcast = IpAddr::V4(b);
                }
                AddressAttribute::Label(l) => {
                    addr.label = l;
                }
                AddressAttribute::Flags(f) => {
                    //since the AddressAttribute::Flags(f) didn't implemented the u32 from trait,
                    //thus here just implemeted a simple transformer.
                    addr.flags = f.bits();
                }
                AddressAttribute::CacheInfo(_c) => {}
                _ => {}
            }
        }

        Ok(addr)
    }
}

pub(crate) fn parse_ip_cidr(ip: &str) -> Result<(IpAddr, u8)> {
    let items: Vec<&str> = ip.split('/').collect();
    if items.len() != 2 {
        return Err(anyhow!(format!(
            "{} is a bad IP address in format of CIDR",
            ip
        )));
    }
    let ipaddr = IpAddr::from_str(items[0]).context("Parse IP address from string")?;
    let mask = u8::from_str(items[1]).context("Parse mask")?;
    if ipaddr.is_ipv4() && mask > 32 {
        return Err(anyhow!(format!(
            "The mask of IPv4 address should be less than or equal to 32, but we got {}.",
            mask
        )));
    }
    if mask > 128 {
        return Err(anyhow!(format!(
            "The mask should be less than or equal to 128, but we got {}.",
            mask
        )));
    }
    Ok((ipaddr, mask))
}

/// Retrieve IP Family defined at agent crate from IpAddr.
#[inline]
pub(crate) fn ip_family_from_ip_addr(ip_addr: &IpAddr) -> IPFamily {
    if ip_addr.is_ipv4() {
        IPFamily::V4
    } else {
        IPFamily::V6
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ip_cidr() {
        let test_cases = [
            ("127.0.0.1/32", ("127.0.0.1", 32u8)),
            ("2001:4860:4860::8888/32", ("2001:4860:4860::8888", 32u8)),
            ("2001:4860:4860::8888/128", ("2001:4860:4860::8888", 128u8)),
        ];
        for tc in test_cases.iter() {
            let (ipaddr, mask) = parse_ip_cidr(tc.0).unwrap();
            assert_eq!(ipaddr.to_string(), tc.1 .0);
            assert_eq!(mask, tc.1 .1);
        }
        let test_cases = [
            "127.0.0.1/33",
            "2001:4860:4860::8888/129",
            "2001:4860:4860::8888/300",
            "127.0.0.1/33/1",
            "127.0.0.1",
        ];
        for tc in test_cases.iter() {
            assert!(parse_ip_cidr(tc).is_err());
        }
    }
}
