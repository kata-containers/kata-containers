use futures::stream::StreamExt;

use netlink_packet_route::{
    constants::*,
    neighbour::{NeighbourMessage, Nla},
    NetlinkPayload,
    RtnlMessage,
};

use netlink_proto::packet::NetlinkMessage;

use crate::{Error, Handle};
use std::net::IpAddr;

pub struct NeighbourAddRequest {
    handle: Handle,
    message: NeighbourMessage,
}

impl NeighbourAddRequest {
    pub(crate) fn new(handle: Handle, index: u32, destination: IpAddr) -> Self {
        let mut message = NeighbourMessage::default();

        message.header.family = match destination {
            IpAddr::V4(_) => AF_INET as u8,
            IpAddr::V6(_) => AF_INET6 as u8,
        };

        message.header.ifindex = index;
        message.header.state = IFA_F_PERMANENT as u16;
        message.header.ntype = NDA_UNSPEC as u8;

        message.nlas.push(Nla::Destination(match destination {
            IpAddr::V4(v4) => v4.octets().to_vec(),
            IpAddr::V6(v6) => v6.octets().to_vec(),
        }));

        NeighbourAddRequest { handle, message }
    }

    /// Set a bitmask of states for the neighbor cache entry.
    /// It should be a combination of `NUD_*` constants.
    pub fn state(mut self, state: u16) -> Self {
        self.message.header.state = state;
        self
    }

    /// Set flags for the neighbor cache entry.
    /// It should be a combination of `NTF_*` constants.
    pub fn flags(mut self, flags: u8) -> Self {
        self.message.header.flags = flags;
        self
    }

    /// Set attributes applicable to the the neighbor cache entry.
    /// It should be one of `NDA_*` constants.
    pub fn ntype(mut self, ntype: u8) -> Self {
        self.message.header.ntype = ntype;
        self
    }

    /// Set a neighbor cache link layer address (see `NDA_LLADDR` for details).
    pub fn link_local_address(mut self, addr: &[u8]) -> Self {
        self.message.nlas.push(Nla::LinkLocalAddress(addr.to_vec()));
        self
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        let NeighbourAddRequest {
            mut handle,
            message,
        } = self;

        let mut req = NetlinkMessage::from(RtnlMessage::NewNeighbour(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK | NLM_F_EXCL | NLM_F_CREATE;

        let mut response = handle.request(req)?;
        while let Some(message) = response.next().await {
            if let NetlinkPayload::Error(err) = message.payload {
                return Err(Error::NetlinkError(err));
            }
        }

        Ok(())
    }

    /// Return a mutable reference to the request message.
    pub fn message_mut(&mut self) -> &mut NeighbourMessage {
        &mut self.message
    }
}
