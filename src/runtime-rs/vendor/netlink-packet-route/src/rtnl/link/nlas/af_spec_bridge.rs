// SPDX-License-Identifier: MIT

use std::convert::TryFrom;

use anyhow::Context;

use crate::{
    constants::*,
    nlas::{self, DefaultNla, NlaBuffer},
    parsers::parse_u16,
    traits::Parseable,
    DecodeError,
};

use byteorder::{ByteOrder, NativeEndian};

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum AfSpecBridge {
    Flags(u16),
    VlanInfo(BridgeVlanInfo),
    Other(DefaultNla),
}

impl nlas::Nla for AfSpecBridge {
    fn value_len(&self) -> usize {
        use self::AfSpecBridge::*;
        match *self {
            VlanInfo(_) => 4,
            Flags(_) => 2,
            Other(ref nla) => nla.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::AfSpecBridge::*;
        match *self {
            Flags(value) => NativeEndian::write_u16(buffer, value),
            VlanInfo(ref info) => {
                (&mut buffer[..4]).copy_from_slice(<[u8; 4]>::from(info).as_slice())
            }
            Other(ref nla) => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::AfSpecBridge::*;
        match *self {
            Flags(_) => IFLA_BRIDGE_FLAGS,
            VlanInfo(_) => IFLA_BRIDGE_VLAN_INFO,
            Other(ref nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for AfSpecBridge {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::AfSpecBridge::*;

        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_BRIDGE_VLAN_INFO => VlanInfo(
                BridgeVlanInfo::try_from(payload).context("Invalid IFLA_BRIDGE_VLAN_INFO value")?,
            ),
            IFLA_BRIDGE_FLAGS => {
                Flags(parse_u16(payload).context("invalid IFLA_BRIDGE_FLAGS value")?)
            }
            kind => Other(DefaultNla::parse(buf).context(format!("Unknown NLA type {}", kind))?),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub struct BridgeVlanInfo {
    pub flags: u16,
    pub vid: u16,
}

impl From<&BridgeVlanInfo> for [u8; 4] {
    fn from(d: &BridgeVlanInfo) -> Self {
        let mut ret = [0u8; 4];
        NativeEndian::write_u16(&mut ret[0..2], d.flags);
        NativeEndian::write_u16(&mut ret[2..4], d.vid);
        ret
    }
}

impl TryFrom<&[u8]> for BridgeVlanInfo {
    type Error = DecodeError;
    fn try_from(raw: &[u8]) -> Result<Self, DecodeError> {
        if raw.len() == 4 {
            Ok(Self {
                flags: parse_u16(&raw[0..2])
                    .context(format!("Invalid IFLA_BRIDGE_VLAN_INFO value: {:?}", raw))?,
                vid: parse_u16(&raw[2..4])
                    .context(format!("Invalid IFLA_BRIDGE_VLAN_INFO value: {:?}", raw))?,
            })
        } else {
            Err(DecodeError::from(format!(
                "Invalid IFLA_BRIDGE_VLAN_INFO value, expecting [u8;4], but got {:?}",
                raw
            )))
        }
    }
}
