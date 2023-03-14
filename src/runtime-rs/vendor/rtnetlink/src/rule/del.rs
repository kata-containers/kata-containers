// SPDX-License-Identifier: MIT

use futures::stream::StreamExt;

use crate::{
    packet::{NetlinkMessage, RtnlMessage, RuleMessage, NLM_F_ACK, NLM_F_REQUEST},
    try_nl,
    Error,
    Handle,
};

pub struct RuleDelRequest {
    handle: Handle,
    message: RuleMessage,
}

impl RuleDelRequest {
    pub(crate) fn new(handle: Handle, message: RuleMessage) -> Self {
        RuleDelRequest { handle, message }
    }

    /// Execute the request
    pub async fn execute(self) -> Result<(), Error> {
        let RuleDelRequest {
            mut handle,
            message,
        } = self;

        let mut req = NetlinkMessage::from(RtnlMessage::DelRule(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK;
        let mut response = handle.request(req)?;
        while let Some(msg) = response.next().await {
            try_nl!(msg);
        }
        Ok(())
    }

    pub fn message_mut(&mut self) -> &mut RuleMessage {
        &mut self.message
    }
}
