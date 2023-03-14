// SPDX-License-Identifier: MIT

pub mod mirred;

use anyhow::Context;
use byteorder::{ByteOrder, NativeEndian};

use crate::{
    nlas::{self, DefaultNla, NlaBuffer, NlasIterator},
    parsers::{parse_string, parse_u32},
    tc::{constants::*, Stats2},
    traits::{Emitable, Parseable, ParseableParametrized},
    DecodeError,
};

pub const TC_GEN_BUF_LEN: usize = 20;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Action {
    pub tab: u16,
    pub nlas: Vec<ActNla>,
}

impl Default for Action {
    fn default() -> Self {
        Self {
            tab: TCA_ACT_TAB,
            nlas: Vec::new(),
        }
    }
}

impl nlas::Nla for Action {
    fn value_len(&self) -> usize {
        self.nlas.as_slice().buffer_len()
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        self.nlas.as_slice().emit(buffer)
    }

    fn kind(&self) -> u16 {
        self.tab
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for Action {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        let mut nlas = vec![];
        let mut kind = String::new();

        for iter in NlasIterator::new(buf.value()) {
            let buf = iter.context("invalid action nla")?;
            let payload = buf.value();
            nlas.push(match buf.kind() {
                TCA_ACT_UNSPEC => ActNla::Unspec(payload.to_vec()),
                TCA_ACT_KIND => {
                    kind = parse_string(payload).context("failed to parse TCA_ACT_KIND")?;
                    ActNla::Kind(kind.clone())
                }
                TCA_ACT_OPTIONS => {
                    let mut nlas = vec![];
                    for nla in NlasIterator::new(payload) {
                        let nla = nla.context("invalid TCA_ACT_OPTIONS")?;
                        nlas.push(
                            ActOpt::parse_with_param(&nla, &kind)
                                .context("failed to parse TCA_ACT_OPTIONS")?,
                        )
                    }
                    ActNla::Options(nlas)
                }
                TCA_ACT_INDEX => {
                    ActNla::Index(parse_u32(payload).context("failed to parse TCA_ACT_INDEX")?)
                }
                TCA_ACT_STATS => {
                    let mut nlas = vec![];
                    for nla in NlasIterator::new(payload) {
                        let nla = nla.context("invalid TCA_ACT_STATS")?;
                        nlas.push(Stats2::parse(&nla).context("failed to parse TCA_ACT_STATS")?);
                    }
                    ActNla::Stats(nlas)
                }
                TCA_ACT_COOKIE => ActNla::Cookie(payload.to_vec()),
                _ => ActNla::Other(DefaultNla::parse(&buf).context("failed to parse action nla")?),
            });
        }
        Ok(Self {
            tab: buf.kind(),
            nlas,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ActNla {
    Unspec(Vec<u8>),
    Kind(String),
    Options(Vec<ActOpt>),
    Index(u32),
    Stats(Vec<Stats2>),
    Cookie(Vec<u8>),
    Other(DefaultNla),
}

impl nlas::Nla for ActNla {
    fn value_len(&self) -> usize {
        use self::ActNla::*;
        match self {
            Unspec(bytes) | Cookie(bytes) => bytes.len(),
            Kind(k) => k.len() + 1,
            Options(opt) => opt.as_slice().buffer_len(),
            Index(_) => 4,
            Stats(s) => s.as_slice().buffer_len(),
            Other(attr) => attr.value_len(),
        }
    }
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::ActNla::*;
        match self {
            Unspec(bytes) | Cookie(bytes) => buffer.copy_from_slice(bytes.as_slice()),
            Kind(string) => {
                buffer[..string.as_bytes().len()].copy_from_slice(string.as_bytes());
                buffer[string.as_bytes().len()] = 0;
            }
            Options(opt) => opt.as_slice().emit(buffer),
            Index(value) => NativeEndian::write_u32(buffer, *value),
            Stats(s) => s.as_slice().emit(buffer),
            Other(attr) => attr.emit_value(buffer),
        }
    }
    fn kind(&self) -> u16 {
        use self::ActNla::*;
        match self {
            Unspec(_) => TCA_ACT_UNSPEC,
            Kind(_) => TCA_ACT_KIND,
            Options(_) => TCA_ACT_OPTIONS,
            Index(_) => TCA_ACT_INDEX,
            Stats(_) => TCA_ACT_STATS,
            Cookie(_) => TCA_ACT_COOKIE,
            Other(nla) => nla.kind(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ActOpt {
    Mirred(mirred::Nla),
    // Other options
    Other(DefaultNla),
}

impl nlas::Nla for ActOpt {
    fn value_len(&self) -> usize {
        use self::ActOpt::*;
        match self {
            Mirred(nla) => nla.value_len(),
            Other(nla) => nla.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::ActOpt::*;
        match self {
            Mirred(nla) => nla.emit_value(buffer),
            Other(nla) => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::ActOpt::*;
        match self {
            Mirred(nla) => nla.kind(),
            Other(nla) => nla.kind(),
        }
    }
}

impl<'a, T, S> ParseableParametrized<NlaBuffer<&'a T>, S> for ActOpt
where
    T: AsRef<[u8]> + ?Sized,
    S: AsRef<str>,
{
    fn parse_with_param(buf: &NlaBuffer<&'a T>, kind: S) -> Result<Self, DecodeError> {
        Ok(match kind.as_ref() {
            mirred::KIND => {
                Self::Mirred(mirred::Nla::parse(buf).context("failed to parse mirred action")?)
            }
            _ => Self::Other(DefaultNla::parse(buf).context("failed to parse action options")?),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct TcGen {
    pub index: u32,
    pub capab: u32,
    pub action: i32,
    pub refcnt: i32,
    pub bindcnt: i32,
}

buffer!(TcGenBuffer(TC_GEN_BUF_LEN) {
    index: (u32, 0..4),
    capab: (u32, 4..8),
    action: (i32, 8..12),
    refcnt: (i32, 12..16),
    bindcnt: (i32, 16..20),
});

impl Emitable for TcGen {
    fn buffer_len(&self) -> usize {
        TC_GEN_BUF_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut packet = TcGenBuffer::new(buffer);
        packet.set_index(self.index);
        packet.set_capab(self.capab);
        packet.set_action(self.action);
        packet.set_refcnt(self.refcnt);
        packet.set_bindcnt(self.bindcnt);
    }
}

impl<T: AsRef<[u8]>> Parseable<TcGenBuffer<T>> for TcGen {
    fn parse(buf: &TcGenBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            index: buf.index(),
            capab: buf.capab(),
            action: buf.action(),
            refcnt: buf.refcnt(),
            bindcnt: buf.bindcnt(),
        })
    }
}
