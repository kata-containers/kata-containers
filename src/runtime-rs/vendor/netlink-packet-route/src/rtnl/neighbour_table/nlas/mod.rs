// SPDX-License-Identifier: MIT

mod config;
pub use config::*;

mod stats;
pub use stats::*;

use anyhow::Context;
use byteorder::{ByteOrder, NativeEndian};

use crate::{
    constants::*,
    nlas::{self, DefaultNla, NlaBuffer},
    parsers::{parse_string, parse_u32, parse_u64},
    traits::Parseable,
    DecodeError,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Nla {
    Unspec(Vec<u8>),
    // FIXME: parse this nla
    Parms(Vec<u8>),
    Name(String),
    Threshold1(u32),
    Threshold2(u32),
    Threshold3(u32),
    Config(Vec<u8>),
    Stats(Vec<u8>),
    GcInterval(u64),
    Other(DefaultNla),
}

impl nlas::Nla for Nla {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::Nla::*;
        match *self {
            Unspec(ref bytes) | Parms(ref bytes) | Config(ref bytes) | Stats(ref bytes)=> bytes.len(),
            // strings: +1 because we need to append a nul byte
            Name(ref s) => s.len() + 1,
            Threshold1(_) | Threshold2(_) | Threshold3(_) => 4,
            GcInterval(_) => 8,
            Other(ref attr) => attr.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::Nla::*;
        match *self {
            Unspec(ref bytes) | Parms(ref bytes) | Config(ref bytes) | Stats(ref bytes) => {
                buffer.copy_from_slice(bytes.as_slice())
            }
            Name(ref string) => {
                buffer[..string.len()].copy_from_slice(string.as_bytes());
                buffer[string.len()] = 0;
            }
            GcInterval(ref value) => NativeEndian::write_u64(buffer, *value),
            Threshold1(ref value) | Threshold2(ref value) | Threshold3(ref value) => {
                NativeEndian::write_u32(buffer, *value)
            }
            Other(ref attr) => attr.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::Nla::*;
        match *self {
            Unspec(_) => NDTA_UNSPEC,
            Name(_) => NDTA_NAME,
            Config(_) => NDTA_CONFIG,
            Stats(_) => NDTA_STATS,
            Parms(_) => NDTA_PARMS,
            GcInterval(_) => NDTA_GC_INTERVAL,
            Threshold1(_) => NDTA_THRESH1,
            Threshold2(_) => NDTA_THRESH2,
            Threshold3(_) => NDTA_THRESH3,
            Other(ref attr) => attr.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for Nla {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::Nla::*;

        let payload = buf.value();
        Ok(match buf.kind() {
            NDTA_UNSPEC => Unspec(payload.to_vec()),
            NDTA_NAME => Name(parse_string(payload).context("invalid NDTA_NAME value")?),
            NDTA_CONFIG => Config(payload.to_vec()),
            NDTA_STATS => Stats(payload.to_vec()),
            NDTA_PARMS => Parms(payload.to_vec()),
            NDTA_GC_INTERVAL => {
                GcInterval(parse_u64(payload).context("invalid NDTA_GC_INTERVAL value")?)
            }
            NDTA_THRESH1 => Threshold1(parse_u32(payload).context("invalid NDTA_THRESH1 value")?),
            NDTA_THRESH2 => Threshold2(parse_u32(payload).context("invalid NDTA_THRESH2 value")?),
            NDTA_THRESH3 => Threshold3(parse_u32(payload).context("invalid NDTA_THRESH3 value")?),
            kind => Other(DefaultNla::parse(buf).context(format!("unknown NLA type {}", kind))?),
        })
    }
}
