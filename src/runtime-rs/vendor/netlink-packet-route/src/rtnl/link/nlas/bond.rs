// SPDX-License-Identifier: MIT

use crate::{
    constants::*,
    nlas::{Nla, NlaBuffer, NlasIterator},
    parsers::{parse_ip, parse_mac, parse_u16, parse_u32, parse_u8},
    traits::{Emitable, Parseable},
    DecodeError,
};

use anyhow::Context;
use byteorder::{ByteOrder, NativeEndian};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    ops::Deref,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BondAdInfo {
    Aggregator(u16),
    NumPorts(u16),
    ActorKey(u16),
    PartnerKey(u16),
    PartnerMac([u8; 6]),
}

impl Nla for BondAdInfo {
    fn value_len(&self) -> usize {
        use self::BondAdInfo::*;
        match self {
            Aggregator(_) | NumPorts(_) | ActorKey(_) | PartnerKey(_) => 2,
            PartnerMac(_) => 6,
        }
    }

    fn kind(&self) -> u16 {
        use self::BondAdInfo::*;
        match self {
            Aggregator(_) => IFLA_BOND_AD_INFO_AGGREGATOR,
            NumPorts(_) => IFLA_BOND_AD_INFO_NUM_PORTS,
            ActorKey(_) => IFLA_BOND_AD_INFO_ACTOR_KEY,
            PartnerKey(_) => IFLA_BOND_AD_INFO_PARTNER_KEY,
            PartnerMac(_) => IFLA_BOND_AD_INFO_PARTNER_MAC,
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::BondAdInfo::*;
        match self {
            Aggregator(d) | NumPorts(d) | ActorKey(d) | PartnerKey(d) => {
                NativeEndian::write_u16(buffer, *d)
            }
            PartnerMac(mac) => buffer.copy_from_slice(mac),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for BondAdInfo {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::BondAdInfo::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_BOND_AD_INFO_AGGREGATOR => Aggregator(
                parse_u16(payload).context("invalid IFLA_BOND_AD_INFO_AGGREGATOR value")?,
            ),
            IFLA_BOND_AD_INFO_NUM_PORTS => {
                NumPorts(parse_u16(payload).context("invalid IFLA_BOND_AD_INFO_NUM_PORTS value")?)
            }
            IFLA_BOND_AD_INFO_ACTOR_KEY => {
                ActorKey(parse_u16(payload).context("invalid IFLA_BOND_AD_INFO_ACTOR_KEY value")?)
            }
            IFLA_BOND_AD_INFO_PARTNER_KEY => PartnerKey(
                parse_u16(payload).context("invalid IFLA_BOND_AD_INFO_PARTNER_KEY value")?,
            ),
            IFLA_BOND_AD_INFO_PARTNER_MAC => PartnerMac(
                parse_mac(payload).context("invalid IFLA_BOND_AD_INFO_PARTNER_MAC value")?,
            ),
            _ => return Err(format!("unknown NLA type {}", buf.kind()).into()),
        })
    }
}

// Some attributes (ARP_IP_TARGET, NS_IP6_TARGET) contain a nested
// list of IP addresses, where each element uses the index as NLA kind
// and the address as value. InfoBond exposes vectors of IP addresses,
// and we use this struct for serialization.
struct BondIpAddrNla {
    index: u16,
    addr: IpAddr,
}

struct BondIpAddrNlaList(Vec<BondIpAddrNla>);

impl Deref for BondIpAddrNlaList {
    type Target = Vec<BondIpAddrNla>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&Vec<Ipv4Addr>> for BondIpAddrNlaList {
    fn from(addrs: &Vec<Ipv4Addr>) -> Self {
        let mut nlas = Vec::new();
        for (i, addr) in addrs.iter().enumerate() {
            let nla = BondIpAddrNla {
                index: i as u16,
                addr: IpAddr::V4(*addr),
            };
            nlas.push(nla);
        }
        BondIpAddrNlaList(nlas)
    }
}

impl From<&Vec<Ipv6Addr>> for BondIpAddrNlaList {
    fn from(addrs: &Vec<Ipv6Addr>) -> Self {
        let mut nlas = Vec::new();
        for (i, addr) in addrs.iter().enumerate() {
            let nla = BondIpAddrNla {
                index: i as u16,
                addr: IpAddr::V6(*addr),
            };
            nlas.push(nla);
        }
        BondIpAddrNlaList(nlas)
    }
}

impl Nla for BondIpAddrNla {
    fn value_len(&self) -> usize {
        if self.addr.is_ipv4() {
            4
        } else {
            16
        }
    }
    fn emit_value(&self, buffer: &mut [u8]) {
        match self.addr {
            IpAddr::V4(addr) => buffer.copy_from_slice(&addr.octets()),
            IpAddr::V6(addr) => buffer.copy_from_slice(&addr.octets()),
        }
    }
    fn kind(&self) -> u16 {
        self.index
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InfoBond {
    Mode(u8),
    ActiveSlave(u32),
    MiiMon(u32),
    UpDelay(u32),
    DownDelay(u32),
    UseCarrier(u8),
    ArpInterval(u32),
    ArpIpTarget(Vec<Ipv4Addr>),
    ArpValidate(u32),
    ArpAllTargets(u32),
    Primary(u32),
    PrimaryReselect(u8),
    FailOverMac(u8),
    XmitHashPolicy(u8),
    ResendIgmp(u32),
    NumPeerNotif(u8),
    AllSlavesActive(u8),
    MinLinks(u32),
    LpInterval(u32),
    PacketsPerSlave(u32),
    AdLacpRate(u8),
    AdSelect(u8),
    AdInfo(Vec<BondAdInfo>),
    AdActorSysPrio(u16),
    AdUserPortKey(u16),
    AdActorSystem([u8; 6]),
    TlbDynamicLb(u8),
    PeerNotifDelay(u32),
    AdLacpActive(u8),
    MissedMax(u8),
    NsIp6Target(Vec<Ipv6Addr>),
}

impl Nla for InfoBond {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::InfoBond::*;
        match *self {
            Mode(_)
                | UseCarrier(_)
                | PrimaryReselect(_)
                | FailOverMac(_)
                | XmitHashPolicy(_)
                | NumPeerNotif(_)
                | AllSlavesActive(_)
                | AdLacpActive(_)
                | AdLacpRate(_)
                | AdSelect(_)
                | TlbDynamicLb(_)
                | MissedMax(_)
            => 1,
            AdActorSysPrio(_)
                | AdUserPortKey(_)
            => 2,
            ActiveSlave(_)
                | MiiMon(_)
                | UpDelay(_)
                | DownDelay(_)
                | ArpInterval(_)
                | ArpValidate(_)
                | ArpAllTargets(_)
                | Primary(_)
                | ResendIgmp(_)
                | MinLinks(_)
                | LpInterval(_)
                | PacketsPerSlave(_)
                | PeerNotifDelay(_)
                => 4,
            ArpIpTarget(ref addrs)
                => {
                    BondIpAddrNlaList::from(addrs).as_slice().buffer_len()
                },
            NsIp6Target(ref addrs)
                =>  {
                    BondIpAddrNlaList::from(addrs).as_slice().buffer_len()
                },
            AdActorSystem(_) => 6,
            AdInfo(ref infos)
            => infos.as_slice().buffer_len(),
        }
    }

    #[rustfmt::skip]
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::InfoBond::*;
        match self {
            Mode(value)
                | UseCarrier(value)
                | PrimaryReselect(value)
                | FailOverMac(value)
                | XmitHashPolicy(value)
                | NumPeerNotif(value)
                | AllSlavesActive(value)
                | AdLacpActive(value)
                | AdLacpRate(value)
                | AdSelect(value)
                | TlbDynamicLb(value)
                | MissedMax(value)
            => buffer[0] = *value,
            AdActorSysPrio(value)
                | AdUserPortKey(value)
            => NativeEndian::write_u16(buffer, *value),
            ActiveSlave(value)
                | MiiMon(value)
                | UpDelay(value)
                | DownDelay(value)
                | ArpInterval(value)
                | ArpValidate(value)
                | ArpAllTargets(value)
                | Primary(value)
                | ResendIgmp(value)
                | MinLinks(value)
                | LpInterval(value)
                | PacketsPerSlave(value)
                | PeerNotifDelay(value)
             => NativeEndian::write_u32(buffer, *value),
            AdActorSystem(bytes) => buffer.copy_from_slice(bytes),
            ArpIpTarget(addrs) => {
                BondIpAddrNlaList::from(addrs).as_slice().emit(buffer)
            },
            NsIp6Target(addrs) => {
                BondIpAddrNlaList::from(addrs).as_slice().emit(buffer)
            },
            AdInfo(infos) => infos.as_slice().emit(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::InfoBond::*;

        match self {
            Mode(_) => IFLA_BOND_MODE,
            ActiveSlave(_) => IFLA_BOND_ACTIVE_SLAVE,
            MiiMon(_) => IFLA_BOND_MIIMON,
            UpDelay(_) => IFLA_BOND_UPDELAY,
            DownDelay(_) => IFLA_BOND_DOWNDELAY,
            UseCarrier(_) => IFLA_BOND_USE_CARRIER,
            ArpInterval(_) => IFLA_BOND_ARP_INTERVAL,
            ArpIpTarget(_) => IFLA_BOND_ARP_IP_TARGET,
            ArpValidate(_) => IFLA_BOND_ARP_VALIDATE,
            ArpAllTargets(_) => IFLA_BOND_ARP_ALL_TARGETS,
            Primary(_) => IFLA_BOND_PRIMARY,
            PrimaryReselect(_) => IFLA_BOND_PRIMARY_RESELECT,
            FailOverMac(_) => IFLA_BOND_FAIL_OVER_MAC,
            XmitHashPolicy(_) => IFLA_BOND_XMIT_HASH_POLICY,
            ResendIgmp(_) => IFLA_BOND_RESEND_IGMP,
            NumPeerNotif(_) => IFLA_BOND_NUM_PEER_NOTIF,
            AllSlavesActive(_) => IFLA_BOND_ALL_SLAVES_ACTIVE,
            MinLinks(_) => IFLA_BOND_MIN_LINKS,
            LpInterval(_) => IFLA_BOND_LP_INTERVAL,
            PacketsPerSlave(_) => IFLA_BOND_PACKETS_PER_SLAVE,
            AdLacpRate(_) => IFLA_BOND_AD_LACP_RATE,
            AdSelect(_) => IFLA_BOND_AD_SELECT,
            AdInfo(_) => IFLA_BOND_AD_INFO,
            AdActorSysPrio(_) => IFLA_BOND_AD_ACTOR_SYS_PRIO,
            AdUserPortKey(_) => IFLA_BOND_AD_USER_PORT_KEY,
            AdActorSystem(_) => IFLA_BOND_AD_ACTOR_SYSTEM,
            TlbDynamicLb(_) => IFLA_BOND_TLB_DYNAMIC_LB,
            PeerNotifDelay(_) => IFLA_BOND_PEER_NOTIF_DELAY,
            AdLacpActive(_) => IFLA_BOND_AD_LACP_ACTIVE,
            MissedMax(_) => IFLA_BOND_MISSED_MAX,
            NsIp6Target(_) => IFLA_BOND_NS_IP6_TARGET,
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for InfoBond {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::InfoBond::*;
        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_BOND_MODE => Mode(parse_u8(payload).context("invalid IFLA_BOND_MODE value")?),
            IFLA_BOND_ACTIVE_SLAVE => {
                ActiveSlave(parse_u32(payload).context("invalid IFLA_BOND_ACTIVE_SLAVE value")?)
            }
            IFLA_BOND_MIIMON => {
                MiiMon(parse_u32(payload).context("invalid IFLA_BOND_MIIMON value")?)
            }
            IFLA_BOND_UPDELAY => {
                UpDelay(parse_u32(payload).context("invalid IFLA_BOND_UPDELAY value")?)
            }
            IFLA_BOND_DOWNDELAY => {
                DownDelay(parse_u32(payload).context("invalid IFLA_BOND_DOWNDELAY value")?)
            }
            IFLA_BOND_USE_CARRIER => {
                UseCarrier(parse_u8(payload).context("invalid IFLA_BOND_USE_CARRIER value")?)
            }
            IFLA_BOND_ARP_INTERVAL => {
                ArpInterval(parse_u32(payload).context("invalid IFLA_BOND_ARP_INTERVAL value")?)
            }
            IFLA_BOND_ARP_IP_TARGET => {
                let mut addrs = Vec::<Ipv4Addr>::new();
                for nla in NlasIterator::new(payload) {
                    let nla = &nla.context("invalid IFLA_BOND_ARP_IP_TARGET value")?;
                    if let Ok(IpAddr::V4(addr)) = parse_ip(nla.value()) {
                        addrs.push(addr);
                    }
                }
                ArpIpTarget(addrs)
            }
            IFLA_BOND_ARP_VALIDATE => {
                ArpValidate(parse_u32(payload).context("invalid IFLA_BOND_ARP_VALIDATE value")?)
            }
            IFLA_BOND_ARP_ALL_TARGETS => ArpAllTargets(
                parse_u32(payload).context("invalid IFLA_BOND_ARP_ALL_TARGETS value")?,
            ),
            IFLA_BOND_PRIMARY => {
                Primary(parse_u32(payload).context("invalid IFLA_BOND_PRIMARY value")?)
            }
            IFLA_BOND_PRIMARY_RESELECT => PrimaryReselect(
                parse_u8(payload).context("invalid IFLA_BOND_PRIMARY_RESELECT value")?,
            ),
            IFLA_BOND_FAIL_OVER_MAC => {
                FailOverMac(parse_u8(payload).context("invalid IFLA_BOND_FAIL_OVER_MAC value")?)
            }
            IFLA_BOND_XMIT_HASH_POLICY => XmitHashPolicy(
                parse_u8(payload).context("invalid IFLA_BOND_XMIT_HASH_POLICY value")?,
            ),
            IFLA_BOND_RESEND_IGMP => {
                ResendIgmp(parse_u32(payload).context("invalid IFLA_BOND_RESEND_IGMP value")?)
            }
            IFLA_BOND_NUM_PEER_NOTIF => {
                NumPeerNotif(parse_u8(payload).context("invalid IFLA_BOND_NUM_PEER_NOTIF value")?)
            }
            IFLA_BOND_ALL_SLAVES_ACTIVE => AllSlavesActive(
                parse_u8(payload).context("invalid IFLA_BOND_ALL_SLAVES_ACTIVE value")?,
            ),
            IFLA_BOND_MIN_LINKS => {
                MinLinks(parse_u32(payload).context("invalid IFLA_BOND_MIN_LINKS value")?)
            }
            IFLA_BOND_LP_INTERVAL => {
                LpInterval(parse_u32(payload).context("invalid IFLA_BOND_LP_INTERVAL value")?)
            }
            IFLA_BOND_PACKETS_PER_SLAVE => PacketsPerSlave(
                parse_u32(payload).context("invalid IFLA_BOND_PACKETS_PER_SLAVE value")?,
            ),
            IFLA_BOND_AD_LACP_RATE => {
                AdLacpRate(parse_u8(payload).context("invalid IFLA_BOND_AD_LACP_RATE value")?)
            }
            IFLA_BOND_AD_SELECT => {
                AdSelect(parse_u8(payload).context("invalid IFLA_BOND_AD_SELECT value")?)
            }
            IFLA_BOND_AD_INFO => {
                let mut infos = Vec::new();
                let err = "failed to parse IFLA_BOND_AD_INFO";
                for nla in NlasIterator::new(payload) {
                    let nla = &nla.context(err)?;
                    let info = BondAdInfo::parse(nla).context(err)?;
                    infos.push(info);
                }
                AdInfo(infos)
            }
            IFLA_BOND_AD_ACTOR_SYS_PRIO => AdActorSysPrio(
                parse_u16(payload).context("invalid IFLA_BOND_AD_ACTOR_SYS_PRIO value")?,
            ),
            IFLA_BOND_AD_USER_PORT_KEY => AdUserPortKey(
                parse_u16(payload).context("invalid IFLA_BOND_AD_USER_PORT_KEY value")?,
            ),
            IFLA_BOND_AD_ACTOR_SYSTEM => AdActorSystem(
                parse_mac(payload).context("invalid IFLA_BOND_AD_ACTOR_SYSTEM value")?,
            ),
            IFLA_BOND_TLB_DYNAMIC_LB => {
                TlbDynamicLb(parse_u8(payload).context("invalid IFLA_BOND_TLB_DYNAMIC_LB value")?)
            }
            IFLA_BOND_PEER_NOTIF_DELAY => PeerNotifDelay(
                parse_u32(payload).context("invalid IFLA_BOND_PEER_NOTIF_DELAY value")?,
            ),
            IFLA_BOND_AD_LACP_ACTIVE => {
                AdLacpActive(parse_u8(payload).context("invalid IFLA_BOND_AD_LACP_ACTIVE value")?)
            }
            IFLA_BOND_MISSED_MAX => {
                MissedMax(parse_u8(payload).context("invalid IFLA_BOND_MISSED_MAX value")?)
            }
            IFLA_BOND_NS_IP6_TARGET => {
                let mut addrs = Vec::<Ipv6Addr>::new();
                for nla in NlasIterator::new(payload) {
                    let nla = &nla.context("invalid IFLA_BOND_NS_IP6_TARGET value")?;
                    if let Ok(IpAddr::V6(addr)) = parse_ip(nla.value()) {
                        addrs.push(addr);
                    }
                }
                NsIp6Target(addrs)
            }
            _ => return Err(format!("unknown NLA type {}", buf.kind()).into()),
        })
    }
}
