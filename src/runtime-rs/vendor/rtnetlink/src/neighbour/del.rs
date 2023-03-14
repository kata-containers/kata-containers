// SPDX-License-Identifier: MIT

use futures::stream::StreamExt;

use netlink_packet_route::{
    constants::*,
    neighbour::NeighbourMessage,
    NetlinkPayload,
    RtnlMessage,
};

use netlink_proto::packet::NetlinkMessage;

use crate::{Error, Handle};

pub struct NeighbourDelRequest {
    handle: Handle,
    message: NeighbourMessage,
}

impl NeighbourDelRequest {
    pub(crate) fn new(handle: Handle, message: NeighbourMessage) -> Self {
        NeighbourDelRequest { handle, message }
    }

    /// Execute the request
    pub async fn execute(self) -> Result<(), Error> {
        let NeighbourDelRequest {
            mut handle,
            message,
        } = self;

        let mut req = NetlinkMessage::from(RtnlMessage::DelNeighbour(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK;
        let mut response = handle.request(req)?;
        while let Some(msg) = response.next().await {
            if let NetlinkPayload::Error(e) = msg.payload {
                return Err(Error::NetlinkError(e));
            }
        }
        Ok(())
    }

    pub fn message_mut(&mut self) -> &mut NeighbourMessage {
        &mut self.message
    }
}
