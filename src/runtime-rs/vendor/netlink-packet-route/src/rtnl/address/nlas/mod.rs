// SPDX-License-Identifier: MIT

mod cache_info;
pub use self::cache_info::*;

use std::mem::size_of;

use anyhow::Context;
use byteorder::{ByteOrder, NativeEndian};

use crate::{
    constants::*,
    nlas::{self, DefaultNla, NlaBuffer},
    parsers::{parse_string, parse_u32},
    traits::Parseable,
    DecodeError,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Nla {
    Unspec(Vec<u8>),
    Address(Vec<u8>),
    Local(Vec<u8>),
    Label(String),
    Broadcast(Vec<u8>),
    Anycast(Vec<u8>),
    CacheInfo(Vec<u8>),
    Multicast(Vec<u8>),
    Flags(u32),
    Other(DefaultNla),
}

impl nlas::Nla for Nla {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::Nla::*;
        match *self {
            // Vec<u8>
            Unspec(ref bytes)
                | Address(ref bytes)
                | Local(ref bytes)
                | Broadcast(ref bytes)
                | Anycast(ref bytes)
                | Multicast(ref bytes) => bytes.len(),

            // strings: +1 because we need to append a nul byte
            Label(ref string) => string.as_bytes().len() + 1,

            // u32
            Flags(_) => size_of::<u32>(),

            // Native
            CacheInfo(ref buffer) => buffer.len(),

            // Defaults
            Other(ref attr)  => attr.value_len(),
        }
    }

    #[rustfmt::skip]
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::Nla::*;
        match *self {
            // Vec<u8>
            Unspec(ref bytes)
                | Address(ref bytes)
                | Local(ref bytes)
                | Broadcast(ref bytes)
                | Anycast(ref bytes)
                | CacheInfo(ref bytes)
                | Multicast(ref bytes) => buffer.copy_from_slice(bytes.as_slice()),

            // String
            Label(ref string) => {
                buffer[..string.len()].copy_from_slice(string.as_bytes());
                buffer[string.len()] = 0;
            }

            // u32
            Flags(ref value) => NativeEndian::write_u32(buffer, *value),


            // Default
            Other(ref attr) => attr.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::Nla::*;
        match *self {
            Unspec(_) => IFA_UNSPEC,
            Address(_) => IFA_ADDRESS,
            Local(_) => IFA_LOCAL,
            Label(_) => IFA_LABEL,
            Broadcast(_) => IFA_BROADCAST,
            Anycast(_) => IFA_ANYCAST,
            CacheInfo(_) => IFA_CACHEINFO,
            Multicast(_) => IFA_MULTICAST,
            Flags(_) => IFA_FLAGS,
            Other(ref nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for Nla {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::Nla::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFA_UNSPEC => Unspec(payload.to_vec()),
            IFA_ADDRESS => Address(payload.to_vec()),
            IFA_LOCAL => Local(payload.to_vec()),
            IFA_LABEL => Label(parse_string(payload).context("invalid IFA_LABEL value")?),
            IFA_BROADCAST => Broadcast(payload.to_vec()),
            IFA_ANYCAST => Anycast(payload.to_vec()),
            IFA_CACHEINFO => CacheInfo(payload.to_vec()),
            IFA_MULTICAST => Multicast(payload.to_vec()),
            IFA_FLAGS => Flags(parse_u32(payload).context("invalid IFA_FLAGS value")?),
            kind => Other(DefaultNla::parse(buf).context(format!("unknown NLA type {}", kind))?),
        })
    }
}
