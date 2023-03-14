// SPDX-License-Identifier: MIT

use crate::{
    constants::*,
    traits::{Emitable, Parseable},
    DecodeError,
    RouteMessageBuffer,
    ROUTE_HEADER_LEN,
};

bitflags! {
    /// Flags that can be set in a `RTM_GETROUTE` ([`RtnlMessage::GetRoute`]) message.
    pub struct RouteFlags: u32 {
        /// If the route changes, notify the user via rtnetlink
        const RTM_F_NOTIFY = RTM_F_NOTIFY;
        /// This route is cloned. Cloned routes are routes coming from the cache instead of the
        /// FIB. For IPv4, the cache was removed in Linux 3.6 (see [IPv4 route lookup on Linux] for
        /// more information about IPv4 routing)
        ///
        /// [IPv4 route lookup on Linux]: https://vincent.bernat.ch/en/blog/2017-ipv4-route-lookup-linux
        const RTM_F_CLONED = RTM_F_CLONED;
        /// Multipath equalizer (not yet implemented)
        const RTM_F_EQUALIZE = RTM_F_EQUALIZE;
        /// Prefix addresses
        const RTM_F_PREFIX = RTM_F_PREFIX;
        /// Show the table from which the lookup result comes. Note that before commit
        /// `c36ba6603a11`, Linux would always hardcode [`RouteMessageHeader.table`] (known as
        /// `rtmsg.rtm_table` in the kernel) to `RT_TABLE_MAIN`.
        ///
        /// [`RouteMessageHeader.table`]: ../struct.RouteMessageHeader.html#structfield.table
        const RTM_F_LOOKUP_TABLE = RTM_F_LOOKUP_TABLE;
        /// Return the full FIB lookup match (see commit `b61798130f1be5bff08712308126c2d7ebe390ef`)
        const RTM_F_FIB_MATCH = RTM_F_FIB_MATCH;
    }
}

impl Default for RouteFlags {
    fn default() -> Self {
        Self::empty()
    }
}

/// High level representation of `RTM_GETROUTE`, `RTM_ADDROUTE`, `RTM_DELROUTE`
/// messages headers.
///
/// These headers have the following structure:
///
/// ```no_rust
/// 0                8                16              24               32
/// +----------------+----------------+----------------+----------------+
/// | address family | dest. length   | source length  |      tos       |
/// +----------------+----------------+----------------+----------------+
/// |     table      |   protocol     |      scope     | type (kind)    |
/// +----------------+----------------+----------------+----------------+
/// |                               flags                               |
/// +----------------+----------------+----------------+----------------+
/// ```
///
/// # Example
///
/// ```rust
/// extern crate netlink_packet_route;
/// use netlink_packet_route::{constants::*, RouteFlags, RouteHeader};
///
/// fn main() {
///     let mut hdr = RouteHeader::default();
///     assert_eq!(hdr.address_family, 0u8);
///     assert_eq!(hdr.destination_prefix_length, 0u8);
///     assert_eq!(hdr.source_prefix_length, 0u8);
///     assert_eq!(hdr.tos, 0u8);
///     assert_eq!(hdr.table, RT_TABLE_UNSPEC);
///     assert_eq!(hdr.protocol, RTPROT_UNSPEC);
///     assert_eq!(hdr.scope, RT_SCOPE_UNIVERSE);
///     assert_eq!(hdr.kind, RTN_UNSPEC);
///     assert_eq!(hdr.flags.bits(), 0u32);
///
///     // set some values
///     hdr.destination_prefix_length = 8;
///     hdr.table = RT_TABLE_MAIN;
///     hdr.protocol = RTPROT_KERNEL;
///     hdr.scope = RT_SCOPE_NOWHERE;
///
///     // ...
/// }
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Default)]
pub struct RouteHeader {
    /// Address family of the route: either [`AF_INET`] for IPv4 prefixes, or [`AF_INET6`] for IPv6
    /// prefixes.
    pub address_family: u8,
    /// Prefix length of the destination subnet.
    ///
    /// Note that setting
    pub destination_prefix_length: u8,
    /// Prefix length of the source address.
    ///
    /// There could be multiple addresses from which a certain network is reachable. To decide which
    /// source address to use to reach and address in that network, the kernel rely on the route's
    /// source address for this destination.
    ///
    /// For instance, interface `if1` could have two addresses `10.0.0.1/24` and `10.0.0.128/24`,
    /// and we could have the following routes:
    ///
    /// ```no_rust
    /// 10.0.0.10/32 dev if1 scope link src 10.0.0.1
    /// 10.0.0.11/32 dev if1 scope link src 10.0.0.1
    /// 10.0.0.12/32 dev if1 scope link src 10.0.0.1
    /// 10.0.0.0/24 dev if1 scope link src 10.0.0.128
    /// ```
    ///
    /// It means that for `10.0.0.10`, `10.0.0.11` and `10.0.0.12` the packets will be sent with
    /// `10.0.0.1` as source address, while for the rest of the `10.0.0.0/24` subnet, the source
    /// address will be `10.0.0.128`
    pub source_prefix_length: u8,
    /// TOS filter
    pub tos: u8,
    /// Routing table ID. It can be one of the `RT_TABLE_*` constants or a custom table number
    /// between 1 and 251 (included). Note that Linux supports routing table with an ID greater than
    /// 255, in which case this attribute will be set to [`RT_TABLE_COMPAT`] and an [`Nla::Table`]
    /// netlink attribute will be present in the message.
    pub table: u8,
    /// Protocol from which the route was learnt. It should be set to one of the `RTPROT_*`
    /// constants.
    pub protocol: u8,
    /// The scope of the area where the addresses in the destination subnet are valid. Predefined
    /// scope values are the `RT_SCOPE_*` constants.
    pub scope: u8,
    /// Route type. It should be set to one of the `RTN_*` constants.
    pub kind: u8,
    /// Flags when querying the kernel with a `RTM_GETROUTE` message. See [`RouteFlags`].
    pub flags: RouteFlags,
}

impl<'a, T: AsRef<[u8]> + ?Sized> Parseable<RouteMessageBuffer<&'a T>> for RouteHeader {
    fn parse(buf: &RouteMessageBuffer<&'a T>) -> Result<Self, DecodeError> {
        Ok(RouteHeader {
            address_family: buf.address_family(),
            destination_prefix_length: buf.destination_prefix_length(),
            source_prefix_length: buf.source_prefix_length(),
            tos: buf.tos(),
            table: buf.table(),
            protocol: buf.protocol(),
            scope: buf.scope(),
            kind: buf.kind(),
            flags: RouteFlags::from_bits_truncate(buf.flags()),
        })
    }
}

impl Emitable for RouteHeader {
    fn buffer_len(&self) -> usize {
        ROUTE_HEADER_LEN
    }

    fn emit(&self, buffer: &mut [u8]) {
        let mut buffer = RouteMessageBuffer::new(buffer);
        buffer.set_address_family(self.address_family);
        buffer.set_destination_prefix_length(self.destination_prefix_length);
        buffer.set_source_prefix_length(self.source_prefix_length);
        buffer.set_tos(self.tos);
        buffer.set_table(self.table);
        buffer.set_protocol(self.protocol);
        buffer.set_scope(self.scope);
        buffer.set_kind(self.kind);
        buffer.set_flags(self.flags.bits());
    }
}
