// SPDX-License-Identifier: MIT

use futures::stream::StreamExt;
use std::net::{IpAddr, Ipv4Addr};

use netlink_packet_route::{
    nlas::address::Nla,
    AddressMessage,
    NetlinkMessage,
    RtnlMessage,
    AF_INET,
    AF_INET6,
    NLM_F_ACK,
    NLM_F_CREATE,
    NLM_F_EXCL,
    NLM_F_REPLACE,
    NLM_F_REQUEST,
};

use crate::{try_nl, Error, Handle};

/// A request to create a new address. This is equivalent to the `ip address add` commands.
pub struct AddressAddRequest {
    handle: Handle,
    message: AddressMessage,
    replace: bool,
}

impl AddressAddRequest {
    pub(crate) fn new(handle: Handle, index: u32, address: IpAddr, prefix_len: u8) -> Self {
        let mut message = AddressMessage::default();

        message.header.prefix_len = prefix_len;
        message.header.index = index;

        let address_vec = match address {
            IpAddr::V4(ipv4) => {
                message.header.family = AF_INET as u8;
                ipv4.octets().to_vec()
            }
            IpAddr::V6(ipv6) => {
                message.header.family = AF_INET6 as u8;
                ipv6.octets().to_vec()
            }
        };

        if address.is_multicast() {
            message.nlas.push(Nla::Multicast(address_vec));
        } else if address.is_unspecified() {
            message.nlas.push(Nla::Unspec(address_vec));
        } else if address.is_ipv6() {
            message.nlas.push(Nla::Address(address_vec));
        } else {
            message.nlas.push(Nla::Address(address_vec.clone()));

            // for IPv4 the IFA_LOCAL address can be set to the same value as IFA_ADDRESS
            message.nlas.push(Nla::Local(address_vec.clone()));

            // set the IFA_BROADCAST address as well (IPv6 does not support broadcast)
            if prefix_len == 32 {
                message.nlas.push(Nla::Broadcast(address_vec));
            } else {
                let ip_addr: u32 = u32::from(Ipv4Addr::new(
                    address_vec[0],
                    address_vec[1],
                    address_vec[2],
                    address_vec[3],
                ));
                let brd = Ipv4Addr::from((0xffff_ffff_u32) >> u32::from(prefix_len) | ip_addr);
                message.nlas.push(Nla::Broadcast(brd.octets().to_vec()));
            };
        }
        AddressAddRequest {
            handle,
            message,
            replace: false,
        }
    }

    /// Replace existing matching address.
    pub fn replace(self) -> Self {
        Self {
            replace: true,
            ..self
        }
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        let AddressAddRequest {
            mut handle,
            message,
            replace,
        } = self;
        let mut req = NetlinkMessage::from(RtnlMessage::NewAddress(message));
        let replace = if replace { NLM_F_REPLACE } else { NLM_F_EXCL };
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK | replace | NLM_F_CREATE;

        let mut response = handle.request(req)?;
        while let Some(message) = response.next().await {
            try_nl!(message);
        }
        Ok(())
    }

    /// Return a mutable reference to the request message.
    pub fn message_mut(&mut self) -> &mut AddressMessage {
        &mut self.message
    }
}
