// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! WinSock2 Extension for TCP/IP protocols
pub type LPLOOKUPSERVICE_COMPLETION_ROUTINE = Option<unsafe extern "system" fn(
    dwError: ::DWORD, dwBytes: ::DWORD, lpOverlapped: ::LPWSAOVERLAPPED,
)>;
pub type socklen_t = ::c_int;
STRUCT!{struct ip_mreq {
    imr_multiaddr: ::in_addr,
    imr_interface: ::in_addr,
}}
pub const IP_OPTIONS: ::c_int = 1;
pub const IP_HDRINCL: ::c_int = 2;
pub const IP_TOS: ::c_int = 3;
pub const IP_TTL: ::c_int = 4;
pub const IP_MULTICAST_IF: ::c_int = 9;
pub const IP_MULTICAST_TTL: ::c_int = 10;
pub const IP_MULTICAST_LOOP: ::c_int = 11;
pub const IP_ADD_MEMBERSHIP: ::c_int = 12;
pub const IP_DROP_MEMBERSHIP: ::c_int = 13;
pub const IP_DONTFRAGMENT: ::c_int = 14;
pub const IP_ADD_SOURCE_MEMBERSHIP: ::c_int = 15;
pub const IP_DROP_SOURCE_MEMBERSHIP: ::c_int = 16;
pub const IP_BLOCK_SOURCE: ::c_int = 17;
pub const IP_UNBLOCK_SOURCE: ::c_int = 18;
pub const IP_PKTINFO: ::c_int = 19;
pub const IP_RECEIVE_BROADCAST: ::c_int = 22;
