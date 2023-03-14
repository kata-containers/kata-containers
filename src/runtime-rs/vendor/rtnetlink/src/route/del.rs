// SPDX-License-Identifier: MIT

use futures::stream::StreamExt;

use crate::{
    packet::{NetlinkMessage, NetlinkPayload, RouteMessage, RtnlMessage, NLM_F_ACK, NLM_F_REQUEST},
    Error,
    Handle,
};

pub struct RouteDelRequest {
    handle: Handle,
    message: RouteMessage,
}

impl RouteDelRequest {
    pub(crate) fn new(handle: Handle, message: RouteMessage) -> Self {
        RouteDelRequest { handle, message }
    }

    /// Execute the request
    pub async fn execute(self) -> Result<(), Error> {
        let RouteDelRequest {
            mut handle,
            message,
        } = self;

        let mut req = NetlinkMessage::from(RtnlMessage::DelRoute(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK;
        let mut response = handle.request(req)?;
        while let Some(msg) = response.next().await {
            if let NetlinkPayload::Error(e) = msg.payload {
                return Err(Error::NetlinkError(e));
            }
        }
        Ok(())
    }

    pub fn message_mut(&mut self) -> &mut RouteMessage {
        &mut self.message
    }
}
