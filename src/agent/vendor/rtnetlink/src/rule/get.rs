use crate::IpVersion;
use futures::{
    future::{self, Either},
    stream::{StreamExt, TryStream},
    FutureExt,
};

use netlink_packet_route::{
    constants::*,
    NetlinkMessage,
    NetlinkPayload,
    RtnlMessage,
    RuleMessage,
};

use crate::{Error, Handle};

pub struct RuleGetRequest {
    handle: Handle,
    message: RuleMessage,
}

impl RuleGetRequest {
    pub(crate) fn new(handle: Handle, ip_version: IpVersion) -> Self {
        let mut message = RuleMessage::default();
        message.header.family = match ip_version {
            IpVersion::V4 => AF_INET as u8,
            IpVersion::V6 => AF_INET6 as u8,
        };

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
            Ok(response) => Either::Left(response.map(move |msg| {
                let (header, payload) = msg.into_parts();
                match payload {
                    NetlinkPayload::InnerMessage(RtnlMessage::NewRule(msg)) => Ok(msg),
                    NetlinkPayload::Error(err) => Err(Error::NetlinkError(err)),
                    _ => Err(Error::UnexpectedMessage(NetlinkMessage::new(
                        header, payload,
                    ))),
                }
            })),
            Err(e) => Either::Right(future::err::<RuleMessage, Error>(e).into_stream()),
        }
    }
}
