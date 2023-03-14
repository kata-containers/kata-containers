use futures::stream::StreamExt;

use crate::{
    packet::{
        LinkMessage,
        NetlinkMessage,
        RtnlMessage,
        NLM_F_ACK,
        NLM_F_CREATE,
        NLM_F_EXCL,
        NLM_F_REQUEST,
    },
    try_nl,
    Error,
    Handle,
};

pub struct LinkDelRequest {
    handle: Handle,
    message: LinkMessage,
}

impl LinkDelRequest {
    pub(crate) fn new(handle: Handle, index: u32) -> Self {
        let mut message = LinkMessage::default();
        message.header.index = index;
        LinkDelRequest { handle, message }
    }

    /// Execute the request
    pub async fn execute(self) -> Result<(), Error> {
        let LinkDelRequest {
            mut handle,
            message,
        } = self;
        let mut req = NetlinkMessage::from(RtnlMessage::DelLink(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK | NLM_F_EXCL | NLM_F_CREATE;

        let mut response = handle.request(req)?;
        while let Some(message) = response.next().await {
            try_nl!(message)
        }
        Ok(())
    }

    /// Return a mutable reference to the request
    pub fn message_mut(&mut self) -> &mut LinkMessage {
        &mut self.message
    }
}
