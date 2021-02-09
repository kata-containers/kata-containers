// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
pub const IPV6_HOPOPTS: ::c_int = 1;
pub const IPV6_HDRINCL: ::c_int = 2;
pub const IPV6_UNICAST_HOPS: ::c_int = 4;
pub const IPV6_MULTICAST_IF: ::c_int = 9;
pub const IPV6_MULTICAST_HOPS: ::c_int = 10;
pub const IPV6_MULTICAST_LOOP: ::c_int = 11;
pub const IPV6_ADD_MEMBERSHIP: ::c_int = 12;
pub const IPV6_JOIN_GROUP: ::c_int = IPV6_ADD_MEMBERSHIP;
pub const IPV6_DROP_MEMBERSHIP: ::c_int = 13;
pub const IPV6_LEAVE_GROUP: ::c_int = IPV6_DROP_MEMBERSHIP;
pub const IPV6_DONTFRAG: ::c_int = 14;
pub const IPV6_PKTINFO: ::c_int = 19;
pub const IPV6_HOPLIMIT: ::c_int = 21;
pub const IPV6_PROTECTION_LEVEL: ::c_int = 23;
pub const IPV6_RECVIF: ::c_int = 24;
pub const IPV6_RECVDSTADDR: ::c_int = 25;
pub const IPV6_CHECKSUM: ::c_int = 26;
pub const IPV6_V6ONLY: ::c_int = 27;
pub const IPV6_IFLIST: ::c_int = 28;
pub const IPV6_ADD_IFLIST: ::c_int = 29;
pub const IPV6_DEL_IFLIST: ::c_int = 30;
pub const IPV6_UNICAST_IF: ::c_int = 31;
pub const IPV6_RTHDR: ::c_int = 32;
pub const IPV6_RECVRTHDR: ::c_int = 38;
pub const IPV6_TCLASS: ::c_int = 39;
pub const IPV6_RECVTCLASS: ::c_int = 40;
STRUCT!{struct ipv6_mreq {
    ipv6mr_multiaddr: in6_addr,
    ipv6mr_interface: ::c_uint,
}}
STRUCT!{struct in6_addr {
    s6_addr: [u8; 16],
}}
STRUCT!{struct sockaddr_in6 {
    sin6_family: ::c_short,
    sin6_port: ::c_ushort,
    sin6_flowinfo: ::c_ulong,
    sin6_addr: in6_addr,
    sin6_scope_id: ::c_ulong,
}}
