// SPDX-License-Identifier: MIT

/// Mirred action
///
/// The mirred action allows packet mirroring (copying) or
/// redirecting (stealing) the packet it receives. Mirroring is what
/// is sometimes referred to as Switch Port Analyzer (SPAN) and is
/// commonly used to analyze and/or debug flows.
use crate::{
    nlas::{self, DefaultNla, NlaBuffer},
    tc::{constants::*, TC_GEN_BUF_LEN},
    traits::{Emitable, Parseable},
    DecodeError,
};

pub const KIND: &str = "mirred";
pub const TC_MIRRED_BUF_LEN: usize = TC_GEN_BUF_LEN + 8;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Nla {
    Unspec(Vec<u8>),
    Tm(Vec<u8>),
    Parms(TcMirred),
    Other(DefaultNla),
}

impl nlas::Nla for Nla {
    fn value_len(&self) -> usize {
        use self::Nla::*;
        match self {
            Unspec(bytes) | Tm(bytes) => bytes.len(),
            Parms(_) => TC_MIRRED_BUF_LEN,
            Other(attr) => attr.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::Nla::*;
        match self {
            Unspec(bytes) | Tm(bytes) => buffer.copy_from_slice(bytes.as_slice()),
            Parms(p) => p.emit(buffer),
            Other(attr) => attr.emit_value(buffer),
        }
    }
    fn kind(&self) -> u16 {
        use self::Nla::*;
        match self {
            Unspec(_) => TCA_MIRRED_UNSPEC,
            Tm(_) => TCA_MIRRED_TM,
            Parms(_) => TCA_MIRRED_PARMS,
            Other(nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for Nla {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::Nla::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            TCA_MIRRED_UNSPEC => Unspec(payload.to_vec()),
            TCA_MIRRED_TM => Tm(payload.to_vec()),
            TCA_MIRRED_PARMS => Parms(TcMirred::parse(&TcMirredBuffer::new_checked(payload)?)?),
            _ => Other(DefaultNla::parse(buf)?),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct TcMirred {
    pub index: u32,
    pub capab: u32,
    pub action: i32,
    pub refcnt: i32,
    pub bindcnt: i32,

    pub eaction: i32,
    pub ifindex: u32,
}

buffer!(TcMirredBuffer(TC_MIRRED_BUF_LEN) {
    index: (u32, 0..4),
    capab: (u32, 4..8),
    action: (i32, 8..12),
    refcnt: (i32, 12..16),
    bindcnt: (i32, 16..20),
    eaction: (i32, TC_GEN_BUF_LEN..(TC_GEN_BUF_LEN + 4)),
    ifindex: (u32, (TC_GEN_BUF_LEN + 4)..TC_MIRRED_BUF_LEN),
});

impl Emitable for TcMirred {
    fn buffer_len(&self) -> usize {
        TC_MIRRED_BUF_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut packet = TcMirredBuffer::new(buffer);
        packet.set_index(self.index);
        packet.set_capab(self.capab);
        packet.set_action(self.action);
        packet.set_refcnt(self.refcnt);
        packet.set_bindcnt(self.bindcnt);

        packet.set_eaction(self.eaction);
        packet.set_ifindex(self.ifindex);
    }
}

impl<T: AsRef<[u8]>> Parseable<TcMirredBuffer<T>> for TcMirred {
    fn parse(buf: &TcMirredBuffer<T>) -> Result<Self, DecodeError> {
        Ok(Self {
            index: buf.index(),
            capab: buf.capab(),
            action: buf.action(),
            refcnt: buf.refcnt(),
            bindcnt: buf.bindcnt(),
            eaction: buf.eaction(),
            ifindex: buf.ifindex(),
        })
    }
}
