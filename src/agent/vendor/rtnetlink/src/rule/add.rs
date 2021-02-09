use futures::stream::StreamExt;
use std::net::{Ipv4Addr, Ipv6Addr};

use netlink_packet_route::{
    constants::*,
    nlas::rule::Nla,
    NetlinkMessage,
    NetlinkPayload,
    RtnlMessage,
    RuleMessage,
};

use crate::{Error, Handle};

/// A request to create a new rule. This is equivalent to the `ip rule add` command.
struct RuleAddRequest {
    handle: Handle,
    message: RuleMessage,
}

impl RuleAddRequest {
    fn new(handle: Handle) -> Self {
        let mut message = RuleMessage::default();

        message.header.table = RT_TABLE_MAIN;
        message.header.action = FR_ACT_UNSPEC;

        RuleAddRequest { handle, message }
    }

    /// Sets the input interface name.
    fn input_interface(mut self, ifname: String) -> Self {
        self.message.nlas.push(Nla::Iifname(ifname));
        self
    }

    /// Sets the output interface name.
    fn output_interface(mut self, ifname: String) -> Self {
        self.message.nlas.push(Nla::OifName(ifname));
        self
    }

    /// Sets the rule table.
    ///
    /// Default is main rule table.
    fn table(mut self, table: u8) -> Self {
        self.message.header.table = table;
        self
    }

    /// Set the tos.
    fn tos(mut self, tos: u8) -> Self {
        self.message.header.tos = tos;
        self
    }

    /// Set action.
    fn action(mut self, action: u8) -> Self {
        self.message.header.action = action;
        self
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        let RuleAddRequest {
            mut handle,
            message,
        } = self;
        let mut req = NetlinkMessage::from(RtnlMessage::NewRule(message));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK | NLM_F_EXCL | NLM_F_CREATE;

        let mut response = handle.request(req)?;
        while let Some(message) = response.next().await {
            if let NetlinkPayload::Error(err) = message.payload {
                return Err(Error::NetlinkError(err));
            }
        }
        Ok(())
    }

    fn message_mut(&mut self) -> &mut RuleMessage {
        &mut self.message
    }
}

pub struct RuleAddIpv4Request(RuleAddRequest);

impl RuleAddIpv4Request {
    pub fn new(handle: Handle) -> Self {
        let mut req = RuleAddRequest::new(handle);
        req.message_mut().header.family = AF_INET as u8;
        Self(req)
    }

    /// Sets the input interface name.
    pub fn input_interface(self, ifname: String) -> Self {
        Self(self.0.input_interface(ifname))
    }

    /// Sets the output interface name.
    pub fn output_interface(self, ifname: String) -> Self {
        Self(self.0.output_interface(ifname))
    }

    /// Sets the source address prefix.
    pub fn source_prefix(mut self, addr: Ipv4Addr, prefix_length: u8) -> Self {
        self.0.message.header.src_len = prefix_length;
        self.0
            .message
            .nlas
            .push(Nla::Source(addr.octets().to_vec()));
        self
    }

    /// Sets the destination address prefix.
    pub fn destination_prefix(mut self, addr: Ipv4Addr, prefix_length: u8) -> Self {
        self.0.message.header.dst_len = prefix_length;
        self.0
            .message
            .nlas
            .push(Nla::Destination(addr.octets().to_vec()));
        self
    }

    /// Sets the rule table.
    ///
    /// Default is main rule table.
    pub fn table(self, table: u8) -> Self {
        Self(self.0.table(table))
    }

    /// Set the tos.
    pub fn tos(self, tos: u8) -> Self {
        Self(self.0.tos(tos))
    }

    /// Set action.
    pub fn action(self, action: u8) -> Self {
        Self(self.0.action(action))
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        self.0.execute().await
    }

    pub fn message_mut(&mut self) -> &mut RuleMessage {
        self.0.message_mut()
    }
}

pub struct RuleAddIpv6Request(RuleAddRequest);

impl RuleAddIpv6Request {
    pub fn new(handle: Handle) -> Self {
        let mut req = RuleAddRequest::new(handle);
        req.message_mut().header.family = AF_INET6 as u8;
        Self(req)
    }

    /// Sets the input interface name.
    pub fn input_interface(self, ifname: String) -> Self {
        Self(self.0.input_interface(ifname))
    }

    /// Sets the output interface name.
    pub fn output_interface(self, ifname: String) -> Self {
        Self(self.0.output_interface(ifname))
    }

    /// Sets the source address prefix.
    pub fn source_prefix(mut self, addr: Ipv6Addr, prefix_length: u8) -> Self {
        self.0.message.header.src_len = prefix_length;
        self.0
            .message
            .nlas
            .push(Nla::Source(addr.octets().to_vec()));
        self
    }

    /// Sets the destination address prefix.
    pub fn destination_prefix(mut self, addr: Ipv6Addr, prefix_length: u8) -> Self {
        self.0.message.header.dst_len = prefix_length;
        self.0
            .message
            .nlas
            .push(Nla::Destination(addr.octets().to_vec()));
        self
    }

    /// Sets the rule table.
    ///
    /// Default is main rule table.
    pub fn table(self, table: u8) -> Self {
        Self(self.0.table(table))
    }

    /// Set the tos.
    pub fn tos(self, tos: u8) -> Self {
        Self(self.0.tos(tos))
    }

    /// Set action.
    pub fn action(self, action: u8) -> Self {
        Self(self.0.action(action))
    }

    /// Execute the request.
    pub async fn execute(self) -> Result<(), Error> {
        self.0.execute().await
    }

    pub fn message_mut(&mut self) -> &mut RuleMessage {
        self.0.message_mut()
    }
}
