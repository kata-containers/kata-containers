// SPDX-License-Identifier: MIT

use futures::{
    future::{self, Either},
    stream::{StreamExt, TryStream},
    FutureExt,
};

use netlink_packet_route::{constants::*, NetlinkMessage, RouteMessage, RtnlMessage};

use crate::{try_rtnl, Error, Handle};

pub struct RouteGetRequest {
    handle: Handle,
    message: RouteMessage,
}

/// Internet Protocol (IP) version.
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd)]
pub enum IpVersion {
    /// IPv4
    V4,
    /// IPv6
    V6,
}

impl IpVersion {
    pub(crate) fn family(self) -> u8 {
        match self {
            IpVersion::V4 => AF_INET as u8,
            IpVersion::V6 => AF_INET6 as u8,
        }
    }
}

impl RouteGetRequest {
    pub(crate) fn new(handle: Handle, ip_version: IpVersion) -> Self {
        let mut message = RouteMessage::default();
        message.header.address_family = ip_version.family();

        // As per rtnetlink(7) documentation, setting the following
        // fields to 0 gets us all the routes from all the tables
        //
        // > For RTM_GETROUTE, setting rtm_dst_len and rtm_src_len to 0
        // > means you get all entries for the specified routing table.
        // > For the other fields, except rtm_table and rtm_protocol, 0
        // > is the wildcard.
        message.header.destination_prefix_length = 0;
        message.header.source_prefix_length = 0;
        message.header.scope = RT_SCOPE_UNIVERSE;
        message.header.kind = RTN_UNSPEC;

        // I don't know if these two fields matter
        message.header.table = RT_TABLE_UNSPEC;
        message.header.protocol = RTPROT_UNSPEC;

        RouteGetRequest { handle, message }
    }

    pub fn message_mut(&mut self) -> &mut RouteMessage {
        &mut self.message
    }

    pub fn execute(self) -> impl TryStream<Ok = RouteMessage, Error = Error> {
        let RouteGetRequest {
            mut handle,
            message,
        } = self;

        let mut req = NetlinkMessage::from(RtnlMessage::GetRoute(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_DUMP;

        match handle.request(req) {
            Ok(response) => {
                Either::Left(response.map(move |msg| Ok(try_rtnl!(msg, RtnlMessage::NewRoute))))
            }
            Err(e) => Either::Right(future::err::<RouteMessage, Error>(e).into_stream()),
        }
    }
}
