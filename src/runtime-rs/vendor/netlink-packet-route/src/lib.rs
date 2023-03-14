// SPDX-License-Identifier: MIT

#[macro_use]
extern crate bitflags;
#[macro_use]
pub(crate) extern crate netlink_packet_utils as utils;
pub(crate) use self::utils::parsers;
pub use self::utils::{traits, DecodeError};

pub use netlink_packet_core::{
    ErrorMessage,
    NetlinkBuffer,
    NetlinkHeader,
    NetlinkMessage,
    NetlinkPayload,
};
pub(crate) use netlink_packet_core::{NetlinkDeserializable, NetlinkSerializable};

pub mod rtnl;
pub use self::rtnl::*;

#[cfg(test)]
#[macro_use]
extern crate lazy_static;

#[cfg(test)]
#[macro_use]
extern crate pretty_assertions;
