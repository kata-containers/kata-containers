// SPDX-License-Identifier: MIT

use anyhow::Context;

use crate::{
    constants::*,
    nlas::{
        tc::{Nla, Stats, Stats2, StatsBuffer, TcOpt},
        DefaultNla,
        NlasIterator,
    },
    parsers::{parse_string, parse_u8},
    traits::{Emitable, Parseable, ParseableParametrized},
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

    /// Create a new `TcMessage` with the given index
    pub fn with_index(index: i32) -> Self {
        Self {
            header: TcHeader {
                index,
                ..Default::default()
            },
            nlas: Vec::new(),
        }
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
        self.nlas
            .as_slice()
            .emit(&mut buffer[self.header.buffer_len()..]);
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
        let mut kind = String::new();

        for nla_buf in buf.nlas() {
            let buf = nla_buf.context("invalid tc nla")?;
            let payload = buf.value();
            let nla = match buf.kind() {
                TCA_UNSPEC => Nla::Unspec(payload.to_vec()),
                TCA_KIND => {
                    kind = parse_string(payload).context("invalid TCA_KIND")?;
                    Nla::Kind(kind.clone())
                }
                TCA_OPTIONS => {
                    let mut nlas = vec![];
                    for nla in NlasIterator::new(payload) {
                        let nla = nla.context("invalid TCA_OPTIONS")?;
                        nlas.push(
                            TcOpt::parse_with_param(&nla, &kind)
                                .context("failed to parse TCA_OPTIONS")?,
                        )
                    }
                    Nla::Options(nlas)
                }
                TCA_STATS => Nla::Stats(
                    Stats::parse(&StatsBuffer::new_checked(payload).context("invalid TCA_STATS")?)
                        .context("failed to parse TCA_STATS")?,
                ),
                TCA_XSTATS => Nla::XStats(payload.to_vec()),
                TCA_RATE => Nla::Rate(payload.to_vec()),
                TCA_FCNT => Nla::Fcnt(payload.to_vec()),
                TCA_STATS2 => {
                    let mut nlas = vec![];
                    for nla in NlasIterator::new(payload) {
                        let nla = nla.context("invalid TCA_STATS2")?;
                        nlas.push(Stats2::parse(&nla).context("failed to parse TCA_STATS2")?);
                    }
                    Nla::Stats2(nlas)
                }
                TCA_STAB => Nla::Stab(payload.to_vec()),
                TCA_CHAIN => Nla::Chain(payload.to_vec()),
                TCA_HW_OFFLOAD => {
                    Nla::HwOffload(parse_u8(payload).context("failed to parse TCA_HW_OFFLOAD")?)
                }
                _ => Nla::Other(DefaultNla::parse(&buf).context("failed to parse tc nla")?),
            };

            nlas.push(nla);
        }
        Ok(nlas)
    }
}
