// SPDX-License-Identifier: MIT

use anyhow::Context;

use crate::{
    constants::{IFLA_INET_CONF, IFLA_INET_UNSPEC},
    nlas::{DefaultNla, Nla, NlaBuffer},
    traits::Parseable,
    DecodeError,
};

mod dev_conf;
pub use self::dev_conf::*;

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum Inet {
    DevConf(Vec<u8>),
    Unspec(Vec<u8>),
    Other(DefaultNla),
}

impl Nla for Inet {
    fn value_len(&self) -> usize {
        use self::Inet::*;
        match *self {
            Unspec(ref bytes) => bytes.len(),
            DevConf(_) => DEV_CONF_LEN,
            Other(ref nla) => nla.value_len(),
        }
    }

    fn emit_value(&self, buffer: &mut [u8]) {
        use self::Inet::*;
        match *self {
            Unspec(ref bytes) => buffer[..bytes.len()].copy_from_slice(bytes.as_slice()),
            DevConf(ref dev_conf) => buffer[..dev_conf.len()].copy_from_slice(dev_conf.as_slice()),
            Other(ref nla) => nla.emit_value(buffer),
        }
    }

    fn kind(&self) -> u16 {
        use self::Inet::*;
        match *self {
            Unspec(_) => IFLA_INET_UNSPEC,
            DevConf(_) => IFLA_INET_CONF,
            Other(ref nla) => nla.kind(),
        }
    }
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<NlaBuffer<&'a T>> for Inet {
    fn parse(buf: &NlaBuffer<&'a T>) -> Result<Self, DecodeError> {
        use self::Inet::*;

        let payload = buf.value();
        Ok(match buf.kind() {
            IFLA_INET_UNSPEC => Unspec(payload.to_vec()),
            IFLA_INET_CONF => DevConf(payload.to_vec()),
            kind => Other(DefaultNla::parse(buf).context(format!("unknown NLA type {}", kind))?),
        })
    }
}
