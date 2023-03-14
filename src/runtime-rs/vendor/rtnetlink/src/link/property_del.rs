// SPDX-License-Identifier: MIT

use crate::{
    packet::{
        nlas::link::{Nla, Prop},
        LinkMessage,
        NetlinkMessage,
        NetlinkPayload,
        RtnlMessage,
        NLM_F_ACK,
        NLM_F_EXCL,
        NLM_F_REQUEST,
    },
    Error,
    Handle,
};
use futures::stream::StreamExt;

pub struct LinkDelPropRequest {
    handle: Handle,
    message: LinkMessage,
}

impl LinkDelPropRequest {
    pub(crate) fn new(handle: Handle, index: u32) -> Self {
        let mut message = LinkMessage::default();
        message.header.index = index;
        LinkDelPropRequest { handle, message }
    }

    /// Execute the request
    pub async fn execute(self) -> Result<(), Error> {
        let LinkDelPropRequest {
            mut handle,
            message,
        } = self;
        let mut req = NetlinkMessage::from(RtnlMessage::DelLinkProp(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK | NLM_F_EXCL;

        let mut response = handle.request(req)?;
        while let Some(message) = response.next().await {
            if let NetlinkPayload::Error(err) = message.payload {
                return Err(Error::NetlinkError(err));
            }
        }
        Ok(())
    }

    /// Return a mutable reference to the request
    pub fn message_mut(&mut self) -> &mut LinkMessage {
        &mut self.message
    }

    /// Remove alternative name to the link. This is equivalent to `ip link property del altname
    /// ALT_IFNAME dev LINK`.
    pub fn alt_ifname(mut self, alt_ifnames: &[&str]) -> Self {
        let mut props = Vec::new();
        for alt_ifname in alt_ifnames {
            props.push(Prop::AltIfName(alt_ifname.to_string()));
        }

        self.message.nlas.push(Nla::PropList(props));
        self
    }
}
