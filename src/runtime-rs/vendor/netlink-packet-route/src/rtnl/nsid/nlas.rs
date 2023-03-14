// SPDX-License-Identifier: MIT

use anyhow::Context;
use byteorder::{ByteOrder, NativeEndian};

use crate::{
    constants::*,
    nlas::{self, DefaultNla, NlaBuffer},
    parsers::{parse_i32, parse_u32},
    traits::Parseable,
    DecodeError,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Nla {
    Unspec(Vec<u8>),
    Id(i32),
    Pid(u32),
    Fd(u32),
    Other(DefaultNla),
}

impl nlas::Nla for Nla {
    fn value_len(&self) -> usize {
        use self::Nla::*;
        match *self {
            Unspec(ref bytes) => bytes.len(),
            Id(_) | Pid(_) | Fd(_) => 4,
            Other(ref attr) => attr.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::Nla::*;
        match *self {
            Unspec(ref bytes) => buffer.copy_from_slice(bytes.as_slice()),
            Fd(ref value) | Pid(ref value) => NativeEndian::write_u32(buffer, *value),
            Id(ref value) => NativeEndian::write_i32(buffer, *value),
            Other(ref attr) => attr.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::Nla::*;
        match *self {
            Unspec(_) => NETNSA_NONE,
            Id(_) => NETNSA_NSID,
            Pid(_) => NETNSA_PID,
            Fd(_) => NETNSA_FD,
            Other(ref attr) => attr.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for Nla {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::Nla::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            NETNSA_NONE => Unspec(payload.to_vec()),
            NETNSA_NSID => Id(parse_i32(payload).context("invalid NETNSA_NSID")?),
            NETNSA_PID => Pid(parse_u32(payload).context("invalid NETNSA_PID")?),
            NETNSA_FD => Fd(parse_u32(payload).context("invalid NETNSA_FD")?),
            kind => Other(DefaultNla::parse(buf).context(format!("unknown NLA type {}", kind))?),
        })
    }
}
