// SPDX-License-Identifier: MIT

use anyhow::Context;

use super::{inet, inet6};
use crate::{
    constants::*,
    nlas::{self, DefaultNla, NlaBuffer, NlasIterator},
    traits::{Emitable, Parseable},
    DecodeError,
};

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum AfSpecInet {
    Unspec(Vec<u8>),
    Unix(Vec<u8>),
    Ax25(Vec<u8>),
    Ipx(Vec<u8>),
    AppleTalk(Vec<u8>),
    Netrom(Vec<u8>),
    Bridge(Vec<u8>),
    AtmPvc(Vec<u8>),
    X25(Vec<u8>),
    Inet(Vec<inet::Inet>),
    Inet6(Vec<inet6::Inet6>),
    Rose(Vec<u8>),
    DecNet(Vec<u8>),
    NetbEui(Vec<u8>),
    Security(Vec<u8>),
    Key(Vec<u8>),
    Netlink(Vec<u8>),
    Packet(Vec<u8>),
    Ash(Vec<u8>),
    EcoNet(Vec<u8>),
    AtmSvc(Vec<u8>),
    Rds(Vec<u8>),
    Sna(Vec<u8>),
    Irda(Vec<u8>),
    Pppox(Vec<u8>),
    WanPipe(Vec<u8>),
    Llc(Vec<u8>),
    Can(Vec<u8>),
    Tipc(Vec<u8>),
    Bluetooth(Vec<u8>),
    Iucv(Vec<u8>),
    RxRpc(Vec<u8>),
    Isdn(Vec<u8>),
    Phonet(Vec<u8>),
    Ieee802154(Vec<u8>),
    Caif(Vec<u8>),
    Alg(Vec<u8>),
    Other(DefaultNla),
}

impl nlas::Nla for AfSpecInet {
    #[rustfmt::skip]
    fn value_len(&self) -> usize {
        use self::AfSpecInet::*;
        match *self {
            Unspec(ref bytes)
                | Unix(ref bytes)
                | Ax25(ref bytes)
                | Ipx(ref bytes)
                | AppleTalk(ref bytes)
                | Netrom(ref bytes)
                | Bridge(ref bytes)
                | AtmPvc(ref bytes)
                | X25(ref bytes)
                | Rose(ref bytes)
                | DecNet(ref bytes)
                | NetbEui(ref bytes)
                | Security(ref bytes)
                | Key(ref bytes)
                | Netlink(ref bytes)
                | Packet(ref bytes)
                | Ash(ref bytes)
                | EcoNet(ref bytes)
                | AtmSvc(ref bytes)
                | Rds(ref bytes)
                | Sna(ref bytes)
                | Irda(ref bytes)
                | Pppox(ref bytes)
                | WanPipe(ref bytes)
                | Llc(ref bytes)
                | Can(ref bytes)
                | Tipc(ref bytes)
                | Bluetooth(ref bytes)
                | Iucv(ref bytes)
                | RxRpc(ref bytes)
                | Isdn(ref bytes)
                | Phonet(ref bytes)
                | Ieee802154(ref bytes)
                | Caif(ref bytes)
                | Alg(ref bytes)
                => bytes.len(),
            Inet6(ref nlas) => nlas.as_slice().buffer_len(),
            Inet(ref nlas) =>  nlas.as_slice().buffer_len(),
            Other(ref nla) => nla.value_len(),
        }
    }

