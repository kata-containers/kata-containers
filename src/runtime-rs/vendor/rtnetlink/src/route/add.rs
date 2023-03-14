// SPDX-License-Identifier: MIT

use futures::stream::StreamExt;
use std::{
    marker::PhantomData,
    net::{Ipv4Addr, Ipv6Addr},
};

use netlink_packet_route::{
    constants::*,
    nlas::route::Nla,
    NetlinkMessage,
    RouteMessage,
    RtnlMessage,
};

use crate::{try_nl, Error, Handle};

/// A request to create a new route. This is equivalent to the `ip route add` commands.
pub struct RouteAddRequest<T = ()> {
    handle: Handle,
    message: RouteMessage,
    replace: bool,
    _phantom: PhantomData<T>,
}

impl<T> RouteAddRequest<T> {
    pub(crate) fn new(handle: Handle) -> Self {
        let mut message = RouteMessage::default();

        message.header.table = RT_TABLE_MAIN;
        message.header.protocol = RTPROT_STATIC;
        message.header.scope = RT_SCOPE_UNIVERSE;
        message.header.kind = RTN_UNICAST;

        RouteAddRequest {
            handle,
            message,
            replace: false,
            _phantom: Default::default(),
        }
    }

    /// Sets the input interface index.
    pub fn input_interface(mut self, index: u32) -> Self {
        self.message.nlas.push(Nla::Iif(index));
        self
    }

    /// Sets the output interface index.
    pub fn output_interface(mut self, index: u32) -> Self {
        self.message.nlas.push(Nla::Oif(index));
        self
    }

    /// Sets the route table.
    ///
    /// Default is main route table.
    pub fn table(mut self, table: u8) -> Self {
        self.message.header.table = table;
        self
    }

    /// Sets the route protocol.
    ///
    /// Default is static route protocol.
    pub fn protocol(mut self, protocol: u8) -> Self {
        self.message.header.protocol = protocol;
        self
    }

    /// Sets the route scope.
    ///
    /// Default is universe route scope.
    pub fn scope(mut self, scope: u8) -> Self {
        self.message.header.scope = scope;
        self
    }

    /// Sets the route kind.
    ///
    /// Default is unicast route kind.
    pub fn kind(mut self, kind: u8) -> Self {
        self.message.header.kind = kind;
        self
    }

    /// Build an IP v4 route request
    pub fn v4(mut self) -> RouteAddRequest<Ipv4Addr> {
        self.message.header.address_family = AF_INET as u8;
        RouteAddRequest {
            handle: self.handle,
            message: self.message,
            replace: false,
            _phantom: Default::default(),
        }
    }

    /// Build an IP v6 route request
    pub fn v6(mut self) -> RouteAddRequest<Ipv6Addr> {
        self.message.header.address_family = AF_INET6 as u8;
        RouteAddRequest {
            handle: self.handle,
            message: self.message,
            replace: false,
            _phantom: Default::default(),
        }
    }

    /// Replace existing matching route.
    pub fn replace(self) -> Self {
        Self {
            replace: true,
            ..self
        }
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        let RouteAddRequest {
            mut handle,
            message,
            replace,
            ..
        } = self;
        let mut req = NetlinkMessage::from(RtnlMessage::NewRoute(message));
        let replace = if replace { NLM_F_REPLACE } else { NLM_F_EXCL };
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK | replace | NLM_F_CREATE;

        let mut response = handle.request(req)?;
        while let Some(message) = response.next().await {
            try_nl!(message);
        }
        Ok(())
    }

    /// Return a mutable reference to the request message.
    pub fn message_mut(&mut self) -> &mut RouteMessage {
        &mut self.message
    }
}

impl RouteAddRequest<Ipv4Addr> {
    /// Sets the source address prefix.
    pub fn source_prefix(mut self, addr: Ipv4Addr, prefix_length: u8) -> Self {
        self.message.header.source_prefix_length = prefix_length;
        let src = addr.octets().to_vec();
        self.message.nlas.push(Nla::Source(src));
        self
    }

    /// Sets the preferred source address.
    pub fn pref_source(mut self, addr: Ipv4Addr) -> Self {
        let src = addr.octets().to_vec();
        self.message.nlas.push(Nla::PrefSource(src));
        self
    }

    /// Sets the destination address prefix.
    pub fn destination_prefix(mut self, addr: Ipv4Addr, prefix_length: u8) -> Self {
        self.message.header.destination_prefix_length = prefix_length;
        let dst = addr.octets().to_vec();
        self.message.nlas.push(Nla::Destination(dst));
        self
    }

    /// Sets the gateway (via) address.
    pub fn gateway(mut self, addr: Ipv4Addr) -> Self {
        let gtw = addr.octets().to_vec();
        self.message.nlas.push(Nla::Gateway(gtw));
        self
    }
}

impl RouteAddRequest<Ipv6Addr> {
    /// Sets the source address prefix.
    pub fn source_prefix(mut self, addr: Ipv6Addr, prefix_length: u8) -> Self {
        self.message.header.source_prefix_length = prefix_length;
        let src = addr.octets().to_vec();
        self.message.nlas.push(Nla::Source(src));
        self
    }

    /// Sets the preferred source address.
    pub fn pref_source(mut self, addr: Ipv6Addr) -> Self {
        let src = addr.octets().to_vec();
        self.message.nlas.push(Nla::PrefSource(src));
        self
    }

    /// Sets the destination address prefix.
    pub fn destination_prefix(mut self, addr: Ipv6Addr, prefix_length: u8) -> Self {
        self.message.header.destination_prefix_length = prefix_length;
        let dst = addr.octets().to_vec();
        self.message.nlas.push(Nla::Destination(dst));
        self
    }

    /// Sets the gateway (via) address.
    pub fn gateway(mut self, addr: Ipv6Addr) -> Self {
        let gtw = addr.octets().to_vec();
        self.message.nlas.push(Nla::Gateway(gtw));
        self
    }
}
