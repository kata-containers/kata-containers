// SPDX-License-Identifier: MIT

use crate::{
    nlas,
    nlas::DefaultNla,
    utils::{
        byteorder::{ByteOrder, NativeEndian},
        nla::NlaBuffer,
        parsers::{parse_string, parse_u32, parse_u8},
        Parseable,
    },
    DecodeError,
    FRA_DPORT_RANGE,
    FRA_DST,
    FRA_FLOW,
    FRA_FWMARK,
    FRA_FWMASK,
    FRA_GOTO,
    FRA_IIFNAME,
    FRA_IP_PROTO,
    FRA_L3MDEV,
    FRA_OIFNAME,
    FRA_PAD,
    FRA_PRIORITY,
    FRA_PROTOCOL,
    FRA_SPORT_RANGE,
    FRA_SRC,
    FRA_SUPPRESS_IFGROUP,
    FRA_SUPPRESS_PREFIXLEN,
    FRA_TABLE,
    FRA_TUN_ID,
    FRA_UID_RANGE,
    FRA_UNSPEC,
};
use anyhow::Context;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Nla {
    Unspec(Vec<u8>),
    /// destination address
    Destination(Vec<u8>),
    /// source address
    Source(Vec<u8>),
    /// input interface name
    Iifname(String),
    /// target to jump to when used with rule action `FR_ACT_GOTO`
    Goto(u32),
    Priority(u32),
    FwMark(u32),
    FwMask(u32),
    /// flow class id,
    Flow(u32),
    TunId(u32),
    SuppressIfGroup(u32),
    SuppressPrefixLen(u32),
    Table(u32),
    /// output interface name
    OifName(String),
    Pad(Vec<u8>),
    /// iif or oif is l3mdev goto its table
    L3MDev(u8),
    UidRange(Vec<u8>),
    /// RTPROT_*
    Protocol(u8),
    /// AF_*
    IpProto(u8),
    SourcePortRange(Vec<u8>),
    DestinationPortRange(Vec<u8>),
    Other(DefaultNla),
}

impl nlas::Nla for Nla {
    fn value_len(&self) -> usize {
        use self::Nla::*;
        match self {
            Unspec(ref bytes)
            | Destination(ref bytes)
            | Source(ref bytes)
            | Pad(ref bytes)
            | UidRange(ref bytes)
            | SourcePortRange(ref bytes)
            | DestinationPortRange(ref bytes) => bytes.len(),
            Iifname(ref s) | OifName(ref s) => s.as_bytes().len() + 1,
            Priority(_) | FwMark(_) | FwMask(_) | Flow(_) | TunId(_) | Goto(_)
            | SuppressIfGroup(_) | SuppressPrefixLen(_) | Table(_) => 4,
            L3MDev(_) | Protocol(_) | IpProto(_) => 1,
            Other(attr) => attr.value_len(),
        }
    }

    fn kind(&self) -> u16 {
        use self::Nla::*;
        match self {
            Unspec(_) => FRA_UNSPEC,
            Destination(_) => FRA_DST,
            Source(_) => FRA_SRC,
            Iifname(_) => FRA_IIFNAME,
            Goto(_) => FRA_GOTO,
            Priority(_) => FRA_PRIORITY,
            FwMark(_) => FRA_FWMARK,
            FwMask(_) => FRA_FWMASK,
            Flow(_) => FRA_FLOW,
            TunId(_) => FRA_TUN_ID,
            SuppressIfGroup(_) => FRA_SUPPRESS_IFGROUP,
            SuppressPrefixLen(_) => FRA_SUPPRESS_PREFIXLEN,
            Table(_) => FRA_TABLE,
            OifName(_) => FRA_OIFNAME,
            Pad(_) => FRA_PAD,
            L3MDev(_) => FRA_L3MDEV,
            UidRange(_) => FRA_UID_RANGE,
            Protocol(_) => FRA_PROTOCOL,
            IpProto(_) => FRA_IP_PROTO,
            SourcePortRange(_) => FRA_SPORT_RANGE,
            DestinationPortRange(_) => FRA_DPORT_RANGE,
            Other(attr) => attr.kind(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::Nla::*;
        match self {
            Unspec(ref bytes)
            | Destination(ref bytes)
            | Source(ref bytes)
            | Pad(ref bytes)
            | UidRange(ref bytes)
            | SourcePortRange(ref bytes)
            | DestinationPortRange(ref bytes) => buffer.copy_from_slice(bytes.as_slice()),
            Iifname(ref s) | OifName(ref s) => buffer.copy_from_slice(s.as_bytes()),

            Priority(value)
            | FwMark(value)
            | FwMask(value)
            | Flow(value)
            | TunId(value)
            | Goto(value)
            | SuppressIfGroup(value)
            | SuppressPrefixLen(value)
            | Table(value) => NativeEndian::write_u32(buffer, *value),
            L3MDev(value) | Protocol(value) | IpProto(value) => buffer[0] = *value,
            Other(attr) => attr.emit_value(buffer),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for Nla {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use Nla::*;

        let payload = buf.value();

        Ok(match buf.kind() {
            FRA_UNSPEC => Unspec(payload.to_vec()),
            FRA_DST => Destination(payload.to_vec()),
            FRA_SRC => Source(payload.to_vec()),
            FRA_IIFNAME => Iifname(parse_string(payload).context("invalid FRA_IIFNAME value")?),
            FRA_GOTO => Goto(parse_u32(payload).context("invalid FRA_GOTO value")?),
            FRA_PRIORITY => Priority(parse_u32(payload).context("invalid FRA_PRIORITY value")?),
            FRA_FWMARK => FwMark(parse_u32(payload).context("invalid FRA_FWMARK value")?),
            FRA_FLOW => Flow(parse_u32(payload).context("invalid FRA_FLOW value")?),
            FRA_TUN_ID => TunId(parse_u32(payload).context("invalid FRA_TUN_ID value")?),
            FRA_SUPPRESS_IFGROUP => {
                SuppressIfGroup(parse_u32(payload).context("invalid FRA_SUPPRESS_IFGROUP value")?)
            }
            FRA_SUPPRESS_PREFIXLEN => SuppressPrefixLen(
                parse_u32(payload).context("invalid FRA_SUPPRESS_PREFIXLEN value")?,
            ),
            FRA_TABLE => Table(parse_u32(payload).context("invalid FRA_TABLE value")?),
            FRA_FWMASK => FwMask(parse_u32(payload).context("invalid FRA_FWMASK value")?),
            FRA_OIFNAME => OifName(parse_string(payload).context("invalid FRA_OIFNAME value")?),
            FRA_PAD => Pad(payload.to_vec()),
            FRA_L3MDEV => L3MDev(parse_u8(payload).context("invalid FRA_L3MDEV value")?),
            FRA_UID_RANGE => UidRange(payload.to_vec()),
            FRA_PROTOCOL => Protocol(parse_u8(payload).context("invalid FRA_PROTOCOL value")?),
            FRA_IP_PROTO => IpProto(parse_u8(payload).context("invalid FRA_IP_PROTO value")?),
            FRA_SPORT_RANGE => SourcePortRange(payload.to_vec()),
            FRA_DPORT_RANGE => DestinationPortRange(payload.to_vec()),
            _ => Other(DefaultNla::parse(buf).context("invalid NLA (unknown kind)")?),
        })
    }
}
