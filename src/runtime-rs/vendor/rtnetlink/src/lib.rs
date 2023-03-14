// SPDX-License-Identifier: MIT

//! This crate provides methods to manipulate networking resources (links, addresses, arp tables,
//! route tables) via the netlink protocol.

#![allow(clippy::module_inception)]

mod handle;
pub use crate::handle::*;

mod ns;
pub use crate::ns::*;

mod errors;
pub use crate::errors::*;

mod link;
pub use crate::link::*;

mod addr;
pub use crate::addr::*;

mod route;
pub use crate::route::*;

mod rule;
pub use crate::rule::*;

mod connection;
pub use crate::connection::*;

mod traffic_control;
pub use crate::traffic_control::*;

mod neighbour;
pub use crate::neighbour::*;

pub mod constants;

pub use netlink_packet_route as packet;
pub mod proto {
    pub use netlink_proto::{
        packet::{NetlinkMessage, NetlinkPayload},
        Connection,
        ConnectionHandle,
        Error,
    };
}
pub use netlink_proto::sys;

mod macros;
