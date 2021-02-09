use futures::stream::StreamExt;
use std::net::{Ipv4Addr, Ipv6Addr};

use netlink_packet_route::{
    constants::*,
    nlas::route::Nla,
    NetlinkMessage,
    NetlinkPayload,
    RouteMessage,
    RtnlMessage,
};

use crate::{Error, Handle};

/// A request to create a new route. This is equivalent to the `ip route add` commands.
struct RouteAddRequest {
    handle: Handle,
    message: RouteMessage,
}

impl RouteAddRequest {
    fn new(handle: Handle) -> Self {
        let mut message = RouteMessage::default();

        message.header.table = RT_TABLE_MAIN;
        message.header.protocol = RTPROT_STATIC;
        message.header.scope = RT_SCOPE_UNIVERSE;
        message.header.kind = RTN_UNICAST;

        RouteAddRequest { handle, message }
    }

    /// Sets the input interface index.
    fn input_interface(mut self, index: u32) -> Self {
        self.message.nlas.push(Nla::Iif(index));
        self
    }

    /// Sets the output interface index.
    fn output_interface(mut self, index: u32) -> Self {
        self.message.nlas.push(Nla::Oif(index));
        self
    }

    /// Sets the route table.
    ///
    /// Default is main route table.
    fn table(mut self, table: u8) -> Self {
        self.message.header.table = table;
        self
    }

    /// Sets the route protocol.
    ///
    /// Default is static route protocol.
    fn protocol(mut self, protocol: u8) -> Self {
        self.message.header.protocol = protocol;
        self
    }

    /// Sets the route scope.
    ///
    /// Default is universe route scope.
    fn scope(mut self, scope: u8) -> Self {
        self.message.header.scope = scope;
        self
    }

    /// Sets the route kind.
    ///
    /// Default is unicast route kind.
    fn kind(mut self, kind: u8) -> Self {
        self.message.header.kind = kind;
        self
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        let RouteAddRequest {
            mut handle,
            message,
        } = self;
        let mut req = NetlinkMessage::from(RtnlMessage::NewRoute(message));
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
    fn message_mut(&mut self) -> &mut RouteMessage {
        &mut self.message
    }
}

pub struct RouteAddIpv4Request(RouteAddRequest);

impl RouteAddIpv4Request {
    pub fn new(handle: Handle) -> Self {
        let mut req = RouteAddRequest::new(handle);
        req.message_mut().header.address_family = AF_INET as u8;
        Self(req)
    }

    /// Sets the input interface index.
    pub fn input_interface(self, index: u32) -> Self {
        Self(self.0.input_interface(index))
    }

    /// Sets the output interface index.
    pub fn output_interface(self, index: u32) -> Self {
        Self(self.0.output_interface(index))
    }

    /// Sets the source address prefix.
    pub fn source_prefix(mut self, addr: Ipv4Addr, prefix_length: u8) -> Self {
        self.0.message.header.source_prefix_length = prefix_length;
        self.0
            .message
            .nlas
            .push(Nla::Source(addr.octets().to_vec()));
        self
    }

    /// Sets the destination address prefix.
    pub fn destination_prefix(mut self, addr: Ipv4Addr, prefix_length: u8) -> Self {
        self.0.message.header.destination_prefix_length = prefix_length;
        self.0
            .message
            .nlas
            .push(Nla::Destination(addr.octets().to_vec()));
        self
    }

    /// Sets the gateway (via) address.
    pub fn gateway(mut self, addr: Ipv4Addr) -> Self {
        self.0
            .message
            .nlas
            .push(Nla::Gateway(addr.octets().to_vec()));
        self
    }

    /// Sets the route table.
    ///
    /// Default is main route table.
    pub fn table(self, table: u8) -> Self {
        Self(self.0.table(table))
    }

    /// Sets the route protocol.
    ///
    /// Default is static route protocol.
    pub fn protocol(self, protocol: u8) -> Self {
        Self(self.0.protocol(protocol))
    }

    /// Sets the route scope.
    ///
    /// Default is universe route scope.
    pub fn scope(self, scope: u8) -> Self {
        Self(self.0.scope(scope))
    }

    /// Sets the route kind.
    ///
    /// Default is unicast route kind.
    pub fn kind(self, kind: u8) -> Self {
        Self(self.0.kind(kind))
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        self.0.execute().await
    }

    /// Return a mutable reference to the request message.
    pub fn message_mut(&mut self) -> &mut RouteMessage {
        self.0.message_mut()
    }
}

pub struct RouteAddIpv6Request(RouteAddRequest);

impl RouteAddIpv6Request {
    pub fn new(handle: Handle) -> Self {
        let mut req = RouteAddRequest::new(handle);
        req.message_mut().header.address_family = AF_INET6 as u8;
        Self(req)
    }

    /// Sets the input interface index.
    pub fn input_interface(self, index: u32) -> Self {
        Self(self.0.input_interface(index))
    }

    /// Sets the output interface index.
    pub fn output_interface(self, index: u32) -> Self {
        Self(self.0.output_interface(index))
    }

    /// Sets the source address prefix.
    pub fn source_prefix(mut self, addr: Ipv6Addr, prefix_length: u8) -> Self {
        self.0.message.header.source_prefix_length = prefix_length;
        self.0
            .message
            .nlas
            .push(Nla::Source(addr.octets().to_vec()));
        self
    }

    /// Sets the destination address prefix.
    pub fn destination_prefix(mut self, addr: Ipv6Addr, prefix_length: u8) -> Self {
        self.0.message.header.destination_prefix_length = prefix_length;
        self.0
            .message
            .nlas
            .push(Nla::Destination(addr.octets().to_vec()));
        self
    }

    /// Sets the gateway (via) address.
    pub fn gateway(mut self, addr: Ipv6Addr) -> Self {
        self.0
            .message
            .nlas
            .push(Nla::Gateway(addr.octets().to_vec()));
        self
    }

    /// Sets the route table.
    ///
    /// Default is main route table.
    pub fn table(self, table: u8) -> Self {
        Self(self.0.table(table))
    }

    /// Sets the route protocol.
    ///
    /// Default is static route protocol.
    pub fn protocol(self, protocol: u8) -> Self {
        Self(self.0.protocol(protocol))
    }

    /// Sets the route scope.
    ///
    /// Default is universe route scope.
    pub fn scope(self, scope: u8) -> Self {
        Self(self.0.scope(scope))
    }

    /// Sets the route kind.
    ///
    /// Default is unicast route kind.
    pub fn kind(self, kind: u8) -> Self {
        Self(self.0.kind(kind))
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        self.0.execute().await
    }

    /// Return a mutable reference to the request message.
    pub fn message_mut(&mut self) -> &mut RouteMessage {
        self.0.message_mut()
    }
}
