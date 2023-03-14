// SPDX-License-Identifier: MIT

use anyhow::Context;

use crate::{
    nlas::neighbour::Nla,
    traits::{Emitable, Parseable},
    DecodeError,
    NeighbourHeader,
    NeighbourMessageBuffer,
};

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct NeighbourMessage {
    pub header: NeighbourHeader,
    pub nlas: Vec<Nla>,
}

impl Emitable for NeighbourMessage {
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

impl<'a, T: AsRef<[u8]> + 'a> Parseable<NeighbourMessageBuffer<&'a T>> for NeighbourMessage {
    fn parse(buf: &NeighbourMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        Ok(NeighbourMessage {
            header: NeighbourHeader::parse(buf)
                .context("failed to parse neighbour message header")?,
            nlas: Vec::<Nla>::parse(buf).context("failed to parse neighbour message NLAs")?,
        })
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Parseable<NeighbourMessageBuffer<&'a T>> for Vec<Nla> {
    fn parse(buf: &NeighbourMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        let mut nlas = vec![];
        for nla_buf in buf.nlas() {
            nlas.push(Nla::parse(&nla_buf?)?);
        }
        Ok(nlas)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        constants::*,
        traits::Emitable,
        NeighbourHeader,
        NeighbourMessage,
        NeighbourMessageBuffer,
    };

    // 0020   0a 00 00 00 02 00 00 00 02 00 80 01 14 00 01 00
    // 0030   2a 02 80 10 66 d5 00 00 f6 90 ea ff fe 00 2d 83
    // 0040   0a 00 02 00 f4 90 ea 00 2d 83 00 00 08 00 04 00
    // 0050   01 00 00 00 14 00 03 00 00 00 00 00 00 00 00 00
    // 0060   00 00 00 00 02 00 00 00

    #[rustfmt::skip]
    static HEADER: [u8; 12] = [
        0x0a, // interface family (inet6)
        0xff, 0xff, 0xff, // padding
        0x01, 0x00, 0x00, 0x00, // interface index = 1
        0x02, 0x00, // state NUD_REACHABLE
        0x80, // flags NTF_PROXY
        0x01  // ntype

        // nlas
        // will add some once I've got them parsed out.
    ];

    #[test]
    fn packet_header_read() {
        let packet = NeighbourMessageBuffer::new(&HEADER[0..12]);
        assert_eq!(packet.family(), AF_INET6 as u8);
        assert_eq!(packet.ifindex(), 1);
        assert_eq!(packet.state(), NUD_REACHABLE);
        assert_eq!(packet.flags(), NTF_ROUTER);
        assert_eq!(packet.ntype(), NDA_DST as u8);
    }

    #[test]
    fn packet_header_build() {
        let mut buf = vec![0xff; 12];
        {
            let mut packet = NeighbourMessageBuffer::new(&mut buf);
            packet.set_family(AF_INET6 as u8);
            packet.set_ifindex(1);
            packet.set_state(NUD_REACHABLE);
            packet.set_flags(NTF_ROUTER);
            packet.set_ntype(NDA_DST as u8);
        }
        assert_eq!(&buf[..], &HEADER[0..12]);
    }

    #[test]
    fn emit() {
        let header = NeighbourHeader {
            family: AF_INET6 as u8,
            ifindex: 1,
            state: NUD_REACHABLE,
            flags: NTF_ROUTER,
            ntype: NDA_DST as u8,
        };

        let nlas = vec![];
        let packet = NeighbourMessage { header, nlas };
        let mut buf = vec![0; 12];

        assert_eq!(packet.buffer_len(), 12);
        packet.emit(&mut buf[..]);
    }
}
