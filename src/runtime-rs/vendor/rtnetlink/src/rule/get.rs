// SPDX-License-Identifier: MIT

use crate::IpVersion;
use futures::{
    future::{self, Either},
    stream::{StreamExt, TryStream},
    FutureExt,
};

use netlink_packet_route::{constants::*, NetlinkMessage, RtnlMessage, RuleMessage};

use crate::{try_rtnl, Error, Handle};

pub struct RuleGetRequest {
    handle: Handle,
    message: RuleMessage,
}

impl RuleGetRequest {
    pub(crate) fn new(handle: Handle, ip_version: IpVersion) -> Self {
        let mut message = RuleMessage::default();
        message.header.family = ip_version.family();

        message.header.dst_len = 0;
        message.header.src_len = 0;
        message.header.tos = 0;
        message.header.action = FR_ACT_UNSPEC;
        message.header.table = RT_TABLE_UNSPEC;

        RuleGetRequest { handle, message }
    }

    pub fn message_mut(&mut self) -> &mut RuleMessage {
        &mut self.message
    }

    pub fn execute(self) -> impl TryStream<Ok = RuleMessage, Error = Error> {
        let RuleGetRequest {
            mut handle,
            message,
        } = self;

        let mut req = NetlinkMessage::from(RtnlMessage::GetRule(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_DUMP;

        match handle.request(req) {
            Ok(response) => {
                Either::Left(response.map(move |msg| Ok(try_rtnl!(msg, RtnlMessage::NewRule))))
            }
            Err(e) => Either::Right(future::err::<RuleMessage, Error>(e).into_stream()),
        }
    }
}
