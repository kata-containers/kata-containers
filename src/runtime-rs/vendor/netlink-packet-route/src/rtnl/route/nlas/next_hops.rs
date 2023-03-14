// SPDX-License-Identifier: MIT

use anyhow::Context;
use std::net::IpAddr;

use crate::{
    constants,
    nlas::{NlaBuffer, NlasIterator},
    parsers::parse_ip,
    route::nlas::Nla,
    traits::{Emitable, Parseable},
    DecodeError,
};

bitflags! {
    pub struct NextHopFlags: u8 {
        const RTNH_F_EMPTY = 0;
        const RTNH_F_DEAD = constants::RTNH_F_DEAD as u8;
        const RTNH_F_PERVASIVE = constants::RTNH_F_PERVASIVE as u8;
        const RTNH_F_ONLINK = constants::RTNH_F_ONLINK as u8;
        const RTNH_F_OFFLOAD = constants::RTNH_F_OFFLOAD as u8;
        const RTNH_F_LINKDOWN = constants::RTNH_F_LINKDOWN as u8;
        const RTNH_F_UNRESOLVED = constants::RTNH_F_UNRESOLVED as u8;
    }
}

const PAYLOAD_OFFSET: usize = 8;

buffer!(NextHopBuffer {
    length: (u16, 0..2),
    flags: (u8, 2),
    hops: (u8, 3),
    interface_id: (u32, 4..8),
    payload: (slice, PAYLOAD_OFFSET..),
});

impl<T: AsRef<[u8]>> NextHopBuffer<T> {
    pub fn new_checked(buffer: T) -> Result<Self, DecodeError> {
        let packet = Self::new(buffer);
        packet.check_buffer_length()?;
        Ok(packet)
    }

    fn check_buffer_length(&self) -> Result<(), DecodeError> {
        let len = self.buffer.as_ref().len();
        if len < PAYLOAD_OFFSET {
            return Err(
                format!("invalid NextHopBuffer: length {} < {}", len, PAYLOAD_OFFSET).into(),
            );
        }
        if len < self.length() as usize {
            return Err(format!(
                "invalid NextHopBuffer: length {} < {}",
                len,
                8 + self.length()
            )
            .into());
        }
        Ok(())
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> NextHopBuffer<&'a T> {
    pub fn nlas(&self) -> impl Iterator<Item = Result<NlaBuffer<&'a [u8]>, DecodeError>> {
        NlasIterator::new(&self.payload()[..(self.length() as usize - PAYLOAD_OFFSET)])
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NextHop {
    /// Next-hop flags (see [`NextHopFlags`])
    pub flags: NextHopFlags,
    /// Next-hop priority
    pub hops: u8,
    /// Interface index for the next-hop
    pub interface_id: u32,
    /// Attributes
    pub nlas: Vec<Nla>,
}

impl<'a, T: AsRef<[u8]>> Parseable<NextHopBuffer<&'a T>> for NextHop {
    fn parse(buf: &NextHopBuffer<&T>) -> Result<NextHop, DecodeError> {
        let nlas = Vec::<Nla>::parse(
            &NextHopBuffer::new_checked(buf.buffer)
                .context("cannot parse route attributes in next-hop")?,
        )
        .context("cannot parse route attributes in next-hop")?;
        Ok(NextHop {
            flags: NextHopFlags::from_bits_truncate(buf.flags()),
            hops: buf.hops(),
            interface_id: buf.interface_id(),
            nlas,
        })
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Parseable<NextHopBuffer<&'a T>> for Vec<Nla> {
    fn parse(buf: &NextHopBuffer<&'a T>) -> Result<Self, DecodeError> {
        let mut nlas = vec![];
        for nla_buf in buf.nlas() {
            nlas.push(Nla::parse(&nla_buf?)?);
        }
        Ok(nlas)
    }
}

impl Emitable for NextHop {
    fn buffer_len(&self) -> usize {
        // len, flags, hops and interface id fields
        PAYLOAD_OFFSET + self.nlas.as_slice().buffer_len()
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut nh_buffer = NextHopBuffer::new(buffer);
        nh_buffer.set_length(self.buffer_len() as u16);
        nh_buffer.set_flags(self.flags.bits());
        nh_buffer.set_hops(self.hops);
        nh_buffer.set_interface_id(self.interface_id);
        self.nlas.as_slice().emit(nh_buffer.payload_mut())
    }
}

impl NextHop {
    /// Gateway address (it is actually encoded as an `RTA_GATEWAY` nla)
    pub fn gateway(&self) -> Option<IpAddr> {
        self.nlas.iter().find_map(|nla| {
            if let Nla::Gateway(ip) = nla {
                parse_ip(ip).ok()
            } else {
                None
            }
        })
    }
}
