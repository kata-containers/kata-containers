use anyhow::Context;

use crate::{
    nlas::tc::Nla,
    traits::{Emitable, Parseable},
    DecodeError,
    TcMessageBuffer,
    TC_HEADER_LEN,
};

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct TcMessage {
    pub header: TcHeader,
    pub nlas: Vec<Nla>,
}

impl TcMessage {
    pub fn into_parts(self) -> (TcHeader, Vec<Nla>) {
        (self.header, self.nlas)
    }

    pub fn from_parts(header: TcHeader, nlas: Vec<Nla>) -> Self {
        TcMessage { header, nlas }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct TcHeader {
    pub family: u8,
    // Interface index
    pub index: i32,
    // Qdisc handle
    pub handle: u32,
    // Parent Qdisc
    pub parent: u32,
    pub info: u32,
}

impl Emitable for TcHeader {
    fn buffer_len(&self) -> usize {
        TC_HEADER_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut packet = TcMessageBuffer::new(buffer);
        packet.set_family(self.family);
        packet.set_index(self.index);
        packet.set_handle(self.handle);
        packet.set_parent(self.parent);
        packet.set_info(self.info);
    }
}

impl Emitable for TcMessage {
    fn buffer_len(&self) -> usize {
        self.header.buffer_len() + self.nlas.as_slice().buffer_len()
    }

    fn emit(&self, buffer: &mut [u8]) {
        self.header.emit(buffer);
        self.nlas.as_slice().emit(buffer);
    }
}

impl<T: AsRef<[u8]>> Parseable<TcMessageBuffer<T>> for TcHeader {
    fn parse(buf: &TcMessageBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            family: buf.family(),
            index: buf.index(),
            handle: buf.handle(),
            parent: buf.parent(),
            info: buf.info(),
        })
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Parseable<TcMessageBuffer<&'a T>> for TcMessage {
    fn parse(buf: &TcMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        Ok(Self {
            header: TcHeader::parse(buf).context("failed to parse tc message header")?,
            nlas: Vec::<Nla>::parse(buf).context("failed to parse tc message NLAs")?,
        })
    }
}

impl<'a, T: AsRef<[u8]> + 'a> Parseable<TcMessageBuffer<&'a T>> for Vec<Nla> {
    fn parse(buf: &TcMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        let mut nlas = vec![];
        for nla_buf in buf.nlas() {
            nlas.push(Nla::parse(&nla_buf?)?);
        }
        Ok(nlas)
    }
}