    #[rustfmt::skip]
    fn emit_value(&self, buffer: &mut [u8]) {
        use self::AfSpecInet::*;
        match *self {
            Unspec(ref bytes)
                | Unix(ref bytes)
                | Ax25(ref bytes)
                | Ipx(ref bytes)
                | AppleTalk(ref bytes)
                | Netrom(ref bytes)
                | Bridge(ref bytes)
                | AtmPvc(ref bytes)
                | X25(ref bytes)
                | Rose(ref bytes)
                | DecNet(ref bytes)
                | NetbEui(ref bytes)
                | Security(ref bytes)
                | Key(ref bytes)
                | Netlink(ref bytes)
                | Packet(ref bytes)
                | Ash(ref bytes)
                | EcoNet(ref bytes)
                | AtmSvc(ref bytes)
                | Rds(ref bytes)
                | Sna(ref bytes)
                | Irda(ref bytes)
                | Pppox(ref bytes)
                | WanPipe(ref bytes)
                | Llc(ref bytes)
                | Can(ref bytes)
                | Tipc(ref bytes)
                | Bluetooth(ref bytes)
                | Iucv(ref bytes)
                | RxRpc(ref bytes)
                | Isdn(ref bytes)
                | Phonet(ref bytes)
                | Ieee802154(ref bytes)
                | Caif(ref bytes)
                | Alg(ref bytes)
                => buffer[..bytes.len()].copy_from_slice(bytes.as_slice()),
            Inet6(ref nlas) => nlas.as_slice().emit(buffer),
            Inet(ref nlas) => nlas.as_slice().emit(buffer),
            Other(ref nla)  => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::AfSpecInet::*;
        match *self {
            Inet(_) => AF_INET,
            Unspec(_) => AF_UNSPEC,
            Unix(_) => AF_UNIX,
            Ax25(_) => AF_AX25,
            Ipx(_) => AF_IPX,
            AppleTalk(_) => AF_APPLETALK,
            Netrom(_) => AF_NETROM,
            Bridge(_) => AF_BRIDGE,
            AtmPvc(_) => AF_ATMPVC,
            X25(_) => AF_X25,
            Inet6(_) => AF_INET6,
            Rose(_) => AF_ROSE,
            DecNet(_) => AF_DECNET,
            NetbEui(_) => AF_NETBEUI,
            Security(_) => AF_SECURITY,
            Key(_) => AF_KEY,
            Netlink(_) => AF_NETLINK,
            Packet(_) => AF_PACKET,
            Ash(_) => AF_ASH,
            EcoNet(_) => AF_ECONET,
            AtmSvc(_) => AF_ATMSVC,
            Rds(_) => AF_RDS,
            Sna(_) => AF_SNA,
            Irda(_) => AF_IRDA,
            Pppox(_) => AF_PPPOX,
            WanPipe(_) => AF_WANPIPE,
            Llc(_) => AF_LLC,
            Can(_) => AF_CAN,
            Tipc(_) => AF_TIPC,
            Bluetooth(_) => AF_BLUETOOTH,
            Iucv(_) => AF_IUCV,
            RxRpc(_) => AF_RXRPC,
            Isdn(_) => AF_ISDN,
            Phonet(_) => AF_PHONET,
            Ieee802154(_) => AF_IEEE802154,
            Caif(_) => AF_CAIF,
            Alg(_) => AF_ALG,
            Other(ref nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for AfSpecInet {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::AfSpecInet::*;

        let payload = buf.value();
        Ok(match buf.kind() {
            AF_UNSPEC => Unspec(payload.to_vec()),
            AF_INET => {
                let mut nlas = vec![];
                for nla in NlasIterator::new(payload) {
                    let nla = nla.context("invalid AF_INET value")?;
                    nlas.push(inet::Inet::parse(&nla).context("invalid AF_INET value")?);
                }
                Inet(nlas)
            }
            AF_INET6 => {
                let mut nlas = vec![];
                for nla in NlasIterator::new(payload) {
                    let nla = nla.context("invalid AF_INET6 value")?;
                    nlas.push(inet6::Inet6::parse(&nla).context("invalid AF_INET6 value")?);
                }
                Inet6(nlas)
            }
            AF_UNIX => Unix(payload.to_vec()),
            AF_AX25 => Ax25(payload.to_vec()),
            AF_IPX => Ipx(payload.to_vec()),
            AF_APPLETALK => AppleTalk(payload.to_vec()),
            AF_NETROM => Netrom(payload.to_vec()),
            AF_BRIDGE => Bridge(payload.to_vec()),
            AF_ATMPVC => AtmPvc(payload.to_vec()),
            AF_X25 => X25(payload.to_vec()),
            AF_ROSE => Rose(payload.to_vec()),
            AF_DECNET => DecNet(payload.to_vec()),
            AF_NETBEUI => NetbEui(payload.to_vec()),
            AF_SECURITY => Security(payload.to_vec()),
            AF_KEY => Key(payload.to_vec()),
            AF_NETLINK => Netlink(payload.to_vec()),
            AF_PACKET => Packet(payload.to_vec()),
            AF_ASH => Ash(payload.to_vec()),
            AF_ECONET => EcoNet(payload.to_vec()),
            AF_ATMSVC => AtmSvc(payload.to_vec()),
            AF_RDS => Rds(payload.to_vec()),
            AF_SNA => Sna(payload.to_vec()),
            AF_IRDA => Irda(payload.to_vec()),
            AF_PPPOX => Pppox(payload.to_vec()),
            AF_WANPIPE => WanPipe(payload.to_vec()),
            AF_LLC => Llc(payload.to_vec()),
            AF_CAN => Can(payload.to_vec()),
            AF_TIPC => Tipc(payload.to_vec()),
            AF_BLUETOOTH => Bluetooth(payload.to_vec()),
            AF_IUCV => Iucv(payload.to_vec()),
            AF_RXRPC => RxRpc(payload.to_vec()),
            AF_ISDN => Isdn(payload.to_vec()),
            AF_PHONET => Phonet(payload.to_vec()),
            AF_IEEE802154 => Ieee802154(payload.to_vec()),
            AF_CAIF => Caif(payload.to_vec()),
            AF_ALG => Alg(payload.to_vec()),
            kind => Other(DefaultNla::parse(buf).context(format!("Unknown NLA type {}", kind))?),
        })
    }
}
