use crate::{
    constants::*,
    nlas::{DefaultNla, Nla, NlaBuffer, NlasIterator},
    parsers::{parse_mac, parse_string, parse_u16, parse_u16_be, parse_u32, parse_u64, parse_u8},
    traits::{Emitable, Parseable},
    DecodeError,
    LinkMessage,
    LinkMessageBuffer,
};
use anyhow::Context;
use byteorder::{BigEndian, ByteOrder, NativeEndian};

const DUMMY: &str = "dummy";
const IFB: &str = "ifb";
const BRIDGE: &str = "bridge";
const TUN: &str = "tun";
const NLMON: &str = "nlmon";
const VLAN: &str = "vlan";
const VETH: &str = "veth";
const VXLAN: &str = "vxlan";
const BOND: &str = "bond";
const IPVLAN: &str = "ipvlan";
const MACVLAN: &str = "macvlan";
const MACVTAP: &str = "macvtap";
const GRETAP: &str = "gretap";
const IP6GRETAP: &str = "ip6gretap";
const IPIP: &str = "ipip";
const SIT: &str = "sit";
const GRE: &str = "gre";
const IP6GRE: &str = "ip6gre";
const VTI: &str = "vti";
const VRF: &str = "vrf";
const GTP: &str = "gtp";
const IPOIB: &str = "ipoib";
const WIREGUARD: &str = "wireguard";

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Info {
    Unspec(Vec<u8>),
    Xstats(Vec<u8>),
    Kind(InfoKind),
    Data(InfoData),
    SlaveKind(Vec<u8>),
    SlaveData(Vec<u8>),
}

impl Nla for Info {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::Info::*;
        match self {
            Unspec(ref bytes)
                | Xstats(ref bytes)
                | SlaveKind(ref bytes)
                | SlaveData(ref bytes)
                => bytes.len(),
            Kind(ref nla) => nla.value_len(),
            Data(ref nla) => nla.value_len(),
        }
    }

    #[rustfmt::skip]
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::Info::*;
        match self {
            Unspec(ref bytes)
                | Xstats(ref bytes)
                | SlaveKind(ref bytes)
                | SlaveData(ref bytes)
                => buffer.copy_from_slice(bytes),
            Kind(ref nla) => nla.emit_value(buffer),
            Data(ref nla) => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::Info::*;
        match self {
            Unspec(_) => IFLA_INFO_UNSPEC,
            Xstats(_) => IFLA_INFO_XSTATS,
            SlaveKind(_) => IFLA_INFO_SLAVE_KIND,
            SlaveData(_) => IFLA_INFO_DATA,
            Kind(_) => IFLA_INFO_KIND,
            Data(_) => IFLA_INFO_DATA,
        }
    }
}

pub(crate) struct VecInfo(pub(crate) Vec<Info>);

