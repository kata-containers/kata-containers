// SPDX-License-Identifier: MIT

use anyhow::Context;

use crate::{
    nlas::address::Nla,
    traits::{Emitable, Parseable},
    AddressMessageBuffer,
    DecodeError,
    ADDRESS_HEADER_LEN,
};

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct AddressMessage {
    pub header: AddressHeader,
    pub nlas: Vec<Nla>,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct AddressHeader {
    pub family: u8,
    pub prefix_len: u8,
    pub flags: u8,
    pub scope: u8,
    pub index: u32,
}

impl Emitable for AddressHeader {
    fn buffer_len(&self) -> usize {
        ADDRESS_HEADER_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut packet = AddressMessageBuffer::new(buffer);
        packet.set_family(self.family);
        packet.set_prefix_len(self.prefix_len);
        packet.set_flags(self.flags);
        packet.set_scope(self.scope);
        packet.set_index(self.index);
    }
}

impl Emitable for AddressMessage {
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

impl<T: AsRef<[u8]>> Parseable<AddressMessageBuffer<T>> for AddressHeader {
    fn parse(buf: &AddressMessageBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            family: buf.family(),
            prefix_len: buf.prefix_len(),
            flags: buf.flags(),
            scope: buf.scope(),
            index: buf.index(),
        })
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Parseable<AddressMessageBuffer<&'a T>> for AddressMessage {
    fn parse(buf: &AddressMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        Ok(AddressMessage {
            header: AddressHeader::parse(buf).context("failed to parse address message header")?,
            nlas: Vec::<Nla>::parse(buf).context("failed to parse address message NLAs")?,
        })
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Parseable<AddressMessageBuffer<&'a T>> for Vec<Nla> {
    fn parse(buf: &AddressMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        let mut nlas = vec![];
        for nla_buf in buf.nlas() {
            nlas.push(Nla::parse(&nla_buf?)?);
        }
        Ok(nlas)
    }
}
