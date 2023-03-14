// SPDX-License-Identifier: MIT

use anyhow::Context;

use crate::{
    constants::*,
    nlas::{DefaultNla, Nla, NlaBuffer},
    parsers::parse_u8,
    traits::Parseable,
    DecodeError,
};

/// Netlink attributes for `RTA_ENCAP` with `RTA_ENCAP_TYPE` set to `LWTUNNEL_ENCAP_MPLS`.
pub enum MplsIpTunnel {
    Destination(Vec<u8>),
    Ttl(u8),
    Other(DefaultNla),
}

impl Nla for MplsIpTunnel {
    fn value_len(&self) -> usize {
        use self::MplsIpTunnel::*;
        match self {
            Destination(bytes) => bytes.len(),
            Ttl(_) => 1,
            Other(attr) => attr.value_len(),
        }
    }

    fn kind(&self) -> u16 {
        use self::MplsIpTunnel::*;
        match self {
            Destination(_) => MPLS_IPTUNNEL_DST,
            Ttl(_) => MPLS_IPTUNNEL_TTL,
            Other(attr) => attr.kind(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::MplsIpTunnel::*;
        match self {
            Destination(bytes) => buffer.copy_from_slice(bytes.as_slice()),
            Ttl(ttl) => buffer[0] = *ttl,
            Other(attr) => attr.emit_value(buffer),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for MplsIpTunnel {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::MplsIpTunnel::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            MPLS_IPTUNNEL_DST => Destination(payload.to_vec()),
            MPLS_IPTUNNEL_TTL => Ttl(parse_u8(payload).context("invalid MPLS_IPTUNNEL_TTL value")?),
            _ => Other(DefaultNla::parse(buf).context("invalid NLA value (unknown type) value")?),
        })
    }
}