// We cannot `impl Parseable<_> for Info` because some attributes
// depend on each other. To parse IFLA_INFO_DATA we first need to
// parse the preceding IFLA_INFO_KIND for example.
//
// Moreover, with cannot `impl Parseable for Vec<Info>` due to the
// orphan rule: `Parseable` and `Vec<_>` are both defined outside of
// this crate. Thus, we create this internal VecInfo struct that wraps
// `Vec<Info>` and allows us to circumvent the orphan rule.
//
// The downside is that this impl will not be exposed.
impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for VecInfo {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        let mut res = Vec::new();
        let nlas = NlasIterator::new(buf.into_inner());
        let mut link_info_kind: Option<InfoKind> = None;
        for nla in nlas {
            let nla = nla?;
            match nla.kind() {
                IFLA_INFO_UNSPEC => res.push(Info::Unspec(nla.value().to_vec())),
                IFLA_INFO_XSTATS => res.push(Info::Xstats(nla.value().to_vec())),
                IFLA_INFO_SLAVE_KIND => res.push(Info::SlaveKind(nla.value().to_vec())),
                IFLA_INFO_SLAVE_DATA => res.push(Info::SlaveData(nla.value().to_vec())),
                IFLA_INFO_KIND => {
                    let parsed = InfoKind::parse(&nla)?;
                    res.push(Info::Kind(parsed.clone()));
                    link_info_kind = Some(parsed);
                }
                IFLA_INFO_DATA => {
                    if let Some(link_info_kind) = link_info_kind {
                        let payload = nla.value();
                        let info_data = match link_info_kind {
                            InfoKind::Dummy => InfoData::Dummy(payload.to_vec()),
                            InfoKind::Ifb => InfoData::Ifb(payload.to_vec()),
                            InfoKind::Bridge => {
                                let mut v = Vec::new();
                                let err =
                                    "failed to parse IFLA_INFO_DATA (IFLA_INFO_KIND is 'bridge')";
                                for nla in NlasIterator::new(payload) {
                                    let nla = &nla.context(err)?;
                                    let parsed = InfoBridge::parse(nla).context(err)?;
                                    v.push(parsed);
                                }
                                InfoData::Bridge(v)
                            }
                            InfoKind::Vlan => {
                                let mut v = Vec::new();
                                let err =
                                    "failed to parse IFLA_INFO_DATA (IFLA_INFO_KIND is 'vlan')";
                                for nla in NlasIterator::new(payload) {
                                    let nla = &nla.context(err)?;
                                    let parsed = InfoVlan::parse(nla).context(err)?;
                                    v.push(parsed);
                                }
                                InfoData::Vlan(v)
                            }
                            InfoKind::Tun => InfoData::Tun(payload.to_vec()),
                            InfoKind::Nlmon => InfoData::Nlmon(payload.to_vec()),
                            InfoKind::Veth => {
                                let err =
                                    "failed to parse IFLA_INFO_DATA (IFLA_INFO_KIND is 'veth')";
                                let nla_buf = NlaBuffer::new_checked(&payload).context(err)?;
                                let parsed = VethInfo::parse(&nla_buf).context(err)?;
                                InfoData::Veth(parsed)
                            }
                            InfoKind::Vxlan => {
                                let mut v = Vec::new();
                                let err =
                                    "failed to parse IFLA_INFO_DATA (IFLA_INFO_KIND is 'vxlan')";
                                for nla in NlasIterator::new(payload) {
                                    let nla = &nla.context(err)?;
                                    let parsed = InfoVxlan::parse(nla).context(err)?;
                                    v.push(parsed);
                                }
                                InfoData::Vxlan(v)
                            }
                            InfoKind::Bond => InfoData::Bond(payload.to_vec()),
                            InfoKind::IpVlan => {
                                let mut v = Vec::new();
                                let err =
                                    "failed to parse IFLA_INFO_DATA (IFLA_INFO_KIND is 'ipvlan')";
                                for nla in NlasIterator::new(payload) {
                                    let nla = &nla.context(err)?;
                                    let parsed = InfoIpVlan::parse(nla).context(err)?;
                                    v.push(parsed);
                                }
                                InfoData::IpVlan(v)
                            }
                            InfoKind::MacVlan => {
                                let mut v = Vec::new();
                                let err =
                                    "failed to parse IFLA_INFO_DATA (IFLA_INFO_KIND is 'macvlan')";
                                for nla in NlasIterator::new(payload) {
                                    let nla = &nla.context(err)?;
                                    let parsed = InfoMacVlan::parse(nla).context(err)?;
                                    v.push(parsed);
                                }
                                InfoData::MacVlan(v)
                            }
                            InfoKind::MacVtap => {
                                let mut v = Vec::new();
                                let err =
                                    "failed to parse IFLA_INFO_DATA (IFLA_INFO_KIND is 'macvtap')";
                                for nla in NlasIterator::new(payload) {
                                    let nla = &nla.context(err)?;
                                    let parsed = InfoMacVtap::parse(nla).context(err)?;
                                    v.push(parsed);
                                }
                                InfoData::MacVtap(v)
                            }
                            InfoKind::GreTap => InfoData::GreTap(payload.to_vec()),
                            InfoKind::GreTap6 => InfoData::GreTap6(payload.to_vec()),
                            InfoKind::IpTun => InfoData::IpTun(payload.to_vec()),
                            InfoKind::SitTun => InfoData::SitTun(payload.to_vec()),
                            InfoKind::GreTun => InfoData::GreTun(payload.to_vec()),
                            InfoKind::GreTun6 => InfoData::GreTun6(payload.to_vec()),
                            InfoKind::Vti => InfoData::Vti(payload.to_vec()),
                            InfoKind::Vrf => {
                                let mut v = Vec::new();
                                let err =
                                    "failed to parse IFLA_INFO_DATA (IFLA_INFO_KIND is 'vrf')";
                                for nla in NlasIterator::new(payload) {
                                    let nla = &nla.context(err)?;
                                    let parsed = InfoVrf::parse(nla).context(err)?;
                                    v.push(parsed);
                                }
                                InfoData::Vrf(v)
                            }
                            InfoKind::Gtp => InfoData::Gtp(payload.to_vec()),
                            InfoKind::Ipoib => {
                                let mut v = Vec::new();
                                let err =
                                    "failed to parse IFLA_INFO_DATA (IFLA_INFO_KIND is 'ipoib')";
                                for nla in NlasIterator::new(payload) {
                                    let nla = &nla.context(err)?;
                                    let parsed = InfoIpoib::parse(nla).context(err)?;
                                    v.push(parsed);
                                }
                                InfoData::Ipoib(v)
                            }
                            InfoKind::Wireguard => InfoData::Wireguard(payload.to_vec()),
                            InfoKind::Other(_) => InfoData::Other(payload.to_vec()),
                        };
                        res.push(Info::Data(info_data));
                    } else {
                        return Err("IFLA_INFO_DATA is not preceded by an IFLA_INFO_KIND".into());
                    }
                    link_info_kind = None;
                }
                _ => return Err(format!("unknown NLA type {}", nla.kind()).into()),
            }
        }
        Ok(VecInfo(res))
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoData {
    Bridge(Vec<InfoBridge>),
    Tun(Vec<u8>),
    Nlmon(Vec<u8>),
    Vlan(Vec<InfoVlan>),
    Dummy(Vec<u8>),
    Ifb(Vec<u8>),
    Veth(VethInfo),
    Vxlan(Vec<InfoVxlan>),
    Bond(Vec<u8>),
    IpVlan(Vec<InfoIpVlan>),
    MacVlan(Vec<InfoMacVlan>),
    MacVtap(Vec<InfoMacVtap>),
    GreTap(Vec<u8>),
    GreTap6(Vec<u8>),
    IpTun(Vec<u8>),
    SitTun(Vec<u8>),
    GreTun(Vec<u8>),
    GreTun6(Vec<u8>),
    Vti(Vec<u8>),
    Vrf(Vec<InfoVrf>),
    Gtp(Vec<u8>),
    Ipoib(Vec<InfoIpoib>),
    Wireguard(Vec<u8>),
    Other(Vec<u8>),
}

impl Nla for InfoData {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::InfoData::*;
        match self {
            Bridge(ref nlas) => nlas.as_slice().buffer_len(),
            Vlan(ref nlas) =>  nlas.as_slice().buffer_len(),
            Veth(ref msg) => msg.buffer_len(),
            IpVlan(ref nlas) => nlas.as_slice().buffer_len(),
            Ipoib(ref nlas) => nlas.as_slice().buffer_len(),
            MacVlan(ref nlas) => nlas.as_slice().buffer_len(),
            MacVtap(ref nlas) => nlas.as_slice().buffer_len(),
            Vrf(ref nlas) => nlas.as_slice().buffer_len(),
            Vxlan(ref nlas) => nlas.as_slice().buffer_len(),
            Dummy(ref bytes)
                | Tun(ref bytes)
                | Nlmon(ref bytes)
                | Ifb(ref bytes)
                | Bond(ref bytes)
                | GreTap(ref bytes)
                | GreTap6(ref bytes)
                | IpTun(ref bytes)
                | SitTun(ref bytes)
                | GreTun(ref bytes)
                | GreTun6(ref bytes)
                | Vti(ref bytes)
                | Gtp(ref bytes)
                | Wireguard(ref bytes)
                | Other(ref bytes)
                => bytes.len(),
        }
    }

    #[rustfmt::skip]
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoData::*;
        match self {
            Bridge(ref nlas) => nlas.as_slice().emit(buffer),
            Vlan(ref nlas) => nlas.as_slice().emit(buffer),
            Veth(ref msg) => msg.emit(buffer),
            IpVlan(ref nlas) => nlas.as_slice().emit(buffer),
            Ipoib(ref nlas) => nlas.as_slice().emit(buffer),
            MacVlan(ref nlas) => nlas.as_slice().emit(buffer),
            MacVtap(ref nlas) => nlas.as_slice().emit(buffer),
            Vrf(ref nlas) => nlas.as_slice().emit(buffer),
            Vxlan(ref nlas) => nlas.as_slice().emit(buffer),
            Dummy(ref bytes)
                | Tun(ref bytes)
                | Nlmon(ref bytes)
                | Ifb(ref bytes)
                | Bond(ref bytes)
                | GreTap(ref bytes)
                | GreTap6(ref bytes)
                | IpTun(ref bytes)
                | SitTun(ref bytes)
                | GreTun(ref bytes)
                | GreTun6(ref bytes)
                | Vti(ref bytes)
                | Gtp(ref bytes)
                | Wireguard(ref bytes)
                | Other(ref bytes)
                => buffer.copy_from_slice(bytes),
        }
    }

    fn kind(&self) -> u16 {
        IFLA_INFO_DATA
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoKind {
    Dummy,
    Ifb,
    Bridge,
    Tun,
    Nlmon,
    Vlan,
    Veth,
    Vxlan,
    Bond,
    IpVlan,
    MacVlan,
    MacVtap,
    GreTap,
    GreTap6,
    IpTun,
    SitTun,
    GreTun,
    GreTun6,
    Vti,
    Vrf,
    Gtp,
    Ipoib,
    Wireguard,
    Other(String),
}

impl Nla for InfoKind {
    fn value_len(&self) -> usize {
        use self::InfoKind::*;
        let len = match *self {
            Dummy => DUMMY.len(),
            Ifb => IFB.len(),
            Bridge => BRIDGE.len(),
            Tun => TUN.len(),
            Nlmon => NLMON.len(),
            Vlan => VLAN.len(),
            Veth => VETH.len(),
            Vxlan => VXLAN.len(),
            Bond => BOND.len(),
            IpVlan => IPVLAN.len(),
            MacVlan => MACVLAN.len(),
            MacVtap => MACVTAP.len(),
            GreTap => GRETAP.len(),
            GreTap6 => IP6GRETAP.len(),
            IpTun => IPIP.len(),
            SitTun => SIT.len(),
            GreTun => GRE.len(),
            GreTun6 => IP6GRE.len(),
            Vti => VTI.len(),
            Vrf => VRF.len(),
            Gtp => GTP.len(),
            Ipoib => IPOIB.len(),
            Wireguard => WIREGUARD.len(),
            Other(ref s) => s.len(),
        };
        len + 1
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoKind::*;
        let s = match *self {
            Dummy => DUMMY,
            Ifb => IFB,
            Bridge => BRIDGE,
            Tun => TUN,
            Nlmon => NLMON,
            Vlan => VLAN,
            Veth => VETH,
            Vxlan => VXLAN,
            Bond => BOND,
            IpVlan => IPVLAN,
            MacVlan => MACVLAN,
            MacVtap => MACVTAP,
            GreTap => GRETAP,
            GreTap6 => IP6GRETAP,
            IpTun => IPIP,
            SitTun => SIT,
            GreTun => GRE,
            GreTun6 => IP6GRE,
            Vti => VTI,
            Vrf => VRF,
            Gtp => GTP,
            Ipoib => IPOIB,
            Wireguard => WIREGUARD,
            Other(ref s) => s.as_str(),
        };
        buffer[..s.len()].copy_from_slice(s.as_bytes());
        buffer[s.len()] = 0;
    }

    fn kind(&self) -> u16 {
        IFLA_INFO_KIND
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for InfoKind {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<InfoKind, DecodeError> {
        use self::InfoKind::*;
        if buf.kind() != IFLA_INFO_KIND {
            return Err(
                format!("failed to parse IFLA_INFO_KIND: NLA type is {}", buf.kind()).into(),
            );
        }
        let s = parse_string(buf.value()).context("invalid IFLA_INFO_KIND value")?;
        Ok(match s.as_str() {
            DUMMY => Dummy,
            IFB => Ifb,
            BRIDGE => Bridge,
            TUN => Tun,
            NLMON => Nlmon,
            VLAN => Vlan,
            VETH => Veth,
            VXLAN => Vxlan,
            BOND => Bond,
            IPVLAN => IpVlan,
            MACVLAN => MacVlan,
            MACVTAP => MacVtap,
            GRETAP => GreTap,
            IP6GRETAP => GreTap6,
            IPIP => IpTun,
            SIT => SitTun,
            GRE => GreTun,
            IP6GRE => GreTun6,
            VTI => Vti,
            VRF => Vrf,
            GTP => Gtp,
            IPOIB => Ipoib,
            WIREGUARD => Wireguard,
            _ => Other(s),
        })
    }
}

// https://elixir.bootlin.com/linux/v5.9.8/source/drivers/net/vxlan.c#L3332
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoVxlan {
    Unspec(Vec<u8>),
    Id(u32),
    Group(Vec<u8>),
    Group6(Vec<u8>),
    Link(u32),
    Local(Vec<u8>),
    Local6(Vec<u8>),
    Tos(u8),
    Ttl(u8),
    Label(u32),
    Learning(u8),
    Ageing(u32),
    Limit(u32),
    PortRange((u16, u16)),
    Proxy(u8),
    Rsc(u8),
    L2Miss(u8),
    L3Miss(u8),
    CollectMetadata(u8),
    Port(u16),
    UDPCsum(u8),
    UDPZeroCsumTX(u8),
    UDPZeroCsumRX(u8),
    RemCsumTX(u8),
    RemCsumRX(u8),
    Gbp(u8),
    Gpe(u8),
    RemCsumNoPartial(u8),
    TtlInherit(u8),
    Df(u8),
}

impl Nla for InfoVxlan {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::InfoVxlan::*;
        match *self {
            Tos(_)
                | Ttl(_)
                | Learning(_)
                | Proxy(_)
                | Rsc(_)
                | L2Miss(_)
                | L3Miss(_)
                | CollectMetadata(_)
                | UDPCsum(_)
                | UDPZeroCsumTX(_)
                | UDPZeroCsumRX(_)
                | RemCsumTX(_)
                | RemCsumRX(_)
                | Gbp(_)
                | Gpe(_)
                | RemCsumNoPartial(_)
                | TtlInherit(_)
                | Df(_)
            => 1,
            Port(_) => 2,
            Id(_)
                | Label(_)
                | Link(_)
                | Ageing(_)
                | Limit(_)
                | PortRange(_)
            => 4,
            Local(ref bytes)
                | Local6(ref bytes)
                | Group(ref bytes)
                | Group6(ref bytes)
                | Unspec(ref bytes)
            => bytes.len(),
        }
    }

    #[rustfmt::skip]
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoVxlan::*;
        match self {
            Unspec(ref bytes) => buffer.copy_from_slice(bytes),
            Id(ref value)
                | Label(ref value)
                | Link(ref value)
                | Ageing(ref value)
                | Limit(ref value)
            => NativeEndian::write_u32(buffer, *value),
            Tos(ref value)
                | Ttl(ref value)
                | Learning (ref value)
                | Proxy(ref value)
                | Rsc(ref value)
                | L2Miss(ref value)
                | L3Miss(ref value)
                | CollectMetadata(ref value)
                | UDPCsum(ref value)
                | UDPZeroCsumTX(ref value)
                | UDPZeroCsumRX(ref value)
                | RemCsumTX(ref value)
                | RemCsumRX(ref value)
                | Gbp(ref value)
                | Gpe(ref value)
                | RemCsumNoPartial(ref value)
                | TtlInherit(ref value)
                | Df(ref value)
            =>  buffer[0] = *value,
            Local(ref value)
                | Group(ref value)
                | Group6(ref value)
                | Local6(ref value)
            => buffer.copy_from_slice(value.as_slice()),
            Port(ref value) => NativeEndian::write_u16(buffer, *value),
            PortRange(ref range) => {
                NativeEndian::write_u16(buffer, range.0);
                NativeEndian::write_u16(buffer, range.1)
            }
        }
    }

    fn kind(&self) -> u16 {
        use self::InfoVxlan::*;

        match self {
            Id(_) => IFLA_VXLAN_ID,
            Group(_) => IFLA_VXLAN_GROUP,
            Group6(_) => IFLA_VXLAN_GROUP6,
            Link(_) => IFLA_VXLAN_LINK,
            Local(_) => IFLA_VXLAN_LOCAL,
            Local6(_) => IFLA_VXLAN_LOCAL,
            Tos(_) => IFLA_VXLAN_TOS,
            Ttl(_) => IFLA_VXLAN_TTL,
            Label(_) => IFLA_VXLAN_LABEL,
            Learning(_) => IFLA_VXLAN_LEARNING,
            Ageing(_) => IFLA_VXLAN_AGEING,
            Limit(_) => IFLA_VXLAN_LIMIT,
            PortRange(_) => IFLA_VXLAN_PORT_RANGE,
            Proxy(_) => IFLA_VXLAN_PROXY,
            Rsc(_) => IFLA_VXLAN_RSC,
            L2Miss(_) => IFLA_VXLAN_L2MISS,
            L3Miss(_) => IFLA_VXLAN_L3MISS,
            CollectMetadata(_) => IFLA_VXLAN_COLLECT_METADATA,
            Port(_) => IFLA_VXLAN_PORT,
            UDPCsum(_) => IFLA_VXLAN_UDP_CSUM,
            UDPZeroCsumTX(_) => IFLA_VXLAN_UDP_ZERO_CSUM6_TX,
            UDPZeroCsumRX(_) => IFLA_VXLAN_UDP_ZERO_CSUM6_RX,
            RemCsumTX(_) => IFLA_VXLAN_REMCSUM_TX,
            RemCsumRX(_) => IFLA_VXLAN_REMCSUM_RX,
            Gbp(_) => IFLA_VXLAN_GBP,
            Gpe(_) => IFLA_VXLAN_GPE,
            RemCsumNoPartial(_) => IFLA_VXLAN_REMCSUM_NOPARTIAL,
            TtlInherit(_) => IFLA_VXLAN_TTL_INHERIT,
            Df(_) => IFLA_VXLAN_DF,
            Unspec(_) => IFLA_VXLAN_UNSPEC,
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for InfoVxlan {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::InfoVxlan::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_VLAN_UNSPEC => Unspec(payload.to_vec()),
            IFLA_VXLAN_ID => Id(parse_u32(payload).context("invalid IFLA_VXLAN_ID value")?),
            IFLA_VXLAN_GROUP => Group(payload.to_vec()),
            IFLA_VXLAN_GROUP6 => Group6(payload.to_vec()),
            IFLA_VXLAN_LINK => Link(parse_u32(payload).context("invalid IFLA_VXLAN_LINK value")?),
            IFLA_VXLAN_LOCAL => Local(payload.to_vec()),
            IFLA_VXLAN_LOCAL6 => Local6(payload.to_vec()),
            IFLA_VXLAN_TOS => Tos(parse_u8(payload).context("invalid IFLA_VXLAN_TOS value")?),
            IFLA_VXLAN_TTL => Ttl(parse_u8(payload).context("invalid IFLA_VXLAN_TTL value")?),
            IFLA_VXLAN_LABEL => {
                Label(parse_u32(payload).context("invalid IFLA_VXLAN_LABEL value")?)
            }
            IFLA_VXLAN_LEARNING => {
                Learning(parse_u8(payload).context("invalid IFLA_VXLAN_LEARNING value")?)
            }
            IFLA_VXLAN_AGEING => {
                Ageing(parse_u32(payload).context("invalid IFLA_VXLAN_AGEING value")?)
            }
            IFLA_VXLAN_LIMIT => {
                Limit(parse_u32(payload).context("invalid IFLA_VXLAN_LIMIT value")?)
            }
            IFLA_VXLAN_PROXY => Proxy(parse_u8(payload).context("invalid IFLA_VXLAN_PROXY value")?),
            IFLA_VXLAN_RSC => Rsc(parse_u8(payload).context("invalid IFLA_VXLAN_RSC value")?),
            IFLA_VXLAN_L2MISS => {
                L2Miss(parse_u8(payload).context("invalid IFLA_VXLAN_L2MISS value")?)
            }
            IFLA_VXLAN_L3MISS => {
                L3Miss(parse_u8(payload).context("invalid IFLA_VXLAN_L3MISS value")?)
            }
            IFLA_VXLAN_COLLECT_METADATA => CollectMetadata(
                parse_u8(payload).context("invalid IFLA_VXLAN_COLLECT_METADATA value")?,
            ),
            IFLA_VXLAN_PORT_RANGE => {
                let err = "invalid IFLA_VXLAN_PORT value";
                if payload.len() != 4 {
                    return Err(err.into());
                }
                let low = parse_u16(&payload[0..2]).context(err)?;
                let high = parse_u16(&payload[2..]).context(err)?;
                PortRange((low, high))
            }
            IFLA_VXLAN_PORT => {
                Port(parse_u16_be(payload).context("invalid IFLA_VXLAN_PORT value")?)
            }
            IFLA_VXLAN_UDP_CSUM => {
                UDPCsum(parse_u8(payload).context("invalid IFLA_VXLAN_UDP_CSUM value")?)
            }
            IFLA_VXLAN_UDP_ZERO_CSUM6_TX => UDPZeroCsumTX(
                parse_u8(payload).context("invalid IFLA_VXLAN_UDP_ZERO_CSUM6_TX value")?,
            ),
            IFLA_VXLAN_UDP_ZERO_CSUM6_RX => UDPZeroCsumRX(
                parse_u8(payload).context("invalid IFLA_VXLAN_UDP_ZERO_CSUM6_RX value")?,
            ),
            IFLA_VXLAN_REMCSUM_TX => {
                RemCsumTX(parse_u8(payload).context("invalid IFLA_VXLAN_REMCSUM_TX value")?)
            }
            IFLA_VXLAN_REMCSUM_RX => {
                RemCsumRX(parse_u8(payload).context("invalid IFLA_VXLAN_REMCSUM_RX value")?)
            }
            IFLA_VXLAN_DF => Df(parse_u8(payload).context("invalid IFLA_VXLAN_DF value")?),
            IFLA_VXLAN_GBP => Gbp(parse_u8(payload).context("invalid IFLA_VXLAN_GBP value")?),
            IFLA_VXLAN_GPE => Gpe(parse_u8(payload).context("invalid IFLA_VXLAN_GPE value")?),
            IFLA_VXLAN_REMCSUM_NOPARTIAL => RemCsumNoPartial(
                parse_u8(payload).context("invalid IFLA_VXLAN_REMCSUM_NO_PARTIAL")?,
            ),
            IFLA_VXLAN_TTL_INHERIT => {
                TtlInherit(parse_u8(payload).context("invalid IFLA_VXLAN_TTL_INHERIT value")?)
            }
            __IFLA_VXLAN_MAX => Unspec(payload.to_vec()),
            _ => return Err(format!("unknown NLA type {}", buf.kind()).into()),
        })
    }
}

// https://elixir.bootlin.com/linux/latest/source/net/8021q/vlan_netlink.c#L21
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoVlan {
    Unspec(Vec<u8>),
    Id(u16),
    Flags((u32, u32)),
    EgressQos(Vec<u8>),
    IngressQos(Vec<u8>),
    Protocol(u16),
}

impl Nla for InfoVlan {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::InfoVlan::*;
        match self {
            Id(_) | Protocol(_) => 2,
            Flags(_) => 8,
            Unspec(bytes)
                | EgressQos(bytes)
                | IngressQos(bytes)
                => bytes.len(),
        }
    }

    #[rustfmt::skip]
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoVlan::*;
        match self {
            Unspec(ref bytes)
                | EgressQos(ref bytes)
                | IngressQos(ref bytes)
                => buffer.copy_from_slice(bytes),

            Id(ref value)
                | Protocol(ref value)
                => NativeEndian::write_u16(buffer, *value),

            Flags(ref flags) => {
                NativeEndian::write_u32(buffer, flags.0);
                NativeEndian::write_u32(buffer, flags.1)
            }
        }
    }

    fn kind(&self) -> u16 {
        use self::InfoVlan::*;
        match self {
            Unspec(_) => IFLA_VLAN_UNSPEC,
            Id(_) => IFLA_VLAN_ID,
            Flags(_) => IFLA_VLAN_FLAGS,
            EgressQos(_) => IFLA_VLAN_EGRESS_QOS,
            IngressQos(_) => IFLA_VLAN_INGRESS_QOS,
            Protocol(_) => IFLA_VLAN_PROTOCOL,
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for InfoVlan {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::InfoVlan::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_VLAN_UNSPEC => Unspec(payload.to_vec()),
            IFLA_VLAN_ID => Id(parse_u16(payload).context("invalid IFLA_VLAN_ID value")?),
            IFLA_VLAN_FLAGS => {
                let err = "invalid IFLA_VLAN_FLAGS value";
                if payload.len() != 8 {
                    return Err(err.into());
                }
                let flags = parse_u32(&payload[0..4]).context(err)?;
                let mask = parse_u32(&payload[4..]).context(err)?;
                Flags((flags, mask))
            }
            IFLA_VLAN_EGRESS_QOS => EgressQos(payload.to_vec()),
            IFLA_VLAN_INGRESS_QOS => IngressQos(payload.to_vec()),
            IFLA_VLAN_PROTOCOL => {
                Protocol(parse_u16_be(payload).context("invalid IFLA_VLAN_PROTOCOL value")?)
            }
            _ => return Err(format!("unknown NLA type {}", buf.kind()).into()),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoBridge {
    Unspec(Vec<u8>),
    GroupAddr([u8; 6]),
    // FIXME: what type is this? putting Vec<u8> for now but it might
    // be a boolean actually
    FdbFlush(Vec<u8>),
    Pad(Vec<u8>),
    HelloTimer(u64),
    TcnTimer(u64),
    TopologyChangeTimer(u64),
    GcTimer(u64),
    MulticastMembershipInterval(u64),
    MulticastQuerierInterval(u64),
    MulticastQueryInterval(u64),
    MulticastQueryResponseInterval(u64),
    MulticastLastMemberInterval(u64),
    MulticastStartupQueryInterval(u64),
    ForwardDelay(u32),
    HelloTime(u32),
    MaxAge(u32),
    AgeingTime(u32),
    StpState(u32),
    MulticastHashElasticity(u32),
    MulticastHashMax(u32),
    MulticastLastMemberCount(u32),
    MulticastStartupQueryCount(u32),
    RootPathCost(u32),
    Priority(u16),
    VlanProtocol(u16),
    GroupFwdMask(u16),
    RootId((u16, [u8; 6])),
    BridgeId((u16, [u8; 6])),
    RootPort(u16),
    VlanDefaultPvid(u16),
    VlanFiltering(u8),
    TopologyChange(u8),
    TopologyChangeDetected(u8),
    MulticastRouter(u8),
    MulticastSnooping(u8),
    MulticastQueryUseIfaddr(u8),
    MulticastQuerier(u8),
    NfCallIpTables(u8),
    NfCallIp6Tables(u8),
    NfCallArpTables(u8),
    VlanStatsEnabled(u8),
    MulticastStatsEnabled(u8),
    MulticastIgmpVersion(u8),
    MulticastMldVersion(u8),
    VlanStatsPerHost(u8),
    MultiBoolOpt(u64),
    Other(DefaultNla),
}

impl Nla for InfoBridge {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::InfoBridge::*;
        match self {
            Unspec(bytes)
                | FdbFlush(bytes)
                | Pad(bytes)
                => bytes.len(),
            HelloTimer(_)
                | TcnTimer(_)
                | TopologyChangeTimer(_)
                | GcTimer(_)
                | MulticastMembershipInterval(_)
                | MulticastQuerierInterval(_)
                | MulticastQueryInterval(_)
                | MulticastQueryResponseInterval(_)
                | MulticastLastMemberInterval(_)
                | MulticastStartupQueryInterval(_)
                => 8,
            ForwardDelay(_)
                | HelloTime(_)
                | MaxAge(_)
                | AgeingTime(_)
                | StpState(_)
                | MulticastHashElasticity(_)
                | MulticastHashMax(_)
                | MulticastLastMemberCount(_)
                | MulticastStartupQueryCount(_)
                | RootPathCost(_)
                => 4,
            Priority(_)
                | VlanProtocol(_)
                | GroupFwdMask(_)
                | RootPort(_)
                | VlanDefaultPvid(_)
                => 2,

            RootId(_)
                | BridgeId(_)
                | MultiBoolOpt(_)
                => 8,

            GroupAddr(_) => 6,

            VlanFiltering(_)
                | TopologyChange(_)
                | TopologyChangeDetected(_)
                | MulticastRouter(_)
                | MulticastSnooping(_)
                | MulticastQueryUseIfaddr(_)
                | MulticastQuerier(_)
                | NfCallIpTables(_)
                | NfCallIp6Tables(_)
                | NfCallArpTables(_)
                | VlanStatsEnabled(_)
                | MulticastStatsEnabled(_)
                | MulticastIgmpVersion(_)
                | MulticastMldVersion(_)
                | VlanStatsPerHost(_)
                => 1,
            Other(nla)
                => nla.value_len(),
        }
    }

    #[rustfmt::skip]
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoBridge::*;
        match self {
            Unspec(ref bytes)
                | FdbFlush(ref bytes)
                | Pad(ref bytes)
                => buffer.copy_from_slice(bytes),

            HelloTimer(ref value)
                | TcnTimer(ref value)
                | TopologyChangeTimer(ref value)
                | GcTimer(ref value)
                | MulticastMembershipInterval(ref value)
                | MulticastQuerierInterval(ref value)
                | MulticastQueryInterval(ref value)
                | MulticastQueryResponseInterval(ref value)
                | MulticastLastMemberInterval(ref value)
                | MulticastStartupQueryInterval(ref value)
                | MultiBoolOpt(ref value)
                => NativeEndian::write_u64(buffer, *value),

            ForwardDelay(ref value)
                | HelloTime(ref value)
                | MaxAge(ref value)
                | AgeingTime(ref value)
                | StpState(ref value)
                | MulticastHashElasticity(ref value)
                | MulticastHashMax(ref value)
                | MulticastLastMemberCount(ref value)
                | MulticastStartupQueryCount(ref value)
                | RootPathCost(ref value)
                => NativeEndian::write_u32(buffer, *value),

            Priority(ref value)
                | GroupFwdMask(ref value)
                | RootPort(ref value)
                | VlanDefaultPvid(ref value)
                => NativeEndian::write_u16(buffer, *value),

            VlanProtocol(ref value)
                => BigEndian::write_u16(buffer, *value),

            RootId((ref priority, ref address))
                | BridgeId((ref priority, ref address))
                => {
                    NativeEndian::write_u16(buffer, *priority);
                    buffer[2..].copy_from_slice(&address[..]);
                }

            GroupAddr(ref value) => buffer.copy_from_slice(&value[..]),

            VlanFiltering(ref value)
                | TopologyChange(ref value)
                | TopologyChangeDetected(ref value)
                | MulticastRouter(ref value)
                | MulticastSnooping(ref value)
                | MulticastQueryUseIfaddr(ref value)
                | MulticastQuerier(ref value)
                | NfCallIpTables(ref value)
                | NfCallIp6Tables(ref value)
                | NfCallArpTables(ref value)
                | VlanStatsEnabled(ref value)
                | MulticastStatsEnabled(ref value)
                | MulticastIgmpVersion(ref value)
                | MulticastMldVersion(ref value)
                | VlanStatsPerHost(ref value)
                => buffer[0] = *value,

            Other(nla)
                => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::InfoBridge::*;
        match self {
            Unspec(_) => IFLA_BR_UNSPEC,
            GroupAddr(_) => IFLA_BR_GROUP_ADDR,
            FdbFlush(_) => IFLA_BR_FDB_FLUSH,
            Pad(_) => IFLA_BR_PAD,
            HelloTimer(_) => IFLA_BR_HELLO_TIMER,
            TcnTimer(_) => IFLA_BR_TCN_TIMER,
            TopologyChangeTimer(_) => IFLA_BR_TOPOLOGY_CHANGE_TIMER,
            GcTimer(_) => IFLA_BR_GC_TIMER,
            MulticastMembershipInterval(_) => IFLA_BR_MCAST_MEMBERSHIP_INTVL,
            MulticastQuerierInterval(_) => IFLA_BR_MCAST_QUERIER_INTVL,
            MulticastQueryInterval(_) => IFLA_BR_MCAST_QUERY_INTVL,
            MulticastQueryResponseInterval(_) => IFLA_BR_MCAST_QUERY_RESPONSE_INTVL,
            ForwardDelay(_) => IFLA_BR_FORWARD_DELAY,
            HelloTime(_) => IFLA_BR_HELLO_TIME,
            MaxAge(_) => IFLA_BR_MAX_AGE,
            AgeingTime(_) => IFLA_BR_AGEING_TIME,
            StpState(_) => IFLA_BR_STP_STATE,
            MulticastHashElasticity(_) => IFLA_BR_MCAST_HASH_ELASTICITY,
            MulticastHashMax(_) => IFLA_BR_MCAST_HASH_MAX,
            MulticastLastMemberCount(_) => IFLA_BR_MCAST_LAST_MEMBER_CNT,
            MulticastStartupQueryCount(_) => IFLA_BR_MCAST_STARTUP_QUERY_CNT,
            MulticastLastMemberInterval(_) => IFLA_BR_MCAST_LAST_MEMBER_INTVL,
            MulticastStartupQueryInterval(_) => IFLA_BR_MCAST_STARTUP_QUERY_INTVL,
            RootPathCost(_) => IFLA_BR_ROOT_PATH_COST,
            Priority(_) => IFLA_BR_PRIORITY,
            VlanProtocol(_) => IFLA_BR_VLAN_PROTOCOL,
            GroupFwdMask(_) => IFLA_BR_GROUP_FWD_MASK,
            RootId(_) => IFLA_BR_ROOT_ID,
            BridgeId(_) => IFLA_BR_BRIDGE_ID,
            RootPort(_) => IFLA_BR_ROOT_PORT,
            VlanDefaultPvid(_) => IFLA_BR_VLAN_DEFAULT_PVID,
            VlanFiltering(_) => IFLA_BR_VLAN_FILTERING,
            TopologyChange(_) => IFLA_BR_TOPOLOGY_CHANGE,
            TopologyChangeDetected(_) => IFLA_BR_TOPOLOGY_CHANGE_DETECTED,
            MulticastRouter(_) => IFLA_BR_MCAST_ROUTER,
            MulticastSnooping(_) => IFLA_BR_MCAST_SNOOPING,
            MulticastQueryUseIfaddr(_) => IFLA_BR_MCAST_QUERY_USE_IFADDR,
            MulticastQuerier(_) => IFLA_BR_MCAST_QUERIER,
            NfCallIpTables(_) => IFLA_BR_NF_CALL_IPTABLES,
            NfCallIp6Tables(_) => IFLA_BR_NF_CALL_IP6TABLES,
            NfCallArpTables(_) => IFLA_BR_NF_CALL_ARPTABLES,
            VlanStatsEnabled(_) => IFLA_BR_VLAN_STATS_ENABLED,
            MulticastStatsEnabled(_) => IFLA_BR_MCAST_STATS_ENABLED,
            MulticastIgmpVersion(_) => IFLA_BR_MCAST_IGMP_VERSION,
            MulticastMldVersion(_) => IFLA_BR_MCAST_MLD_VERSION,
            VlanStatsPerHost(_) => IFLA_BR_VLAN_STATS_PER_PORT,
            MultiBoolOpt(_) => IFLA_BR_MULTI_BOOLOPT,
            Other(nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for InfoBridge {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::InfoBridge::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_BR_UNSPEC => Unspec(payload.to_vec()),
            IFLA_BR_FDB_FLUSH => FdbFlush(payload.to_vec()),
            IFLA_BR_PAD => Pad(payload.to_vec()),
            IFLA_BR_HELLO_TIMER => {
                HelloTimer(parse_u64(payload).context("invalid IFLA_BR_HELLO_TIMER value")?)
            }
            IFLA_BR_TCN_TIMER => {
                TcnTimer(parse_u64(payload).context("invalid IFLA_BR_TCN_TIMER value")?)
            }
            IFLA_BR_TOPOLOGY_CHANGE_TIMER => TopologyChangeTimer(
                parse_u64(payload).context("invalid IFLA_BR_TOPOLOGY_CHANGE_TIMER value")?,
            ),
            IFLA_BR_GC_TIMER => {
                GcTimer(parse_u64(payload).context("invalid IFLA_BR_GC_TIMER value")?)
            }
            IFLA_BR_MCAST_LAST_MEMBER_INTVL => MulticastLastMemberInterval(
                parse_u64(payload).context("invalid IFLA_BR_MCAST_LAST_MEMBER_INTVL value")?,
            ),
            IFLA_BR_MCAST_MEMBERSHIP_INTVL => MulticastMembershipInterval(
                parse_u64(payload).context("invalid IFLA_BR_MCAST_MEMBERSHIP_INTVL value")?,
            ),
            IFLA_BR_MCAST_QUERIER_INTVL => MulticastQuerierInterval(
                parse_u64(payload).context("invalid IFLA_BR_MCAST_QUERIER_INTVL value")?,
            ),
            IFLA_BR_MCAST_QUERY_INTVL => MulticastQueryInterval(
                parse_u64(payload).context("invalid IFLA_BR_MCAST_QUERY_INTVL value")?,
            ),
            IFLA_BR_MCAST_QUERY_RESPONSE_INTVL => MulticastQueryResponseInterval(
                parse_u64(payload).context("invalid IFLA_BR_MCAST_QUERY_RESPONSE_INTVL value")?,
            ),
            IFLA_BR_MCAST_STARTUP_QUERY_INTVL => MulticastStartupQueryInterval(
                parse_u64(payload).context("invalid IFLA_BR_MCAST_STARTUP_QUERY_INTVL value")?,
            ),
            IFLA_BR_FORWARD_DELAY => {
                ForwardDelay(parse_u32(payload).context("invalid IFLA_BR_FORWARD_DELAY value")?)
            }
            IFLA_BR_HELLO_TIME => {
                HelloTime(parse_u32(payload).context("invalid IFLA_BR_HELLO_TIME value")?)
            }
            IFLA_BR_MAX_AGE => MaxAge(parse_u32(payload).context("invalid IFLA_BR_MAX_AGE value")?),
            IFLA_BR_AGEING_TIME => {
                AgeingTime(parse_u32(payload).context("invalid IFLA_BR_AGEING_TIME value")?)
            }
            IFLA_BR_STP_STATE => {
                StpState(parse_u32(payload).context("invalid IFLA_BR_STP_STATE value")?)
            }
            IFLA_BR_MCAST_HASH_ELASTICITY => MulticastHashElasticity(
                parse_u32(payload).context("invalid IFLA_BR_MCAST_HASH_ELASTICITY value")?,
            ),
            IFLA_BR_MCAST_HASH_MAX => MulticastHashMax(
                parse_u32(payload).context("invalid IFLA_BR_MCAST_HASH_MAX value")?,
            ),
            IFLA_BR_MCAST_LAST_MEMBER_CNT => MulticastLastMemberCount(
                parse_u32(payload).context("invalid IFLA_BR_MCAST_LAST_MEMBER_CNT value")?,
            ),
            IFLA_BR_MCAST_STARTUP_QUERY_CNT => MulticastStartupQueryCount(
                parse_u32(payload).context("invalid IFLA_BR_MCAST_STARTUP_QUERY_CNT value")?,
            ),
            IFLA_BR_ROOT_PATH_COST => {
                RootPathCost(parse_u32(payload).context("invalid IFLA_BR_ROOT_PATH_COST value")?)
            }
            IFLA_BR_PRIORITY => {
                Priority(parse_u16(payload).context("invalid IFLA_BR_PRIORITY value")?)
            }
            IFLA_BR_VLAN_PROTOCOL => {
                VlanProtocol(parse_u16_be(payload).context("invalid IFLA_BR_VLAN_PROTOCOL value")?)
            }
            IFLA_BR_GROUP_FWD_MASK => {
                GroupFwdMask(parse_u16(payload).context("invalid IFLA_BR_GROUP_FWD_MASK value")?)
            }
            IFLA_BR_ROOT_ID | IFLA_BR_BRIDGE_ID => {
                if payload.len() != 8 {
                    return Err("invalid IFLA_BR_ROOT_ID or IFLA_BR_BRIDGE_ID value".into());
                }

                let priority = NativeEndian::read_u16(&payload[..2]);
                let address = parse_mac(&payload[2..])
                    .context("invalid IFLA_BR_ROOT_ID or IFLA_BR_BRIDGE_ID value")?;

                match buf.kind() {
                    IFLA_BR_ROOT_ID => RootId((priority, address)),
                    IFLA_BR_BRIDGE_ID => BridgeId((priority, address)),
                    _ => unreachable!(),
                }
            }
            IFLA_BR_GROUP_ADDR => {
                GroupAddr(parse_mac(payload).context("invalid IFLA_BR_GROUP_ADDR value")?)
            }
            IFLA_BR_ROOT_PORT => {
                RootPort(parse_u16(payload).context("invalid IFLA_BR_ROOT_PORT value")?)
            }
            IFLA_BR_VLAN_DEFAULT_PVID => VlanDefaultPvid(
                parse_u16(payload).context("invalid IFLA_BR_VLAN_DEFAULT_PVID value")?,
            ),
            IFLA_BR_VLAN_FILTERING => {
                VlanFiltering(parse_u8(payload).context("invalid IFLA_BR_VLAN_FILTERING value")?)
            }
            IFLA_BR_TOPOLOGY_CHANGE => {
                TopologyChange(parse_u8(payload).context("invalid IFLA_BR_TOPOLOGY_CHANGE value")?)
            }
            IFLA_BR_TOPOLOGY_CHANGE_DETECTED => TopologyChangeDetected(
                parse_u8(payload).context("invalid IFLA_BR_TOPOLOGY_CHANGE_DETECTED value")?,
            ),
            IFLA_BR_MCAST_ROUTER => {
                MulticastRouter(parse_u8(payload).context("invalid IFLA_BR_MCAST_ROUTER value")?)
            }
            IFLA_BR_MCAST_SNOOPING => MulticastSnooping(
                parse_u8(payload).context("invalid IFLA_BR_MCAST_SNOOPING value")?,
            ),
            IFLA_BR_MCAST_QUERY_USE_IFADDR => MulticastQueryUseIfaddr(
                parse_u8(payload).context("invalid IFLA_BR_MCAST_QUERY_USE_IFADDR value")?,
            ),
            IFLA_BR_MCAST_QUERIER => {
                MulticastQuerier(parse_u8(payload).context("invalid IFLA_BR_MCAST_QUERIER value")?)
            }
            IFLA_BR_NF_CALL_IPTABLES => {
                NfCallIpTables(parse_u8(payload).context("invalid IFLA_BR_NF_CALL_IPTABLES value")?)
            }
            IFLA_BR_NF_CALL_IP6TABLES => NfCallIp6Tables(
                parse_u8(payload).context("invalid IFLA_BR_NF_CALL_IP6TABLES value")?,
            ),
            IFLA_BR_NF_CALL_ARPTABLES => NfCallArpTables(
                parse_u8(payload).context("invalid IFLA_BR_NF_CALL_ARPTABLES value")?,
            ),
            IFLA_BR_VLAN_STATS_ENABLED => VlanStatsEnabled(
                parse_u8(payload).context("invalid IFLA_BR_VLAN_STATS_ENABLED value")?,
            ),
            IFLA_BR_MCAST_STATS_ENABLED => MulticastStatsEnabled(
                parse_u8(payload).context("invalid IFLA_BR_MCAST_STATS_ENABLED value")?,
            ),
            IFLA_BR_MCAST_IGMP_VERSION => MulticastIgmpVersion(
                parse_u8(payload).context("invalid IFLA_BR_MCAST_IGMP_VERSION value")?,
            ),
            IFLA_BR_MCAST_MLD_VERSION => MulticastMldVersion(
                parse_u8(payload).context("invalid IFLA_BR_MCAST_MLD_VERSION value")?,
            ),
            IFLA_BR_VLAN_STATS_PER_PORT => VlanStatsPerHost(
                parse_u8(payload).context("invalid IFLA_BR_VLAN_STATS_PER_PORT value")?,
            ),
            IFLA_BR_MULTI_BOOLOPT => {
                MultiBoolOpt(parse_u64(payload).context("invalid IFLA_BR_MULTI_BOOLOPT value")?)
            }
            _ => Other(
                DefaultNla::parse(buf)
                    .context("invalid link info bridge NLA value (unknown type)")?,
            ),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoIpoib {
    Unspec(Vec<u8>),
    Pkey(u16),
    Mode(u16),
    UmCast(u16),
    Other(DefaultNla),
}

impl Nla for InfoIpoib {
    fn value_len(&self) -> usize {
        use self::InfoIpoib::*;
        match self {
            Unspec(bytes) => bytes.len(),
            Pkey(_) | Mode(_) | UmCast(_) => 2,
            Other(nla) => nla.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoIpoib::*;
        match self {
            Unspec(bytes) => buffer.copy_from_slice(bytes.as_slice()),
            Pkey(value) => NativeEndian::write_u16(buffer, *value),
            Mode(value) => NativeEndian::write_u16(buffer, *value),
            UmCast(value) => NativeEndian::write_u16(buffer, *value),
            Other(nla) => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::InfoIpoib::*;
        match self {
            Unspec(_) => IFLA_IPOIB_UNSPEC,
            Pkey(_) => IFLA_IPOIB_PKEY,
            Mode(_) => IFLA_IPOIB_MODE,
            UmCast(_) => IFLA_IPOIB_UMCAST,
            Other(nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for InfoIpoib {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::InfoIpoib::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_IPOIB_UNSPEC => Unspec(payload.to_vec()),
            IFLA_IPOIB_PKEY => Pkey(parse_u16(payload).context("invalid IFLA_IPOIB_PKEY value")?),
            IFLA_IPOIB_MODE => Mode(parse_u16(payload).context("invalid IFLA_IPOIB_MODE value")?),
            IFLA_IPOIB_UMCAST => {
                UmCast(parse_u16(payload).context("invalid IFLA_IPOIB_UMCAST value")?)
            }
            kind => Other(DefaultNla::parse(buf).context(format!("unknown NLA type {}", kind))?),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum VethInfo {
    Unspec(Vec<u8>),
    Peer(LinkMessage),
    Other(DefaultNla),
}

impl Nla for VethInfo {
    fn value_len(&self) -> usize {
        use self::VethInfo::*;
        match *self {
            Unspec(ref bytes) => bytes.len(),
            Peer(ref message) => message.buffer_len(),
            Other(ref attr) => attr.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::VethInfo::*;
        match *self {
            Unspec(ref bytes) => buffer.copy_from_slice(bytes.as_slice()),
            Peer(ref message) => message.emit(buffer),
            Other(ref attr) => attr.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::VethInfo::*;
        match *self {
            Unspec(_) => VETH_INFO_UNSPEC,
            Peer(_) => VETH_INFO_PEER,
            Other(ref attr) => attr.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for VethInfo {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::VethInfo::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            VETH_INFO_UNSPEC => Unspec(payload.to_vec()),
            VETH_INFO_PEER => {
                let err = "failed to parse veth link info";
                let buffer = LinkMessageBuffer::new_checked(&payload).context(err)?;
                Peer(LinkMessage::parse(&buffer).context(err)?)
            }
            kind => Other(DefaultNla::parse(buf).context(format!("unknown NLA type {}", kind))?),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoIpVlan {
    Unspec(Vec<u8>),
    Mode(u16),
    Flags(u16),
    Other(DefaultNla),
}

impl Nla for InfoIpVlan {
    fn value_len(&self) -> usize {
        use self::InfoIpVlan::*;
        match self {
            Unspec(bytes) => bytes.len(),
            Mode(_) | Flags(_) => 2,
            Other(nla) => nla.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoIpVlan::*;
        match self {
            Unspec(bytes) => buffer.copy_from_slice(bytes.as_slice()),
            Mode(value) => NativeEndian::write_u16(buffer, *value),
            Flags(value) => NativeEndian::write_u16(buffer, *value),
            Other(nla) => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::InfoIpVlan::*;
        match self {
            Unspec(_) => IFLA_IPVLAN_UNSPEC,
            Mode(_) => IFLA_IPVLAN_MODE,
            Flags(_) => IFLA_IPVLAN_FLAGS,
            Other(nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for InfoIpVlan {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::InfoIpVlan::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_IPVLAN_UNSPEC => Unspec(payload.to_vec()),
            IFLA_IPVLAN_MODE => Mode(parse_u16(payload).context("invalid IFLA_IPVLAN_MODE value")?),
            IFLA_IPVLAN_FLAGS => {
                Flags(parse_u16(payload).context("invalid IFLA_IPVLAN_FLAGS value")?)
            }
            kind => Other(DefaultNla::parse(buf).context(format!("unknown NLA type {}", kind))?),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoVrf {
    TableId(u32),
    Other(DefaultNla),
}

impl Nla for InfoVrf {
    fn value_len(&self) -> usize {
        use self::InfoVrf::*;
        match self {
            TableId(_) => 4,
            Other(nla) => nla.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoVrf::*;
        match self {
            TableId(value) => NativeEndian::write_u32(buffer, *value),
            Other(nla) => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::InfoVrf::*;
        match self {
            TableId(_) => IFLA_VRF_TABLE,
            Other(nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for InfoVrf {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::InfoVrf::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_VRF_TABLE => TableId(parse_u32(payload).context("invalid IFLA_VRF_TABLE value")?),
            kind => Other(DefaultNla::parse(buf).context(format!("unknown NLA type {}", kind))?),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoMacVlan {
    Unspec(Vec<u8>),
    Mode(u32),
    Flags(u16),
    MacAddrMode(u32),
    MacAddr([u8; 6]),
    MacAddrData(Vec<InfoMacVlan>),
    MacAddrCount(u32),
    Other(DefaultNla),
}

impl Nla for InfoMacVlan {
    fn value_len(&self) -> usize {
        use self::InfoMacVlan::*;
        match self {
            Unspec(bytes) => bytes.len(),
            Mode(_) => 4,
            Flags(_) => 2,
            MacAddrMode(_) => 4,
            MacAddr(_) => 6,
            MacAddrData(ref nlas) => nlas.as_slice().buffer_len(),
            MacAddrCount(_) => 4,
            Other(nla) => nla.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoMacVlan::*;
        match self {
            Unspec(bytes) => buffer.copy_from_slice(bytes.as_slice()),
            Mode(value) => NativeEndian::write_u32(buffer, *value),
            Flags(value) => NativeEndian::write_u16(buffer, *value),
            MacAddrMode(value) => NativeEndian::write_u32(buffer, *value),
            MacAddr(bytes) => buffer.copy_from_slice(bytes),
            MacAddrData(ref nlas) => nlas.as_slice().emit(buffer),
            MacAddrCount(value) => NativeEndian::write_u32(buffer, *value),
            Other(nla) => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::InfoMacVlan::*;
        match self {
            Unspec(_) => IFLA_MACVLAN_UNSPEC,
            Mode(_) => IFLA_MACVLAN_MODE,
            Flags(_) => IFLA_MACVLAN_FLAGS,
            MacAddrMode(_) => IFLA_MACVLAN_MACADDR_MODE,
            MacAddr(_) => IFLA_MACVLAN_MACADDR,
            MacAddrData(_) => IFLA_MACVLAN_MACADDR_DATA,
            MacAddrCount(_) => IFLA_MACVLAN_MACADDR_COUNT,
            Other(nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for InfoMacVlan {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::InfoMacVlan::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_MACVLAN_UNSPEC => Unspec(payload.to_vec()),
            IFLA_MACVLAN_MODE => {
                Mode(parse_u32(payload).context("invalid IFLA_MACVLAN_MODE value")?)
            }
            IFLA_MACVLAN_FLAGS => {
                Flags(parse_u16(payload).context("invalid IFLA_MACVLAN_FLAGS value")?)
            }
            IFLA_MACVLAN_MACADDR_MODE => {
                MacAddrMode(parse_u32(payload).context("invalid IFLA_MACVLAN_MACADDR_MODE value")?)
            }
            IFLA_MACVLAN_MACADDR => {
                MacAddr(parse_mac(payload).context("invalid IFLA_MACVLAN_MACADDR value")?)
            }
            IFLA_MACVLAN_MACADDR_DATA => {
                let mut mac_data = Vec::new();
                let err = "failed to parse IFLA_MACVLAN_MACADDR_DATA";
                for nla in NlasIterator::new(payload) {
                    let nla = &nla.context(err)?;
                    let parsed = InfoMacVlan::parse(nla).context(err)?;
                    mac_data.push(parsed);
                }
                MacAddrData(mac_data)
            }
            IFLA_MACVLAN_MACADDR_COUNT => MacAddrCount(
                parse_u32(payload).context("invalid IFLA_MACVLAN_MACADDR_COUNT value")?,
            ),
            kind => Other(DefaultNla::parse(buf).context(format!("unknown NLA type {}", kind))?),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoMacVtap {
    Unspec(Vec<u8>),
    Mode(u32),
    Flags(u16),
    MacAddrMode(u32),
    MacAddr([u8; 6]),
    MacAddrData(Vec<InfoMacVtap>),
    MacAddrCount(u32),
    Other(DefaultNla),
}

impl Nla for InfoMacVtap {
    fn value_len(&self) -> usize {
        use self::InfoMacVtap::*;
        match self {
            Unspec(bytes) => bytes.len(),
            Mode(_) => 4,
            Flags(_) => 2,
            MacAddrMode(_) => 4,
            MacAddr(_) => 6,
            MacAddrData(ref nlas) => nlas.as_slice().buffer_len(),
            MacAddrCount(_) => 4,
            Other(nla) => nla.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoMacVtap::*;
        match self {
            Unspec(bytes) => buffer.copy_from_slice(bytes.as_slice()),
            Mode(value) => NativeEndian::write_u32(buffer, *value),
            Flags(value) => NativeEndian::write_u16(buffer, *value),
            MacAddrMode(value) => NativeEndian::write_u32(buffer, *value),
            MacAddr(bytes) => buffer.copy_from_slice(bytes),
            MacAddrData(ref nlas) => nlas.as_slice().emit(buffer),
            MacAddrCount(value) => NativeEndian::write_u32(buffer, *value),
            Other(nla) => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::InfoMacVtap::*;
        match self {
            Unspec(_) => IFLA_MACVLAN_UNSPEC,
            Mode(_) => IFLA_MACVLAN_MODE,
            Flags(_) => IFLA_MACVLAN_FLAGS,
            MacAddrMode(_) => IFLA_MACVLAN_MACADDR_MODE,
            MacAddr(_) => IFLA_MACVLAN_MACADDR,
            MacAddrData(_) => IFLA_MACVLAN_MACADDR_DATA,
            MacAddrCount(_) => IFLA_MACVLAN_MACADDR_COUNT,
            Other(nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for InfoMacVtap {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::InfoMacVtap::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_MACVLAN_UNSPEC => Unspec(payload.to_vec()),
            IFLA_MACVLAN_MODE => {
                Mode(parse_u32(payload).context("invalid IFLA_MACVLAN_MODE value")?)
            }
            IFLA_MACVLAN_FLAGS => {
                Flags(parse_u16(payload).context("invalid IFLA_MACVLAN_FLAGS value")?)
            }
            IFLA_MACVLAN_MACADDR_MODE => {
                MacAddrMode(parse_u32(payload).context("invalid IFLA_MACVLAN_MACADDR_MODE value")?)
            }
            IFLA_MACVLAN_MACADDR => {
                MacAddr(parse_mac(payload).context("invalid IFLA_MACVLAN_MACADDR value")?)
            }
            IFLA_MACVLAN_MACADDR_DATA => {
                let mut mac_data = Vec::new();
                let err = "failed to parse IFLA_MACVLAN_MACADDR_DATA";
                for nla in NlasIterator::new(payload) {
                    let nla = &nla.context(err)?;
                    let parsed = InfoMacVtap::parse(nla).context(err)?;
                    mac_data.push(parsed);
                }
                MacAddrData(mac_data)
            }
            IFLA_MACVLAN_MACADDR_COUNT => MacAddrCount(
                parse_u32(payload).context("invalid IFLA_MACVLAN_MACADDR_COUNT value")?,
            ),
            kind => Other(DefaultNla::parse(buf).context(format!("unknown NLA type {}", kind))?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{nlas::link::Nla, traits::Emitable, LinkHeader, LinkMessage};

    #[rustfmt::skip]
    static BRIDGE: [u8; 424] = [
        0x0b, 0x00, // L = 11
        0x01, 0x00, // T = 1 (IFLA_INFO_KIND)
        0x62, 0x72, 0x69, 0x64, 0x67, 0x65, 0x00, // V = "bridge"
        0x00, // padding

        0x9c, 0x01, // L = 412
        0x02, 0x00, // T = 2 (IFLA_INFO_DATA)

            0x0c, 0x00, // L = 12
            0x10, 0x00, // T = 16 (IFLA_BR_HELLO_TIMER)
            0x23, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // V = 35

            0x0c, 0x00, // L = 12
            0x11, 0x00, // T = 17 (IFLA_BR_TCN_TIMER)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // V = 0

            0x0c, 0x00, // L = 12
            0x12, 0x00, // T = 18 (IFLA_BR_TOPOLOGY_CHANGE_TIMER)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // V = 0

            0x0c, 0x00, //  L = 12
            0x13, 0x00, // T = 19 (IFLA_BR_GC_TIMER)
            0xb5, 0x37, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // V = 14261 (0x37b5)

            0x08, 0x00, // L = 8
            0x01, 0x00, // T = 1 (IFLA_BR_FORWARD_DELAY)
            0xc7, 0x00, 0x00, 0x00, // V = 199

            0x08, 0x00, // L = 8
            0x02, 0x00, // T = 2 (IFLA_BR_HELLO_TIME)
            0xc7, 0x00, 0x00, 0x00, // V = 199

            0x08, 0x00, // L = 8
            0x03, 0x00, // T = 3 (IFLA_BR_MAX_AGE)
            0xcf, 0x07, 0x00, 0x00, // V = 1999 (0x07cf)

            0x08, 0x00, // L = 8
            0x04, 0x00, // T = 4 (IFLA_BR_AGEING_TIME)
            0x2f, 0x75, 0x00, 0x00, // V = 29999 (0x752f)

            0x08, 0x00, // L = 8
            0x05, 0x00, // T = 5 (IFLA_BR_STP_STATE)
            0x01, 0x00, 0x00, 0x00, // V = 1

            0x06, 0x00, // L = 6
            0x06, 0x00, // T = 6 (IFLA_BR_PRIORITY)
            0x00, 0x80, // V =  32768 (0x8000)
            0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x07, 0x00, // T = 7 (IFLA_BR_VLAN_FILTERING)
            0x00, // V = 0
            0x00, 0x00, 0x00, // Padding

            0x06, 0x00, // L = 6
            0x09, 0x00, // T = 9 (IFLA_BR_GROUP_FWD_MASK)
            0x00, 0x00, // V = 0
            0x00, 0x00, // Padding

            0x0c, 0x00, // L = 12
            0x0b, 0x00, // T = 11 (IFLA_BR_BRIDGE_ID)
            0x80, 0x00, // V (priority) = 128 (0x80)
            0x52, 0x54, 0x00, 0xd7, 0x19, 0x3e, // V (address) = 52:54:00:d7:19:3e

            0x0c, 0x00, // L = 12
            0x0a, 0x00, // T = 10 (IFLA_BR_ROOT_ID)
            0x80, 0x00, // V (priority) = 128 (0x80)
            0x52, 0x54, 0x00, 0xd7, 0x19, 0x3e, // V (address) = 52:54:00:d7:19:3e

            0x06, 0x00, // L = 6
            0x0c, 0x00, // T = 12 (IFLA_BR_ROOT_PORT)
            0x00, 0x00, // V = 0
            0x00, 0x00, // Padding

            0x08, 0x00, // L = 8
            0x0d, 0x00, // T = 13 (IFLA_BR_ROOT_PATH_COST)
            0x00, 0x00, 0x00, 0x00, // V = 0

            0x05, 0x00, // L = 5
            0x0e, 0x00, // T = 14 (IFLA_BR_TOPOLOGY_CHANGE)
            0x00, // V = 0
            0x00, 0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x0f, 0x00, // T = 15 (IFLA_BR_TOPOLOGY_CHANGE_DETECTED)
            0x00, // V = 0
            0x00, 0x00, 0x00, // Padding

            0x0a, 0x00, // L = 10
            0x14, 0x00, // T = 20 (IFLA_BR_GROUP_ADDR)
            0x01, 0x80, 0xc2, 0x00, 0x00, 0x00, // V = 01:80:c2:00:00:00
            0x00, 0x00, // Padding

            0x06, 0x00, // L = 6
            0x08, 0x00, // T = 8 (IFLA_BR_VLAN_PROTOCOL)
            0x81, 0x00, // V = 33024 (big-endian)
            0x00, 0x00, // Padding

            0x06, 0x00, // L = 6
            0x27, 0x00, // T = 39 (IFLA_BR_VLAN_DEFAULT_PVID)
            0x01, 0x00, // V = 1
            0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x29, 0x00, // T = 41 (IFLA_BR_VLAN_STATS_ENABLED)
            0x00, // V = 0
            0x00, 0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x16, 0x00, // T = 22 (IFLA_BR_MCAST_ROUTER)
            0x01, // V = 1
            0x00, 0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x17, 0x00, // T = 23 (IFLA_BR_MCAST_SNOOPING)
            0x01, // V = 1
            0x00, 0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x18, 0x00, // T = 24 (IFLA_BR_MCAST_QUERY_USE_IFADDR)
            0x00, // V = 0
            0x00, 0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x19, 0x00, // T = 25 (IFLA_BR_MCAST_QUERIER)
            0x00, // V = 0
            0x00, 0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x2a, 0x00, // T = 42 (IFLA_BR_MCAST_STATS_ENABLED)
            0x00, // V = 0
            0x00, 0x00, 0x00, // Padding

            0x08, 0x00, // L = 8
            0x1a, 0x00, // T = 26 (IFLA_BR_MCAST_HASH_ELASTICITY)
            0x04, 0x00, 0x00, 0x00, // V = 4

            0x08, 0x00, // L = 8
            0x1b, 0x00, // T = 27 (IFLA_BR_MCAST_HASH_MAX)
            0x00, 0x02, 0x00, 0x00, // V = 512 (0x0200)

            0x08, 0x00, // L = 8
            0x1c, 0x00, // T = 28 (IFLA_BR_MCAST_LAST_MEMBER_CNT)
            0x02, 0x00, 0x00, 0x00, // V = 2

            0x08, 0x00, // L = 8
            0x1d, 0x00, // T = 29 (IFLA_BR_MCAST_STARTUP_QUERY_CNT)
            0x02, 0x00, 0x00, 0x00, // V = 2

            0x05, 0x00, // L = 5
            0x2b, 0x00, // T = 43 (IFLA_BR_MCAST_IGMP_VERSION)
            0x02, // V = 2
            0x00, 0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x2c, 0x00, // T = 44 (IFLA_BR_MCAST_MLD_VERSION)
            0x01, // V = 1
            0x00, 0x00, 0x00, // Padding

            0x0c, 0x00, // L = 12
            0x1e, 0x00, // T = 30 (IFLA_BR_MCAST_LAST_MEMBER_INTVL)
            0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // V = 99

            0x0c, 0x00, // L = 12
            0x1f, 0x00, // T = 31 (IFLA_BR_MCAST_MEMBERSHIP_INTVL)
            0x8f, 0x65, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // V = 25999 (0x658f)

            0x0c, 0x00, // L = 12
            0x20, 0x00, // T = 32 (IFLA_BR_MCAST_QUERIER_INTVL)
            0x9b, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // V = 25499 (0x639b)

            0x0c, 0x00, // L = 12
            0x21, 0x00, // T = 33 (IFLA_BR_MCAST_QUERY_INTVL)
            0xd3, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // V = 12499 (0x30d3)

            0x0c, 0x00, // L = 12
            0x22, 0x00, // T = 34 (IFLA_BR_MCAST_QUERY_RESPONSE_INTVL)
            0xe7, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // V = 999 (0x03e7)

            0x0c, 0x00, // L = 12
            0x23, 0x00, // T = 35 (IFLA_BR_MCAST_STARTUP_QUERY_INTVL)
            0x34, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // V = 3124 (0x0c34)

            0x05, 0x00, // L = 5
            0x24, 0x00, // T = 36 (IFLA_BR_NF_CALL_IPTABLES)
            0x00, // V = 0
            0x00, 0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x25, 0x00, // T = 37 (IFLA_BR_NF_CALL_IP6TABLES)
            0x00, // V = 0
            0x00, 0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x26, 0x00, // T = 38 (IFLA_BR_NF_CALL_ARPTABLES)
            0x00, // V = 0
            0x00, 0x00, 0x00, // Padding

            0x05, 0x00, // L = 5
            0x2d, 0x00, // T = 45 (IFLA_BR_VLAN_STATS_PER_PORT)
            0x01, // V = 1
            0x00, 0x00, 0x00, // Padding

            0x0c, 0x00, // L = 12
            0x2e, 0x00, // T = 46 (IFLA_BR_MULTI_BOOLOPT)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,  // V = 0

    ];

    lazy_static! {
        static ref BRIDGE_INFO: Vec<InfoBridge> = vec![
            InfoBridge::HelloTimer(35),
            InfoBridge::TcnTimer(0),
            InfoBridge::TopologyChangeTimer(0),
            InfoBridge::GcTimer(14261),
            InfoBridge::ForwardDelay(199),
            InfoBridge::HelloTime(199),
            InfoBridge::MaxAge(1999),
            InfoBridge::AgeingTime(29999),
            InfoBridge::StpState(1),
            InfoBridge::Priority(0x8000),
            InfoBridge::VlanFiltering(0),
            InfoBridge::GroupFwdMask(0),
            InfoBridge::BridgeId((128, [0x52, 0x54, 0x00, 0xd7, 0x19, 0x3e])),
            InfoBridge::RootId((128, [0x52, 0x54, 0x00, 0xd7, 0x19, 0x3e])),
            InfoBridge::RootPort(0),
            InfoBridge::RootPathCost(0),
            InfoBridge::TopologyChange(0),
            InfoBridge::TopologyChangeDetected(0),
            InfoBridge::GroupAddr([0x01, 0x80, 0xc2, 0x00, 0x00, 0x00]),
            InfoBridge::VlanProtocol(33024),
            InfoBridge::VlanDefaultPvid(1),
            InfoBridge::VlanStatsEnabled(0),
            InfoBridge::MulticastRouter(1),
            InfoBridge::MulticastSnooping(1),
            InfoBridge::MulticastQueryUseIfaddr(0),
            InfoBridge::MulticastQuerier(0),
            InfoBridge::MulticastStatsEnabled(0),
            InfoBridge::MulticastHashElasticity(4),
            InfoBridge::MulticastHashMax(512),
            InfoBridge::MulticastLastMemberCount(2),
            InfoBridge::MulticastStartupQueryCount(2),
            InfoBridge::MulticastIgmpVersion(2),
            InfoBridge::MulticastMldVersion(1),
            InfoBridge::MulticastLastMemberInterval(99),
            InfoBridge::MulticastMembershipInterval(25999),
            InfoBridge::MulticastQuerierInterval(25499),
            InfoBridge::MulticastQueryInterval(12499),
            InfoBridge::MulticastQueryResponseInterval(999),
            InfoBridge::MulticastStartupQueryInterval(3124),
            InfoBridge::NfCallIpTables(0),
            InfoBridge::NfCallIp6Tables(0),
            InfoBridge::NfCallArpTables(0),
            InfoBridge::VlanStatsPerHost(1),
            InfoBridge::MultiBoolOpt(0),
        ];
    }

    #[test]
    fn parse_info_kind() {
        let info_kind_nla = NlaBuffer::new_checked(&BRIDGE[..12]).unwrap();
        let parsed = InfoKind::parse(&info_kind_nla).unwrap();
        assert_eq!(parsed, InfoKind::Bridge);
    }

    #[test]
    fn parse_info_bridge() {
        let nlas = NlasIterator::new(&BRIDGE[16..]);
        for nla in nlas.map(|nla| nla.unwrap()) {
            InfoBridge::parse(&nla).unwrap();
        }
    }

    #[rustfmt::skip]
    #[test]
    fn parse_veth_info() {
        let data = vec![
            0x08, 0x00, // length = 8
            0x01, 0x00, // type = 1 = IFLA_INFO_KIND
            0x76, 0x65, 0x74, 0x68, // VETH

            0x30, 0x00, // length = 48
            0x02, 0x00, // type = IFLA_INFO_DATA

                0x2c, 0x00, // length = 44
                0x01, 0x00, // type = VETH_INFO_PEER
                // The data a NEWLINK message
                0x00, // interface family
                0x00, // padding
                0x00, 0x00, // link layer type
                0x00, 0x00, 0x00, 0x00, // link index
                0x00, 0x00, 0x00, 0x00, // flags
                0x00, 0x00, 0x00, 0x00, // flags change mask
                    // NLA
                    0x10, 0x00, // length = 16
                    0x03, 0x00, // type = IFLA_IFNAME
                    0x76, 0x65, 0x74, 0x68, 0x63, 0x30, 0x65, 0x36, 0x30, 0x64, 0x36, 0x00,
                    // NLA
                    0x08, 0x00, // length = 8
                    0x0d, 0x00, // type = IFLA_TXQLEN
                    0x00, 0x00, 0x00, 0x00,
        ];
        let nla = NlaBuffer::new_checked(&data[..]).unwrap();
        let parsed = VecInfo::parse(&nla).unwrap().0;
        let expected = vec![
            Info::Kind(InfoKind::Veth),
            Info::Data(InfoData::Veth(VethInfo::Peer(LinkMessage {
                header: LinkHeader {
                    interface_family: 0,
                    index: 0,
                    link_layer_type: ARPHRD_NETROM,
                    flags: 0,
                    change_mask: 0,
                },
                nlas: vec![
                    Nla::IfName("vethc0e60d6".to_string()),
                    Nla::TxQueueLen(0),
                ],
            }))),
        ];
        assert_eq!(expected, parsed);
    }

    #[rustfmt::skip]
    static IPVLAN: [u8; 32] = [
        0x0b, 0x00, // length = 11
        0x01, 0x00, // type = 1 = IFLA_INFO_KIND
        0x69, 0x70, 0x76, 0x6c, 0x61, 0x6e, 0x00, // V = "ipvlan\0"
        0x00, // padding

        0x14, 0x00, // length = 20
        0x02, 0x00, // type = 2 = IFLA_INFO_DATA
            0x06, 0x00, // length = 6
            0x01, 0x00, // type = 1 = IFLA_IPVLAN_MODE
            0x01, 0x00, // l3
            0x00, 0x00, // padding

            0x06, 0x00, // length = 6
            0x02, 0x00, // type = 2 = IFLA_IPVLAN_FLAGS
            0x02, 0x00, // vepa flag
            0x00, 0x00, // padding
    ];

    lazy_static! {
        static ref IPVLAN_INFO: Vec<InfoIpVlan> = vec![
            InfoIpVlan::Mode(1), // L3
            InfoIpVlan::Flags(2), // vepa flag
        ];
    }

    #[test]
    fn parse_info_ipvlan() {
        let nla = NlaBuffer::new_checked(&IPVLAN[..]).unwrap();
        let parsed = VecInfo::parse(&nla).unwrap().0;
        let expected = vec![
            Info::Kind(InfoKind::IpVlan),
            Info::Data(InfoData::IpVlan(IPVLAN_INFO.clone())),
        ];
        assert_eq!(expected, parsed);
    }

    #[test]
    fn emit_info_ipvlan() {
        let nlas = vec![
            Info::Kind(InfoKind::IpVlan),
            Info::Data(InfoData::IpVlan(IPVLAN_INFO.clone())),
        ];

        assert_eq!(nlas.as_slice().buffer_len(), 32);

        let mut vec = vec![0xff; 32];
        nlas.as_slice().emit(&mut vec);
        assert_eq!(&vec[..], &IPVLAN[..]);
    }

    #[rustfmt::skip]
    static MACVLAN: [u8; 24] = [
        0x0c, 0x00, // length = 12
        0x01, 0x00, // type = 1 = IFLA_INFO_KIND
        0x6d, 0x61, 0x63, 0x76, 0x6c, 0x61, 0x6e, 0x00, // V = "macvlan\0"
        0x0c, 0x00, // length = 12
        0x02, 0x00, // type = 2 = IFLA_INFO_DATA
            0x08, 0x00, // length = 8
            0x01, 0x00, // type = IFLA_MACVLAN_MODE
            0x04, 0x00, 0x00, 0x00, // V = 4 = bridge
    ];

    lazy_static! {
        static ref MACVLAN_INFO: Vec<InfoMacVlan> = vec![
            InfoMacVlan::Mode(4), // bridge
        ];
    }

    #[test]
    fn parse_info_macvlan() {
        let nla = NlaBuffer::new_checked(&MACVLAN[..]).unwrap();
        let parsed = VecInfo::parse(&nla).unwrap().0;
        let expected = vec![
            Info::Kind(InfoKind::MacVlan),
            Info::Data(InfoData::MacVlan(MACVLAN_INFO.clone())),
        ];
        assert_eq!(expected, parsed);
    }

    #[test]
    fn emit_info_macvlan() {
        let nlas = vec![
            Info::Kind(InfoKind::MacVlan),
            Info::Data(InfoData::MacVlan(MACVLAN_INFO.clone())),
        ];

        assert_eq!(nlas.as_slice().buffer_len(), 24);

        let mut vec = vec![0xff; 24];
        nlas.as_slice().emit(&mut vec);
        assert_eq!(&vec[..], &MACVLAN[..]);
    }

    #[rustfmt::skip]
    static MACVLAN_SOURCE_SET: [u8; 84] = [
        0x0c, 0x00, // length = 12
        0x01, 0x00, // type = 1 = IFLA_INFO_KIND
        0x6d, 0x61, 0x63, 0x76, 0x6c, 0x61, 0x6e, 0x00, // V = "macvlan\0"
        0x48, 0x00, // length = 72
        0x02, 0x00, // type = 2 = IFLA_INFO_DATA
            0x08, 0x00, // length = 8
            0x03, 0x00, // type = 3 = IFLA_MACVLAN_MACADDR_MODE
            0x03, 0x00, 0x00, 0x00, // V = 3 = set

            0x34, 0x00, // length = 52
            0x05, 0x00, // type = 5 = IFLA_MACVLAN_MACADDR_DATA
                0x0a, 0x00, // length = 10
                0x04, 0x00, // type = 4 = IFLA_MACVLAN_MACADDR
                0x22, 0xf5, 0x54, 0x09, 0x88, 0xd7, // V = mac address
                0x00, 0x00, // padding

                0x0a, 0x00, // length = 10
                0x04, 0x00, // type = 4 = IFLA_MACVLAN_MACADDR
                0x22, 0xf5, 0x54, 0x09, 0x99, 0x32, // V = mac address
                0x00, 0x00, // padding

                0x0a, 0x00, // length = 10
                0x04, 0x00, // type = 4 = IFLA_MACVLAN_MACADDR
                0x22, 0xf5, 0x54, 0x09, 0x87, 0x45, // V = mac address
                0x00, 0x00, // padding

                0x0a, 0x00, // length = 10
                0x04, 0x00, // type = 4 = IFLA_MACVLAN_MACADDR
                0x22, 0xf5, 0x54, 0x09, 0x11, 0x45, // V = mac address
                0x00, 0x00, // padding
            0x08, 0x00, // length = 8
            0x01, 0x00, // Type = 1 = IFLA_MACVLAN_MODE
            0x10, 0x00, 0x00, 0x00, // V = 16 = source
    ];

    lazy_static! {
        static ref MACVLAN_SOURCE_SET_INFO: Vec<InfoMacVlan> = vec![
            InfoMacVlan::MacAddrMode(3), // set
            InfoMacVlan::MacAddrData(vec![
                                 InfoMacVlan::MacAddr([0x22, 0xf5, 0x54, 0x09, 0x88, 0xd7,]),
                                 InfoMacVlan::MacAddr([0x22, 0xf5, 0x54, 0x09, 0x99, 0x32,]),
                                 InfoMacVlan::MacAddr([0x22, 0xf5, 0x54, 0x09, 0x87, 0x45,]),
                                 InfoMacVlan::MacAddr([0x22, 0xf5, 0x54, 0x09, 0x11, 0x45,]),
            ]),
            InfoMacVlan::Mode(16), // source
        ];
    }

    #[test]
    fn parse_info_macvlan_source_set() {
        let nla = NlaBuffer::new_checked(&MACVLAN_SOURCE_SET[..]).unwrap();
        let parsed = VecInfo::parse(&nla).unwrap().0;
        let expected = vec![
            Info::Kind(InfoKind::MacVlan),
            Info::Data(InfoData::MacVlan(MACVLAN_SOURCE_SET_INFO.clone())),
        ];
        assert_eq!(expected, parsed);
    }

    #[test]
    fn emit_info_macvlan_source_set() {
        let nlas = vec![
            Info::Kind(InfoKind::MacVlan),
            Info::Data(InfoData::MacVlan(MACVLAN_SOURCE_SET_INFO.clone())),
        ];

        assert_eq!(nlas.as_slice().buffer_len(), 84);

        let mut vec = vec![0xff; 84];
        nlas.as_slice().emit(&mut vec);
        assert_eq!(&vec[..], &MACVLAN_SOURCE_SET[..]);
    }

    #[test]
    fn parse() {
        let nla = NlaBuffer::new_checked(&BRIDGE[..]).unwrap();
        let parsed = VecInfo::parse(&nla).unwrap().0;
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], Info::Kind(InfoKind::Bridge));
        if let Info::Data(InfoData::Bridge(nlas)) = parsed[1].clone() {
            assert_eq!(nlas.len(), BRIDGE_INFO.len());
            for (expected, parsed) in BRIDGE_INFO.iter().zip(nlas) {
                assert_eq!(*expected, parsed);
            }
        } else {
            panic!(
                "expected  Info::Data(InfoData::Bridge(_) got {:?}",
                parsed[1]
            )
        }
    }

    #[test]
    fn emit() {
        let nlas = vec![
            Info::Kind(InfoKind::Bridge),
            Info::Data(InfoData::Bridge(BRIDGE_INFO.clone())),
        ];

        assert_eq!(nlas.as_slice().buffer_len(), 424);

        let mut vec = vec![0xff; 424];
        nlas.as_slice().emit(&mut vec);
        assert_eq!(&vec[..], &BRIDGE[..]);
    }
}
