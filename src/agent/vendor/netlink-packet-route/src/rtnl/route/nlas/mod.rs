mod cache_info;
pub use self::cache_info::*;

mod metrics;
pub use self::metrics::*;

mod mfc_stats;
pub use self::mfc_stats::*;

use anyhow::Context;
use byteorder::{ByteOrder, NativeEndian};

use crate::{
    constants::*,
    nlas::{self, DefaultNla, NlaBuffer},
    parsers::{parse_u16, parse_u32},
    traits::Parseable,
    DecodeError,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Nla {
    Unspec(Vec<u8>),
    Destination(Vec<u8>),
    Source(Vec<u8>),
    Gateway(Vec<u8>),
    PrefSource(Vec<u8>),
    Metrics(Vec<u8>),
    MultiPath(Vec<u8>),
    CacheInfo(Vec<u8>),
    Session(Vec<u8>),
    MpAlgo(Vec<u8>),
    MfcStats(Vec<u8>),
    Via(Vec<u8>),
    NewDestination(Vec<u8>),
    Pref(Vec<u8>),
    Encap(Vec<u8>),
    Expires(Vec<u8>),
    Pad(Vec<u8>),
    Uid(Vec<u8>),
    TtlPropagate(Vec<u8>),
    EncapType(u16),
    Iif(u32),
    Oif(u32),
    Priority(u32),
    ProtocolInfo(u32),
    Flow(u32),
    Table(u32),
    Mark(u32),
    Other(DefaultNla),
}

impl nlas::Nla for Nla {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::Nla::*;
        match *self {
            Unspec(ref bytes)
                | Destination(ref bytes)
                | Source(ref bytes)
                | Gateway(ref bytes)
                | PrefSource(ref bytes)
                | MultiPath(ref bytes)
                | Session(ref bytes)
                | MpAlgo(ref bytes)
                | Via(ref bytes)
                | NewDestination(ref bytes)
                | Pref(ref bytes)
                | Encap(ref bytes)
                | Expires(ref bytes)
                | Pad(ref bytes)
                | Uid(ref bytes)
                | TtlPropagate(ref bytes)
                | CacheInfo(ref bytes)
                | MfcStats(ref bytes)
                | Metrics(ref bytes)
                => bytes.len(),

            EncapType(_) => 2,
            Iif(_)
                | Oif(_)
                | Priority(_)
                | ProtocolInfo(_)
                | Flow(_)
                | Table(_)
                | Mark(_)
                => 4,

            Other(ref attr) => attr.value_len(),
        }
    }

    #[rustfmt::skip]
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::Nla::*;
        match *self {
            Unspec(ref bytes)
                | Destination(ref bytes)
                | Source(ref bytes)
                | Gateway(ref bytes)
                | PrefSource(ref bytes)
                | MultiPath(ref bytes)
                | Session(ref bytes)
                | MpAlgo(ref bytes)
                | Via(ref bytes)
                | NewDestination(ref bytes)
                | Pref(ref bytes)
                | Encap(ref bytes)
                | Expires(ref bytes)
                | Pad(ref bytes)
                | Uid(ref bytes)
                | TtlPropagate(ref bytes)
                | CacheInfo(ref bytes)
                | MfcStats(ref bytes)
                | Metrics(ref bytes)
                => buffer.copy_from_slice(bytes.as_slice()),
            EncapType(value) => NativeEndian::write_u16(buffer, value),
            Iif(value)
                | Oif(value)
                | Priority(value)
                | ProtocolInfo(value)
                | Flow(value)
                | Table(value)
                | Mark(value)
                => NativeEndian::write_u32(buffer, value),
            Other(ref attr) => attr.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::Nla::*;
        match *self {
            Unspec(_) => RTA_UNSPEC,
            Destination(_) => RTA_DST,
            Source(_) => RTA_SRC,
            Iif(_) => RTA_IIF,
            Oif(_) => RTA_OIF,
            Gateway(_) => RTA_GATEWAY,
            Priority(_) => RTA_PRIORITY,
            PrefSource(_) => RTA_PREFSRC,
            Metrics(_) => RTA_METRICS,
            MultiPath(_) => RTA_MULTIPATH,
            ProtocolInfo(_) => RTA_PROTOINFO,
            Flow(_) => RTA_FLOW,
            CacheInfo(_) => RTA_CACHEINFO,
            Session(_) => RTA_SESSION,
            MpAlgo(_) => RTA_MP_ALGO,
            Table(_) => RTA_TABLE,
            Mark(_) => RTA_MARK,
            MfcStats(_) => RTA_MFC_STATS,
            Via(_) => RTA_VIA,
            NewDestination(_) => RTA_NEWDST,
            Pref(_) => RTA_PREF,
            EncapType(_) => RTA_ENCAP_TYPE,
            Encap(_) => RTA_ENCAP,
            Expires(_) => RTA_EXPIRES,
            Pad(_) => RTA_PAD,
            Uid(_) => RTA_UID,
            TtlPropagate(_) => RTA_TTL_PROPAGATE,
            Other(ref attr) => attr.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for Nla {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::Nla::*;

        let payload = buf.value();
        Ok(match buf.kind() {
            RTA_UNSPEC => Unspec(payload.to_vec()),
            RTA_DST => Destination(payload.to_vec()),
            RTA_SRC => Source(payload.to_vec()),
            RTA_GATEWAY => Gateway(payload.to_vec()),
            RTA_PREFSRC => PrefSource(payload.to_vec()),
            RTA_MULTIPATH => MultiPath(payload.to_vec()),
            RTA_SESSION => Session(payload.to_vec()),
            RTA_MP_ALGO => MpAlgo(payload.to_vec()),
            RTA_VIA => Via(payload.to_vec()),
            RTA_NEWDST => NewDestination(payload.to_vec()),
            RTA_PREF => Pref(payload.to_vec()),
            RTA_ENCAP => Encap(payload.to_vec()),
            RTA_EXPIRES => Expires(payload.to_vec()),
            RTA_PAD => Pad(payload.to_vec()),
            RTA_UID => Uid(payload.to_vec()),
            RTA_TTL_PROPAGATE => TtlPropagate(payload.to_vec()),
            RTA_ENCAP_TYPE => {
                EncapType(parse_u16(payload).context("invalid RTA_ENCAP_TYPE value")?)
            }
            RTA_IIF => Iif(parse_u32(payload).context("invalid RTA_IIF value")?),
            RTA_OIF => Oif(parse_u32(payload).context("invalid RTA_OIF value")?),
            RTA_PRIORITY => Priority(parse_u32(payload).context("invalid RTA_PRIORITY value")?),
            RTA_PROTOINFO => {
                ProtocolInfo(parse_u32(payload).context("invalid RTA_PROTOINFO value")?)
            }
            RTA_FLOW => Flow(parse_u32(payload).context("invalid RTA_FLOW value")?),
            RTA_TABLE => Table(parse_u32(payload).context("invalid RTA_TABLE value")?),
            RTA_MARK => Mark(parse_u32(payload).context("invalid RTA_MARK value")?),
            RTA_CACHEINFO => CacheInfo(payload.to_vec()),
            RTA_MFC_STATS => MfcStats(payload.to_vec()),
            RTA_METRICS => Metrics(payload.to_vec()),
            _ => Other(DefaultNla::parse(buf).context("invalid NLA (unknown kind)")?),
        })
    }
}
