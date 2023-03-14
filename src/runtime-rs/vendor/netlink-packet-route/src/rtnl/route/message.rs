// SPDX-License-Identifier: MIT

use crate::{
    nlas::route::Nla,
    traits::{Emitable, Parseable},
    DecodeError,
    RouteHeader,
    RouteMessageBuffer,
};
use anyhow::Context;
use std::net::IpAddr;

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct RouteMessage {
    pub header: RouteHeader,
    pub nlas: Vec<Nla>,
}

impl Emitable for RouteMessage {
    fn buffer_len(&self) -> usize {
        self.header.buffer_len() + self.nlas.as_slice().buffer_len()
    }

    fn emit(&self, buffer: &mut [u8]) {
        self.header.emit(buffer);
        self.nlas
            .as_slice()
            .emit(&mut buffer[self.header.buffer_len()..]);
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Parseable<RouteMessageBuffer<&'a T>> for RouteMessage {
    fn parse(buf: &RouteMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        Ok(RouteMessage {
            header: RouteHeader::parse(buf).context("failed to parse route message header")?,
            nlas: Vec::<Nla>::parse(buf).context("failed to parse route message NLAs")?,
        })
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Parseable<RouteMessageBuffer<&'a T>> for Vec<Nla> {
    fn parse(buf: &RouteMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        let mut nlas = vec![];
        for nla_buf in buf.nlas() {
            nlas.push(Nla::parse(&nla_buf?)?);
        }
        Ok(nlas)
    }
}

fn octets_to_addr(octets: &[u8]) -> Result<IpAddr, DecodeError> {
    if octets.len() == 4 {
        let mut ary: [u8; 4] = Default::default();
        ary.copy_from_slice(octets);
        Ok(IpAddr::from(ary))
    } else if octets.len() == 16 {
        let mut ary: [u8; 16] = Default::default();
        ary.copy_from_slice(octets);
        Ok(IpAddr::from(ary))
    } else {
        Err(DecodeError::from("Cannot decode IP address"))
    }
}

impl RouteMessage {
    /// Returns the input interface index, if present.
    pub fn input_interface(&self) -> Option<u32> {
        self.nlas.iter().find_map(|nla| {
            if let Nla::Iif(v) = nla {
                Some(*v)
            } else {
                None
            }
        })
    }

    /// Returns the output interface index, if present.
    pub fn output_interface(&self) -> Option<u32> {
        self.nlas.iter().find_map(|nla| {
            if let Nla::Oif(v) = nla {
                Some(*v)
            } else {
                None
            }
        })
    }

    /// Returns the source address prefix, if present.
    pub fn source_prefix(&self) -> Option<(IpAddr, u8)> {
        self.nlas.iter().find_map(|nla| {
            if let Nla::Source(v) = nla {
                octets_to_addr(v)
                    .ok()
                    .map(|addr| (addr, self.header.source_prefix_length))
            } else {
                None
            }
        })
    }

    /// Returns the destination subnet prefix, if present.
    pub fn destination_prefix(&self) -> Option<(IpAddr, u8)> {
        self.nlas.iter().find_map(|nla| {
            if let Nla::Destination(v) = nla {
                octets_to_addr(v)
                    .ok()
                    .map(|addr| (addr, self.header.destination_prefix_length))
            } else {
                None
            }
        })
    }

    /// Returns the gateway address, if present.
    pub fn gateway(&self) -> Option<IpAddr> {
        self.nlas.iter().find_map(|nla| {
            if let Nla::Gateway(v) = nla {
                octets_to_addr(v).ok()
            } else {
                None
            }
        })
    }
}
