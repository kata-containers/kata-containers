// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    convert::TryFrom,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use anyhow::{anyhow, Result};
use netlink_packet_route::{nlas::address::Nla, AddressMessage, AF_INET, AF_INET6};

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
        let AddressMessage { header, nlas } = msg;
        let mut addr = Address {
            addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            peer: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            broadcast: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            label: String::default(),
            flags: 0,
            scope: header.scope,
            perfix_len: header.prefix_len,
            prefered_lft: 0,
            valid_ltf: 0,
        };

        let mut local = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
        let mut dst = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));

        for nla in nlas.into_iter() {
            match nla {
                Nla::Address(a) => {
                    dst = parse_ip(a, header.family)?;
                }
                Nla::Local(a) => {
                    local = parse_ip(a, header.family)?;
                }
                Nla::Broadcast(b) => {
                    addr.broadcast = parse_ip(b, header.family)?;
                }
                Nla::Label(l) => {
                    addr.label = l;
                }
                Nla::Flags(f) => {
                    addr.flags = f;
                }
                Nla::CacheInfo(_c) => {}
                _ => {}
            }
        }

        // IPv6 sends the local address as IFA_ADDRESS with no
        // IFA_LOCAL, IPv4 sends both IFA_LOCAL and IFA_ADDRESS
        // with IFA_ADDRESS being the peer address if they differ
        //
        // But obviously, as there are IPv6 PtP addresses, too,
        // IFA_LOCAL should also be handled for IPv6.
        if local.is_unspecified() {
            if header.family == AF_INET as u8 && local == dst {
                addr.addr = dst;
            } else {
                addr.addr = local;
                addr.peer = dst;
            }
        } else {
            addr.addr = dst;
        }
        Ok(addr)
    }
}

fn parse_ip(ip: Vec<u8>, family: u8) -> Result<IpAddr> {
    let support_len = if family as u16 == AF_INET { 4 } else { 16 };
    if ip.len() != support_len {
        return Err(anyhow!(
            "invalid ip addresses {:?} support {}",
            &ip,
            support_len
        ));
    }
    match family as u16 {
        AF_INET => Ok(IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]))),
        AF_INET6 => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&ip[..16]);
            Ok(IpAddr::V6(Ipv6Addr::from(octets)))
        }
        _ => {
            return Err(anyhow!("unknown IP network family {}", family));
        }
    }
}
