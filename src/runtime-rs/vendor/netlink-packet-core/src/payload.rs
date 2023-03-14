// SPDX-License-Identifier: MIT

use std::fmt::Debug;

use crate::{AckMessage, ErrorMessage, NetlinkSerializable};

/// The message is ignored.
pub const NLMSG_NOOP: u16 = 1;
/// The message signals an error and the payload contains a nlmsgerr structure. This can be looked
/// at as a NACK and typically it is from FEC to CPC.
pub const NLMSG_ERROR: u16 = 2;
/// The message terminates a multipart message.
/// Data lost
pub const NLMSG_DONE: u16 = 3;
pub const NLMSG_OVERRUN: u16 = 4;
pub const NLMSG_ALIGNTO: u16 = 4;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NetlinkPayload<I> {
    Done,
    Error(ErrorMessage),
    Ack(AckMessage),
    Noop,
    Overrun(Vec<u8>),
    InnerMessage(I),
}

impl<I> NetlinkPayload<I>
where
    I: NetlinkSerializable,
{
    pub fn message_type(&self) -> u16 {
        match self {
            NetlinkPayload::Done => NLMSG_DONE,
            NetlinkPayload::Error(_) | NetlinkPayload::Ack(_) => NLMSG_ERROR,
            NetlinkPayload::Noop => NLMSG_NOOP,
            NetlinkPayload::Overrun(_) => NLMSG_OVERRUN,
            NetlinkPayload::InnerMessage(message) => message.message_type(),
        }
    }
}
