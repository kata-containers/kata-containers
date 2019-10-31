// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]

use libc;
use nix::errno::Errno;
use protobuf::RepeatedField;
use protocols::types::{IPAddress, IPFamily, Interface, Route};
use rustjail::errors::*;
use std::clone::Clone;
use std::default::Default;
use std::fmt;
use std::mem;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

// Convenience macro to obtain the scope logger
macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "netlink"))
    };
}

// define the struct, const, etc needed by
// netlink operations

pub type __s8 = libc::c_char;
pub type __u8 = libc::c_uchar;
pub type __s16 = libc::c_short;
pub type __u16 = libc::c_ushort;
pub type __s32 = libc::c_int;
pub type __u32 = libc::c_uint;
pub type __s64 = libc::c_longlong;
pub type __u64 = libc::c_ulonglong;

// we need ifaddrmsg, ifinfomasg, rtmsg
// we need some constant

pub const RTM_BASE: libc::c_ushort = 16;
pub const RTM_NEWLINK: libc::c_ushort = 16;
pub const RTM_DELLINK: libc::c_ushort = 17;
pub const RTM_GETLINK: libc::c_ushort = 18;
pub const RTM_SETLINK: libc::c_ushort = 19;
pub const RTM_NEWADDR: libc::c_ushort = 20;
pub const RTM_DELADDR: libc::c_ushort = 21;
pub const RTM_GETADDR: libc::c_ushort = 22;
pub const RTM_NEWROUTE: libc::c_ushort = 24;
pub const RTM_DELROUTE: libc::c_ushort = 25;
pub const RTM_GETROUTE: libc::c_ushort = 26;
pub const RTM_NEWNEIGH: libc::c_ushort = 28;
pub const RTM_DELNEIGH: libc::c_ushort = 29;
pub const RTM_GETNEIGH: libc::c_ushort = 30;
pub const RTM_NEWRULE: libc::c_ushort = 32;
pub const RTM_DELRULE: libc::c_ushort = 33;
pub const RTM_GETRULE: libc::c_ushort = 34;
pub const RTM_NEWQDISC: libc::c_ushort = 36;
pub const RTM_DELQDISC: libc::c_ushort = 37;
pub const RTM_GETQDISC: libc::c_ushort = 38;
pub const RTM_NEWTCLASS: libc::c_ushort = 40;
pub const RTM_DELTCLASS: libc::c_ushort = 41;
pub const RTM_GETTCLASS: libc::c_ushort = 42;
pub const RTM_NEWTFILTER: libc::c_ushort = 44;
pub const RTM_DELTFILTER: libc::c_ushort = 45;
pub const RTM_GETTFILTER: libc::c_ushort = 46;
pub const RTM_NEWACTION: libc::c_ushort = 48;
pub const RTM_DELACTION: libc::c_ushort = 49;
pub const RTM_GETACTION: libc::c_ushort = 50;
pub const RTM_NEWPREFIX: libc::c_ushort = 52;
pub const RTM_GETMULTICAST: libc::c_ushort = 58;
pub const RTM_GETANYCAST: libc::c_ushort = 62;
pub const RTM_NEWNEIGHTBL: libc::c_ushort = 64;
pub const RTM_GETNEIGHTBL: libc::c_ushort = 66;
pub const RTM_SETNEIGHTBL: libc::c_ushort = 67;
pub const RTM_NEWNDUSEROPT: libc::c_ushort = 68;
pub const RTM_NEWADDRLABEL: libc::c_ushort = 72;
pub const RTM_DELADDRLABEL: libc::c_ushort = 73;
pub const RTM_GETADDRLABEL: libc::c_ushort = 74;
pub const RTM_GETDCB: libc::c_ushort = 78;
pub const RTM_SETDCB: libc::c_ushort = 79;
pub const RTM_NEWNETCONF: libc::c_ushort = 80;
pub const RTM_GETNETCONF: libc::c_ushort = 82;
pub const RTM_NEWMDB: libc::c_ushort = 84;
pub const RTM_DELMDB: libc::c_ushort = 85;
pub const RTM_GETMDB: libc::c_ushort = 86;
pub const RTM_NEWNSID: libc::c_ushort = 88;
pub const RTM_DELNSID: libc::c_ushort = 89;
pub const RTM_GETNSID: libc::c_ushort = 90;
pub const RTM_NEWSTATS: libc::c_ushort = 92;
pub const RTM_GETSTATS: libc::c_ushort = 94;
pub const RTM_NEWCACHEREPORT: libc::c_ushort = 96;
pub const RTM_NEWCHAIN: libc::c_ushort = 100;
pub const RTM_DELCHAIN: libc::c_ushort = 101;
pub const RTM_GETCHAIN: libc::c_ushort = 102;
pub const __RTM_MAX: libc::c_ushort = 103;

pub const RTM_MAX: libc::c_ushort = (((__RTM_MAX + 3) & !3) - 1);
pub const RTM_NR_MSGTYPES: libc::c_ushort = (RTM_MAX + 1) - RTM_BASE;
pub const RTM_NR_FAMILIES: libc::c_ushort = RTM_NR_MSGTYPES >> 2;

#[macro_export]
macro_rules! RTM_FAM {
    ($cmd: expr) => {
        ($cmd - RTM_BASE) >> 2
    };
}

#[repr(C)]
#[derive(Copy)]
pub struct rtattr {
    rta_len: libc::c_ushort,
    rta_type: libc::c_ushort,
}

impl Clone for rtattr {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for rtattr {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

#[repr(C)]
#[derive(Copy)]
pub struct rtmsg {
    rtm_family: libc::c_uchar,
    rtm_dst_len: libc::c_uchar,
    rtm_src_len: libc::c_uchar,
    rtm_tos: libc::c_uchar,
    rtm_table: libc::c_uchar,
    rtm_protocol: libc::c_uchar,
    rtm_scope: libc::c_uchar,
    rtm_type: libc::c_uchar,
    rtm_flags: libc::c_uint,
}

impl Clone for rtmsg {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for rtmsg {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

// rtm_type c_uchar
pub const RTN_UNSPEC: libc::c_uchar = 0;
pub const RTN_UNICAST: libc::c_uchar = 1;
pub const RTN_LOCAL: libc::c_uchar = 2;
pub const RTN_BROADCAST: libc::c_uchar = 3;
pub const RTN_ANYCAST: libc::c_uchar = 4;
pub const RTN_MULTICAST: libc::c_uchar = 5;
pub const RTN_BLACKHOLE: libc::c_uchar = 6;
pub const RTN_UNREACHABLE: libc::c_uchar = 7;
pub const RTN_PROHIBIT: libc::c_uchar = 8;
pub const RTN_THROW: libc::c_uchar = 9;
pub const RTN_NAT: libc::c_uchar = 10;
pub const RTN_XRESOLVE: libc::c_uchar = 11;
pub const __RTN_MAX: libc::c_uchar = 12;
pub const RTN_MAX: libc::c_uchar = __RTN_MAX - 1;

// rtm_protocol c_uchar
pub const RTPROTO_UNSPEC: libc::c_uchar = 0;
pub const RTPROTO_REDIRECT: libc::c_uchar = 1;
pub const RTPROTO_KERNEL: libc::c_uchar = 2;
pub const RTPROTO_BOOT: libc::c_uchar = 3;
pub const RTPROTO_STATIC: libc::c_uchar = 4;

pub const RTPROTO_GATED: libc::c_uchar = 8;
pub const RTPROTO_RA: libc::c_uchar = 9;
pub const RTPROTO_MRT: libc::c_uchar = 10;
pub const RTPROTO_ZEBRA: libc::c_uchar = 11;
pub const RTPROTO_BIRD: libc::c_uchar = 12;
pub const RTPROTO_DNROUTED: libc::c_uchar = 13;
pub const RTPROTO_XORP: libc::c_uchar = 14;
pub const RTPROTO_NTK: libc::c_uchar = 15;
pub const RTPROTO_DHCP: libc::c_uchar = 16;
pub const RTPROTO_MROUTED: libc::c_uchar = 17;
pub const RTPROTO_BABEL: libc::c_uchar = 42;
pub const RTPROTO_BGP: libc::c_uchar = 186;
pub const RTPROTO_ISIS: libc::c_uchar = 187;
pub const RTPROTO_OSPF: libc::c_uchar = 188;
pub const RTPROTO_RIP: libc::c_uchar = 189;
pub const RTPROTO_EIGRP: libc::c_uchar = 192;

//rtm_scope c_uchar
pub const RT_SCOPE_UNIVERSE: libc::c_uchar = 0;
pub const RT_SCOPE_SITE: libc::c_uchar = 200;
pub const RT_SCOPE_LINK: libc::c_uchar = 253;
pub const RT_SCOPE_HOST: libc::c_uchar = 254;
pub const RT_SCOPE_NOWHERE: libc::c_uchar = 255;

// rtm_flags c_uint
pub const RTM_F_NOTIFY: libc::c_uint = 0x100;
pub const RTM_F_CLONED: libc::c_uint = 0x200;
pub const RTM_F_EQUALIZE: libc::c_uint = 0x400;
pub const RTM_F_PREFIX: libc::c_uint = 0x800;
pub const RTM_F_LOOKUP_TABLE: libc::c_uint = 0x1000;
pub const RTM_F_FIB_MATCH: libc::c_uint = 0x2000;

// table identifier
pub const RT_TABLE_UNSPEC: libc::c_uint = 0;
pub const RT_TABLE_COMPAT: libc::c_uint = 252;
pub const RT_TABLE_DEFAULT: libc::c_uint = 253;
pub const RT_TABLE_MAIN: libc::c_uint = 254;
pub const RT_TABLE_LOCAL: libc::c_uint = 255;
pub const RT_TABLE_MAX: libc::c_uint = 0xffffffff;

// rat_type c_ushort
pub const RTA_UNSPEC: libc::c_ushort = 0;
pub const RTA_DST: libc::c_ushort = 1;
pub const RTA_SRC: libc::c_ushort = 2;
pub const RTA_IIF: libc::c_ushort = 3;
pub const RTA_OIF: libc::c_ushort = 4;
pub const RTA_GATEWAY: libc::c_ushort = 5;
pub const RTA_PRIORITY: libc::c_ushort = 6;
pub const RTA_PREFSRC: libc::c_ushort = 7;
pub const RTA_METRICS: libc::c_ushort = 8;
pub const RTA_MULTIPATH: libc::c_ushort = 9;
pub const RTA_PROTOINFO: libc::c_ushort = 10;
pub const RTA_FLOW: libc::c_ushort = 11;
pub const RTA_CACHEINFO: libc::c_ushort = 12;
pub const RTA_SESSION: libc::c_ushort = 13;
pub const RTA_MP_ALGO: libc::c_ushort = 14;
pub const RTA_TABLE: libc::c_ushort = 15;
pub const RTA_MARK: libc::c_ushort = 16;
pub const RTA_MFC_STATS: libc::c_ushort = 17;
pub const RTA_VIA: libc::c_ushort = 18;
pub const RTA_NEWDST: libc::c_ushort = 19;
pub const RTA_PREF: libc::c_ushort = 20;
pub const RTA_ENCAP_TYPE: libc::c_ushort = 21;
pub const RTA_ENCAP: libc::c_ushort = 22;
pub const RTA_EXPIRES: libc::c_ushort = 23;
pub const RTA_PAD: libc::c_ushort = 24;
pub const RTA_UID: libc::c_ushort = 25;
pub const RTA_TTL_PROPAGATE: libc::c_ushort = 26;
pub const RTA_IP_PROTO: libc::c_ushort = 27;
pub const RTA_SPORT: libc::c_ushort = 28;
pub const RTA_DPORT: libc::c_ushort = 29;
pub const __RTA_MAX: libc::c_ushort = 30;
pub const RTA_MAX: libc::c_ushort = __RTA_MAX - 1;

#[macro_export]
macro_rules! RTM_RTA {
    ($rtm: expr) => {
        unsafe {
            let mut p = $rtm as *mut rtmsg as i64;
            p += NLMSG_ALIGN!(mem::size_of::<rtmsg>()) as i64;
            p as *mut rtattr
        }
    };
}

#[macro_export]
macro_rules! RTM_PAYLOAD {
    ($h: expr) => {
        NLMSG_PAYLOAD!($h, mem::size_of::<rtmsg>())
    };
}

// RTA_MULTIPATH
#[repr(C)]
#[derive(Copy)]
pub struct rtnexthop {
    rtnh_len: libc::c_ushort,
    rtnh_flags: libc::c_uchar,
    rtnh_hops: libc::c_uchar,
    rtnh_ifindex: libc::c_int,
}

impl Clone for rtnexthop {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for rtnexthop {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

// rtnh_flags
pub const RTNH_F_DEAD: libc::c_uchar = 1;
pub const RTNH_F_PERVASIVE: libc::c_uchar = 2;
pub const RTNH_F_ONLINK: libc::c_uchar = 4;
pub const RTNH_F_OFFLOAD: libc::c_uchar = 8;
pub const RTNH_F_LINKDOWN: libc::c_uchar = 16;
pub const RTNH_F_UNRESOLVED: libc::c_uchar = 32;

pub const RTNH_COMPARE_MASK: libc::c_uchar = RTNH_F_DEAD | RTNH_F_LINKDOWN | RTNH_F_OFFLOAD;

pub const RTNH_ALIGN: i32 = 4;
#[macro_export]
macro_rules! RTNH_ALIGN {
    ($len: expr) => {
        (($len as u32 + (RTNH_ALIGN - 1) as u32) & !(RTNH_ALIGN - 1) as u32)
    };
}

#[macro_export]
macro_rules! RTNH_OK {
    ($rtnh: expr, $len: expr) => {
        $rtnh.rtnh_len >= mem::size_of::<rtnexthop>() && $rtnh.rtnh_len <= $len
    };
}

#[macro_export]
macro_rules! RTNH_NEXT {
    ($rtnh: expr) => {
        unsafe {
            let mut p = $rtnh as *mut rtnexthop as i64;
            p += RTNH_ALIGN!($rtnh.rtnh_len);
            p as *mut rtnexthop
        }
    };
}

#[macro_export]
macro_rules! RTNH_LENGTH {
    ($len: expr) => {
        RTNH_ALIGN!(mem::size_of::<rtnexthop>()) + $len
    };
}

#[macro_export]
macro_rules! RTNH_SPACE {
    ($len: expr) => {
        RTNH_ALIGN!(RTNH_LENGTH!($len))
    };
}

#[macro_export]
macro_rules! RTNH_DATA {
    ($rtnh: expr) => {
        unsafe {
            let mut p = $rtnh as *mut rtnexthop as i64;
            p += RTNH_LENGTH!(0);
            p as *mut rtattr
        }
    };
}

// RTA_VIA
type __kernel_sa_family_t = libc::c_ushort;
#[repr(C)]
#[derive(Copy)]
pub struct rtvia {
    rtvia_family: __kernel_sa_family_t,
    // array with size 0. omitted here. be careful
    // with how to access it. cannot use rtvia.rtvia_addr
}

impl Clone for rtvia {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for rtvia {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

// rta_cacheinfo
#[repr(C)]
#[derive(Copy)]
pub struct rta_cacheinfo {
    rta_clntref: __u32,
    rta_lastuse: __u32,
    rta_expires: __u32,
    rta_error: __u32,
    rta_used: __u32,

    rta_id: __u32,
    rta_ts: __u32,
    rta_tsage: __u32,
}

impl Clone for rta_cacheinfo {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for rta_cacheinfo {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

// RTA_METRICS
pub const RTAX_UNSPEC: libc::c_ushort = 0;
pub const RTAX_LOCK: libc::c_ushort = 1;
pub const RTAX_MTU: libc::c_ushort = 2;
pub const RTAX_WINDOW: libc::c_ushort = 3;
pub const RTAX_RTT: libc::c_ushort = 4;
pub const RTAX_RTTVAR: libc::c_ushort = 5;
pub const RTAX_SSTHRESH: libc::c_ushort = 6;
pub const RTAX_CWND: libc::c_ushort = 7;
pub const RTAX_ADVMSS: libc::c_ushort = 8;
pub const RTAX_REORDERING: libc::c_ushort = 9;
pub const RTAX_HOPLIMIT: libc::c_ushort = 10;
pub const RTAX_INITCWND: libc::c_ushort = 11;
pub const RTAX_FEATURES: libc::c_ushort = 12;
pub const RTAX_RTO_MIN: libc::c_ushort = 13;
pub const RTAX_INITRWND: libc::c_ushort = 14;
pub const RTAX_QUICKACK: libc::c_ushort = 15;
pub const RTAX_CC_ALGO: libc::c_ushort = 16;
pub const RTAX_FASTOPEN_NO_COOKIE: libc::c_ushort = 17;
pub const __RTAX_MAX: libc::c_ushort = 18;

pub const RTAX_MAX: libc::c_ushort = __RTAX_MAX - 1;
pub const RTAX_FEATURE_ECN: libc::c_ushort = 1 << 0;
pub const RTAX_FEATURE_SACK: libc::c_ushort = 1 << 1;
pub const RTAX_FEATURE_TIMESTAMP: libc::c_ushort = 1 << 2;
pub const RTAX_FEATURE_ALLFRAG: libc::c_ushort = 1 << 3;
pub const RTAX_FEATURE_MASK: libc::c_ushort =
    RTAX_FEATURE_ECN | RTAX_FEATURE_SACK | RTAX_FEATURE_TIMESTAMP | RTAX_FEATURE_ALLFRAG;

// RTA_SESSION
#[repr(C)]
#[derive(Copy)]
pub struct Ports {
    sport: __u16,
    dport: __u16,
}

impl Clone for Ports {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for Ports {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

#[repr(C)]
#[derive(Copy)]
pub struct Icmpt {
    r#type: __u8,
    code: __u8,
    ident: __u16,
}

impl Clone for Icmpt {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for Icmpt {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

#[repr(C)]
#[derive(Copy)]
pub union U {
    pub ports: Ports,
    pub icmpt: Icmpt,
    spi: __u32,
}

impl Clone for U {
    fn clone(&self) -> Self {
        Self {
            spi: unsafe { self.spi },
        }
    }
}

impl Default for U {
    fn default() -> Self {
        let s = unsafe { mem::zeroed::<Self>() };
        Self {
            spi: unsafe { s.spi },
        }
    }
}

#[repr(C)]
#[derive(Copy)]
pub struct rta_session {
    proto: __u8,
    pad1: __u8,
    pad2: __u16,
    u: U,
}

impl Clone for rta_session {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for rta_session {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

#[repr(C)]
#[derive(Copy)]
pub struct rta_mfc_stats {
    mfcs_packets: __u64,
    mfcs_bytes: __u64,
    mfcs_wrong_if: __u64,
}

impl Clone for rta_mfc_stats {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for rta_mfc_stats {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

#[repr(C)]
#[derive(Copy)]
pub struct ifinfomsg {
    ifi_family: libc::c_uchar,
    __ifi_pad: libc::c_uchar,
    ifi_type: libc::c_ushort,
    ifi_index: libc::c_int,
    ifi_flags: libc::c_uint,
    ifi_change: libc::c_uint,
}

impl Clone for ifinfomsg {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for ifinfomsg {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

#[repr(C)]
#[derive(Copy)]
pub struct rtnl_link_stats64 {
    rx_packets: __u64,
    tx_packets: __u64,
    rx_bytes: __u64,
    tx_bytes: __u64,
    rx_errors: __u64,
    tx_errors: __u64,
    rx_dropped: __u64,
    tx_dropped: __u64,
    multicast: __u64,
    collisions: __u64,

    // detailed rx_errors
    rx_length_errors: __u64,
    rx_over_errors: __u64,
    rx_crc_errors: __u64,
    rx_frame_errrors: __u64,
    rx_fifo_errors: __u64,
    rx_missed_errors: __u64,

    // detailed tx_errors
    tx_aborted_errors: __u64,
    tx_carrier_errors: __u64,
    tx_fifo_errors: __u64,
    tx_heartbeat_errors: __u64,
    tx_window_errors: __u64,

    rx_compressed: __u64,
    tx_compressed: __u64,
    rx_nohandler: __u64,
}

impl Clone for rtnl_link_stats64 {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for rtnl_link_stats64 {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

#[repr(C)]
#[derive(Copy)]
pub struct rtnl_link_stats {
    rx_packets: __u32,
    tx_packets: __u32,
    rx_bytes: __u32,
    tx_bytes: __u32,
    rx_errors: __u32,
    tx_errors: __u32,
    rx_dropped: __u32,
    tx_dropped: __u32,
    multicast: __u32,
    collisions: __u32,

    // detailed rx_errors
    rx_length_errors: __u32,
    rx_over_errors: __u32,
    rx_crc_errors: __u32,
    rx_frame_errrors: __u32,
    rx_fifo_errors: __u32,
    rx_missed_errors: __u32,

    // detailed tx_errors
    tx_aborted_errors: __u32,
    tx_carrier_errors: __u32,
    tx_fifo_errors: __u32,
    tx_heartbeat_errors: __u32,
    tx_window_errors: __u32,

    rx_compressed: __u32,
    tx_compressed: __u32,
    rx_nohandler: __u32,
}

impl Clone for rtnl_link_stats {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for rtnl_link_stats {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

#[repr(C)]
#[derive(Copy)]
pub struct ifaddrmsg {
    ifa_family: __u8,
    ifa_prefixlen: __u8,
    ifa_flags: __u8,
    ifa_scope: __u8,
    ifa_index: __u32,
}

impl Clone for ifaddrmsg {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for ifaddrmsg {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

#[repr(C)]
#[derive(Copy)]
pub struct ifa_cacheinfo {
    ifa_prefered: __u32,
    ifa_valid: __u32,
    cstamp: __u32,
    tstamp: __u32,
}

impl Clone for ifa_cacheinfo {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for ifa_cacheinfo {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

pub const RTA_ALIGNTO: libc::c_uint = 4;
#[macro_export]
macro_rules! RTA_ALIGN {
    ($x: expr) => {
        ($x as u32 + (RTA_ALIGNTO - 1) as u32) & !((RTA_ALIGNTO - 1) as u32)
    };
}

#[macro_export]
macro_rules! RTA_OK {
    ($attr: expr, $len: expr) => {
        ($len as u32 >= mem::size_of::<rtattr>() as u32)
            && ((*$attr).rta_len as u32 >= mem::size_of::<rtattr>() as u32)
            && ((*$attr).rta_len as u32 <= $len as u32)
    };
}

#[macro_export]
macro_rules! RTA_NEXT {
    ($attr: expr, $len: expr) => {
        unsafe {
            $len -= RTA_ALIGN!((*$attr).rta_len) as u32;
            let mut p = $attr as *mut libc::c_char as i64;
            p += RTA_ALIGN!((*$attr).rta_len) as i64;
            p as *mut rtattr
        }
    };
}

#[macro_export]
macro_rules! RTA_LENGTH {
    ($len: expr) => {
        RTA_ALIGN!($len as u32 + mem::size_of::<rtattr>() as u32)
    };
}

#[macro_export]
macro_rules! RTA_SPACE {
    ($len: expr) => {
        RTA_ALIGN!(RTA_LENGTH!($len))
    };
}

#[macro_export]
macro_rules! RTA_DATA {
    ($attr: expr) => {
        unsafe {
            let mut p = $attr as *mut libc::c_char as i64;
            p += RTA_LENGTH!(0) as i64;
            p as *mut libc::c_char
        }
    };
}

#[macro_export]
macro_rules! RTA_PAYLOAD {
    ($attr: expr) => {
        ((*$attr).rta_len as u32 - RTA_LENGTH!(0) as u32)
    };
}

pub const NLMSGERR_ATTR_UNUSED: libc::c_uchar = 0;
pub const NLMSGERR_ATTR_MASG: libc::c_uchar = 1;
pub const NLMSGERR_ATTR_OFFS: libc::c_uchar = 2;
pub const NLMSGERR_ATTR_COOKIE: libc::c_uchar = 3;
pub const __NLMSGERR_ATTR_MAX: libc::c_uchar = 4;
pub const NLMSGERR_ATTR_MAX: libc::c_uchar = __NLMSGERR_ATTR_MAX - 1;

pub const NLMSG_ALIGNTO: libc::c_uint = 4;
#[macro_export]
macro_rules! NLMSG_ALIGN {
    ($len: expr) => {
        ($len as u32 + NLMSG_ALIGNTO - 1) & !(NLMSG_ALIGNTO - 1)
    };
}

// weird, static link cannot find libc::nlmsghdr
// define macro here ro work around it for now
// till someone can find out the reason
// pub const NLMSG_HDRLEN: libc::c_int = NLMSG_ALIGN!(mem::size_of::<libc::nlmsghdr>() as libc::c_uint) as libc::c_int;

#[macro_export]
macro_rules! NLMSG_HDRLEN {
    () => {
        NLMSG_ALIGN!(mem::size_of::<nlmsghdr>())
    };
}

#[macro_export]
macro_rules! NLMSG_LENGTH {
    ($len: expr) => {
        ($len as u32 + NLMSG_HDRLEN!())
    };
}

#[macro_export]
macro_rules! NLMSG_SPACE {
    ($len: expr) => {
        NLMSG_ALIGN!(NLMSG_LENGTH!($len))
    };
}

#[macro_export]
macro_rules! NLMSG_DATA {
    ($nlh: expr) => {
        unsafe {
            let mut p = $nlh as *mut nlmsghdr as i64;
            p += NLMSG_LENGTH!(0) as i64;
            p as *mut libc::c_void
        }
    };
}

#[macro_export]
macro_rules! NLMSG_NEXT {
    ($nlh: expr, $len: expr) => {
        unsafe {
            $len -= NLMSG_ALIGN!((*$nlh).nlmsg_len) as u32;
            let mut p = $nlh as *mut libc::c_char;
            p = (p as i64 + NLMSG_ALIGN!((*$nlh).nlmsg_len) as i64) as *mut libc::c_char;
            p as *mut nlmsghdr
        }
    };
}

#[macro_export]
macro_rules! NLMSG_OK {
    ($nlh: expr, $len: expr) => {
        $len as usize >= mem::size_of::<nlmsghdr>()
            && (*$nlh).nlmsg_len as usize >= mem::size_of::<nlmsghdr>()
            && (*$nlh).nlmsg_len as usize <= $len as usize
    };
}

#[macro_export]
macro_rules! NLMSG_PAYLOAD {
    ($nlh: expr, $len: expr) => {
        ((*$nlh).nlmsg_len - NLMSG_SPACE!($len))
    };
}

#[macro_export]
macro_rules! RTA_TAIL {
	($attr: expr) => {
		unsafe {
			let mut p = $attr as *mut rtattr as i64;
			p += RTA_ALIGN!($attr->rta_len) as i64;
			p as *mut rtattr
		}
	}
}

#[macro_export]
macro_rules! NLMSG_TAIL {
    ($msg: expr) => {
        unsafe {
            let mut p = $msg as *mut nlmsghdr as i64;
            p += NLMSG_ALIGN!((*$msg).nlmsg_len) as i64;
            p as *mut rtattr
        }
    };
}

#[macro_export]
macro_rules! IFA_RTA {
    ($ifmsg: expr) => {
        unsafe {
            let mut p = $ifmsg as *mut ifaddrmsg as *mut libc::c_char;
            p = (p as i64 + NLMSG_ALIGN!(mem::size_of::<ifaddrmsg>()) as i64) as *mut libc::c_char;
            p as *mut rtattr
        }
    };
}

#[macro_export]
macro_rules! IFA_PAYLOAD {
    ($h: expr) => {
        NLMSG_PAYLOAD!($h, mem::size_of::<ifaddrmsg>())
    };
}

#[macro_export]
macro_rules! IFLA_RTA {
    ($ifinfo: expr) => {
        unsafe {
            let mut p = $ifinfo as *mut ifinfomsg as i64;
            p += NLMSG_ALIGN!(mem::size_of::<ifinfomsg>()) as i64;
            p as *mut rtattr
        }
    };
}

#[macro_export]
macro_rules! IFLA_PAYLOAD {
    ($h: expr) => {
        (NLMSG_PAYLOAD!($h, mem::size_of::<ifinfomsg>()))
    };
}

#[macro_export]
macro_rules! IFLA_STATS_RTA {
    ($stats: expr) => {
        unsafe {
            let mut p = $stats as *mut if_stats_msg as i64;
            p += NLMSG_ALIGN!(mem::size_of::<if_stats_msg>()) as i64;
            p as *mut rtattr
        }
    };
}

#[repr(C)]
#[derive(Copy)]
pub struct nlmsghdr {
    pub nlmsg_len: __u32,
    pub nlmsg_type: __u16,
    pub nlmsg_flags: __u16,
    pub nlmsg_seq: __u32,
    pub nlmsg_pid: __u32,
}

impl Clone for nlmsghdr {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for nlmsghdr {
    fn default() -> Self {
        unsafe { mem::zeroed::<Self>() }
    }
}

// nlmsg_flags
pub const NLM_F_REQUEST: __u16 = 0x01;
pub const NLM_F_MULTI: __u16 = 0x02;
pub const NLM_F_ACK: __u16 = 0x04;
pub const NLM_F_ECHO: __u16 = 0x08;
pub const NLM_F_DUMP_INTR: __u16 = 0x10;
pub const NLM_F_DUMP_FILTERED: __u16 = 0x20;

// Get Request
pub const NLM_F_ROOT: __u16 = 0x100;
pub const NLM_F_MATCH: __u16 = 0x200;
pub const NLM_F_ATOMIC: __u16 = 0x400;
pub const NLM_F_DUMP: __u16 = NLM_F_ROOT | NLM_F_MATCH;

// New Request
pub const NLM_F_REPLACE: __u16 = 0x100;
pub const NLM_F_EXCL: __u16 = 0x200;
pub const NLM_F_CREATE: __u16 = 0x400;
pub const NLM_F_APPEND: __u16 = 0x800;

// Delete Request
pub const NLM_F_NONREC: __u16 = 0x100;

//ACK message
pub const NLM_F_CAPPED: __u16 = 0x100;
pub const NLM_F_ACK_TLVS: __u16 = 0x200;

// error message type
pub const NLMSG_NOOP: __u16 = 0x1;
pub const NLMSG_ERROR: __u16 = 0x2;
pub const NLMSG_DONE: __u16 = 0x3;
pub const NLMSG_OVERRUN: __u16 = 0x4;

pub const NLMSG_MIN_TYPE: __u16 = 0x10;

// IFLA_EXT_MASK
pub const RTEXT_FILTER_VF: __u32 = 1 << 0;
pub const RTEXT_FILTER_BRVLAN: __u32 = 1 << 1;
pub const RTEXT_FILTER_BRVLAN_COMPRESSED: __u32 = 1 << 2;
pub const RTEXT_FILTER_SKIP_STATS: __u32 = 1 << 3;

// IFLA attr
pub const IFLA_UNSPEC: __u16 = 0;
pub const IFLA_ADDRESS: __u16 = 1;
pub const IFLA_BROADCAST: __u16 = 2;
pub const IFLA_IFNAME: __u16 = 3;
pub const IFLA_MTU: __u16 = 4;
pub const IFLA_LINK: __u16 = 5;
pub const IFLA_QDISC: __u16 = 6;
pub const IFLA_STATS: __u16 = 7;
pub const IFLA_COST: __u16 = 8;
pub const IFLA_PRIORITY: __u16 = 9;
pub const IFLA_MASTER: __u16 = 10;
pub const IFLA_WIRELESS: __u16 = 11;
pub const IFLA_PROTINFO: __u16 = 12;
pub const IFLA_TXQLEN: __u16 = 13;
pub const IFLA_MAP: __u16 = 14;
pub const IFLA_WEIGHT: __u16 = 15;
pub const IFLA_OPERSTATE: __u16 = 16;
pub const IFLA_LINKMODE: __u16 = 17;
pub const IFLA_LINKINFO: __u16 = 18;
pub const IFLA_NET_NS_PID: __u16 = 19;
pub const IFLA_IFALIAS: __u16 = 20;
pub const IFLA_NUM_VF: __u16 = 21;
pub const IFLA_VFINFO_LIST: __u16 = 22;
pub const IFLA_STATS64: __u16 = 23;
pub const IFLA_VF_PORTS: __u16 = 24;
pub const IFLA_PORT_SELF: __u16 = 25;
pub const IFLA_AF_SPEC: __u16 = 26;
pub const IFLA_GROUP: __u16 = 27;
pub const IFLA_NET_NS_FD: __u16 = 28;
pub const IFLA_EXT_MASK: __u16 = 29;
pub const IFLA_PROMISCUITY: __u16 = 30;
pub const IFLA_NUM_TX_QUEUES: __u16 = 31;
pub const IFLA_NUM_RX_QUEUES: __u16 = 32;
pub const IFLA_CARRIER: __u16 = 33;
pub const IFLA_PHYS_PORT_ID: __u16 = 34;
pub const IFLA_CARRIER_CHANGES: __u16 = 35;
pub const IFLA_PHYS_SWITCH_ID: __u16 = 36;
pub const IFLA_LINK_NETNSID: __u16 = 37;
pub const IFLA_PHYS_PORT_NAME: __u16 = 38;
pub const IFLA_PROTO_DOWN: __u16 = 39;
pub const IFLA_GSO_MAX_SEGS: __u16 = 40;
pub const IFLA_GSO_MAX_SIZE: __u16 = 41;
pub const IFLA_PAD: __u16 = 42;
pub const IFLA_XDP: __u16 = 43;
pub const IFLA_EVENT: __u16 = 44;
pub const IFLA_NEW_NETNSID: __u16 = 45;
pub const IFLA_IF_NETNSID: __u16 = 46;
pub const IFLA_CARRIER_UP_COUNT: __u16 = 47;
pub const IFLA_CARRIER_DOWN_COUNT: __u16 = 48;
pub const IFLA_NEW_IFINDEX: __u16 = 49;
pub const IFLA_MIN_MTU: __u16 = 50;
pub const IFLA_MAX_MTU: __u16 = 51;
pub const __IFLA_MAX: __u16 = 52;
pub const IFLA_MAX: __u16 = __IFLA_MAX - 1;

pub const IFA_UNSPEC: __u16 = 0;
pub const IFA_ADDRESS: __u16 = 1;
pub const IFA_LOCAL: __u16 = 2;
pub const IFA_LABEL: __u16 = 3;
pub const IFA_BROADCAST: __u16 = 4;
pub const IFA_ANYCAST: __u16 = 5;
pub const IFA_CACHEINFO: __u16 = 6;
pub const IFA_MULTICAST: __u16 = 7;
pub const IFA_FLAGS: __u16 = 8;
pub const IFA_RT_PRIORITY: __u16 = 9;
pub const __IFA_MAX: __u16 = 10;
pub const IFA_MAX: __u16 = __IFA_MAX - 1;

// ifa_flags
pub const IFA_F_SECONDARY: __u32 = 0x01;
pub const IFA_F_TEMPORARY: __u32 = IFA_F_SECONDARY;
pub const IFA_F_NODAD: __u32 = 0x02;
pub const IFA_F_OPTIMISTIC: __u32 = 0x04;
pub const IFA_F_DADFAILED: __u32 = 0x08;
pub const IFA_F_HOMEADDRESS: __u32 = 0x10;
pub const IFA_F_DEPRECATED: __u32 = 0x20;
pub const IFA_F_TENTATIVE: __u32 = 0x40;
pub const IFA_F_PERMANENT: __u32 = 0x80;
pub const IFA_F_MANAGETEMPADDR: __u32 = 0x100;
pub const IFA_F_NOPREFIXROUTE: __u32 = 0x200;
pub const IFA_F_MCAUTOJOIN: __u32 = 0x400;
pub const IFA_F_STABLE_PRIVACY: __u32 = 0x800;

#[repr(C)]
#[derive(Copy)]
pub struct nlmsgerr {
    pub error: libc::c_int,
    pub msg: nlmsghdr,
}

impl Clone for nlmsgerr {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for nlmsgerr {
    fn default() -> Self {
        unsafe { mem::zeroed::<nlmsgerr>() }
    }
}

// #[derive(Copy)]
pub struct RtnlHandle {
    pub fd: libc::c_int,
    local: libc::sockaddr_nl,
    seq: __u32,
    dump: __u32,
}

impl Clone for RtnlHandle {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl Default for RtnlHandle {
    fn default() -> Self {
        Self {
            ..unsafe { mem::zeroed::<Self>() }
        }
    }
}

impl fmt::Debug for RtnlHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fd: {}\nadrr:{{pid: {}, family: {}}}\nseq:{}\ndump:{}",
            self.fd, self.local.nl_family, self.local.nl_pid, self.seq, self.dump
        )
    }
}

pub const NETLINK_ROUTE: libc::c_int = 0;
pub const NETLINK_EXT_ACK: libc::c_int = 11;
pub const NETLINK_UEVENT: libc::c_int = 15;

impl RtnlHandle {
    pub fn new(protocal: libc::c_int, group: u32) -> Result<Self> {
        // open netlink_route socket
        let mut sa: libc::sockaddr_nl = unsafe { mem::zeroed::<libc::sockaddr_nl>() };
        let fd = unsafe {
            let tmpfd = libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
                protocal,
            );

            let sndbuf: libc::c_int = 32768;
            let rcvbuf: libc::c_int = 1024 * 1024;
            let one: libc::c_int = 1;
            let mut addrlen: libc::socklen_t =
                mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;

            if tmpfd < 0 {
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::last())).into());
            }

            let mut err = libc::setsockopt(
                tmpfd,
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                &sndbuf as *const libc::c_int as *const libc::c_void,
                mem::size_of::<libc::c_int>() as libc::socklen_t,
            );

            if err < 0 {
                libc::close(tmpfd);
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::last())).into());
            }

            err = libc::setsockopt(
                tmpfd,
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                &rcvbuf as *const libc::c_int as *const libc::c_void,
                mem::size_of::<libc::c_int>() as libc::socklen_t,
            );

            if err < 0 {
                libc::close(tmpfd);
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::last())).into());
            }

            libc::setsockopt(
                tmpfd,
                libc::SOL_NETLINK,
                NETLINK_EXT_ACK,
                &one as *const libc::c_int as *const libc::c_void,
                mem::size_of::<libc::c_int>() as libc::socklen_t,
            );

            sa.nl_family = libc::AF_NETLINK as __u16;
            sa.nl_groups = group;

            err = libc::bind(
                tmpfd,
                (&sa as *const libc::sockaddr_nl) as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            );
            if err < 0 {
                libc::close(tmpfd);
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::last())).into());
            }

            err = libc::getsockname(
                tmpfd,
                &mut sa as *mut libc::sockaddr_nl as *mut libc::sockaddr,
                &mut addrlen as *mut libc::socklen_t,
            );
            if err < 0 {
                libc::close(tmpfd);
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::last())).into());
            }

            if sa.nl_family as i32 != libc::AF_NETLINK
                || addrlen as usize != mem::size_of::<libc::sockaddr_nl>()
            {
                libc::close(tmpfd);
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::EINVAL)).into());
            }

            tmpfd
        };

        Ok(Self {
            fd,
            local: sa,
            seq: unsafe { libc::time(0 as *mut libc::time_t) } as __u32,
            dump: 0,
        })
    }

    // implement update{interface,routes}, list{interface, routes}
    fn send_message(&self, data: &mut [u8]) -> Result<()> {
        let mut sa: libc::sockaddr_nl = unsafe { mem::zeroed::<libc::sockaddr_nl>() };

        sa.nl_family = libc::AF_NETLINK as u16;

        unsafe {
            let nh = data.as_mut_ptr() as *mut nlmsghdr;
            let mut iov: libc::iovec = libc::iovec {
                iov_base: nh as *mut libc::c_void,
                iov_len: (*nh).nlmsg_len as libc::size_t,
            };

            let mut h = mem::zeroed::<libc::msghdr>();
            h.msg_name = &mut sa as *mut libc::sockaddr_nl as *mut libc::c_void;
            h.msg_namelen = mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
            h.msg_iov = &mut iov as *mut libc::iovec;
            h.msg_iovlen = 1;

            let err = libc::sendmsg(self.fd, &h as *const libc::msghdr, 0);

            if err < 0 {
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::last())).into());
            }
        }
        Ok(())
    }

    pub fn recv_message(&self) -> Result<Vec<u8>> {
        let mut sa: libc::sockaddr_nl = unsafe { mem::zeroed::<libc::sockaddr_nl>() };

        let mut iov = libc::iovec {
            iov_base: 0 as *mut libc::c_void,
            iov_len: 0 as libc::size_t,
        };

        unsafe {
            let mut h = mem::zeroed::<libc::msghdr>();
            h.msg_name = &mut sa as *mut libc::sockaddr_nl as *mut libc::c_void;
            h.msg_namelen = mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
            h.msg_iov = &mut iov as *mut libc::iovec;
            h.msg_iovlen = 1;

            let mut rlen = libc::recvmsg(
                self.fd,
                &mut h as *mut libc::msghdr,
                libc::MSG_PEEK | libc::MSG_TRUNC,
            );

            if rlen < 0 {
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::last())).into());
            }

            // if rlen < 32768 {
            //	rlen = 32768;
            // }

            let mut v: Vec<u8> = vec![0; rlen as usize];
            // v.set_len(rlen as usize);

            iov.iov_base = v.as_mut_ptr() as *mut libc::c_void;
            iov.iov_len = rlen as libc::size_t;

            rlen = libc::recvmsg(self.fd, &mut h as *mut libc::msghdr, 0);
            if rlen < 0 {
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::last())).into());
            }

            if sa.nl_pid != 0 {
                // not our netlink message
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::EBADMSG)).into());
            }

            if h.msg_flags & libc::MSG_TRUNC != 0 {
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::EBADMSG)).into());
            }

            v.resize(rlen as usize, 0);

            Ok(v)
        }
    }

    unsafe fn recv_dump_message(&self) -> Result<(Vec<Vec<u8>>, Vec<*const nlmsghdr>)> {
        let mut slv: Vec<Vec<u8>> = Vec::new();
        let mut lv: Vec<*const nlmsghdr> = Vec::new();

        loop {
            let buf = self.recv_message()?;

            let mut msglen = buf.len() as u32;
            let mut nlh = buf.as_ptr() as *const nlmsghdr;
            let mut dump_intr = 0;
            let mut done = 0;

            while NLMSG_OK!(nlh, msglen) {
                if (*nlh).nlmsg_pid != self.local.nl_pid || (*nlh).nlmsg_seq != self.dump {
                    nlh = NLMSG_NEXT!(nlh, msglen);
                    continue;
                }

                // got one nlmsg
                if (*nlh).nlmsg_flags & NLM_F_DUMP_INTR > 0 {
                    dump_intr = 1;
                }

                if (*nlh).nlmsg_type == NLMSG_DONE {
                    done = 1;
                }

                if (*nlh).nlmsg_type == NLMSG_ERROR {
                    // error message, better to return
                    // error code in error messages

                    if (*nlh).nlmsg_len < NLMSG_LENGTH!(mem::size_of::<nlmsgerr>()) {
                        // truncated
                        return Err(ErrorKind::ErrorCode("truncated message".to_string()).into());
                    }

                    let el: *const nlmsgerr = NLMSG_DATA!(nlh) as *const nlmsgerr;
                    return Err(
                        ErrorKind::Nix(nix::Error::Sys(Errno::from_i32(-(*el).error))).into(),
                    );
                }

                lv.push(nlh);

                if done == 1 {
                    break;
                }

                nlh = NLMSG_NEXT!(nlh, msglen);
            }

            slv.push(buf);

            if done == 1 {
                if dump_intr == 1 {
                    info!(sl!(), "dump interuppted, maybe incomplete");
                }

                break;
            }

            // still remain some bytes?

            if msglen != 0 {
                return Err(ErrorKind::Nix(nix::Error::Sys(Errno::EINVAL)).into());
            }
        }
        Ok((slv, lv))
    }

    pub fn list_interfaces(&mut self) -> Result<Vec<Interface>> {
        let mut ifaces: Vec<Interface> = Vec::new();

        unsafe {
            // get link info
            let (_slv, lv) = self.dump_all_links()?;

            // get addrinfo
            let (_sav, av) = self.dump_all_addresses(0)?;

            // got all the link message and address message
            // into lv and av repectively, parse attributes
            for link in &lv {
                let nlh: *const nlmsghdr = *link;
                let ifi: *const ifinfomsg = NLMSG_DATA!(nlh) as *const ifinfomsg;

                if (*nlh).nlmsg_type != RTM_NEWLINK && (*nlh).nlmsg_type != RTM_DELLINK {
                    continue;
                }

                if (*nlh).nlmsg_len < NLMSG_SPACE!(mem::size_of::<ifinfomsg>()) {
                    info!(
                        sl!(),
                        "invalid nlmsg! nlmsg_len: {}, nlmsg_space: {}",
                        (*nlh).nlmsg_len,
                        NLMSG_SPACE!(mem::size_of::<ifinfomsg>())
                    );
                    break;
                }

                let rta: *mut rtattr = IFLA_RTA!(ifi) as *mut rtattr;
                let rtalen = IFLA_PAYLOAD!(nlh) as u32;

                let attrs = parse_attrs(rta, rtalen, (IFLA_MAX + 1) as usize)?;

                // fill out some fields of Interface,
                let mut iface: Interface = Interface::default();

                if attrs[IFLA_IFNAME as usize] as i64 != 0 {
                    let t = attrs[IFLA_IFNAME as usize];
                    iface.name = String::from_utf8(getattr_var(t as *const rtattr))?;
                }

                if attrs[IFLA_MTU as usize] as i64 != 0 {
                    let t = attrs[IFLA_MTU as usize];
                    iface.mtu = getattr32(t) as u64;
                }

                if attrs[IFLA_ADDRESS as usize] as i64 != 0 {
                    let alen = RTA_PAYLOAD!(attrs[IFLA_ADDRESS as usize]);
                    let a: *const u8 = RTA_DATA!(attrs[IFLA_ADDRESS as usize]) as *const u8;
                    iface.hwAddr = format_address(a, alen as u32)?;
                }

                // get ip address info from av
                let mut ads: Vec<IPAddress> = Vec::new();

                for address in &av {
                    let alh: *const nlmsghdr = *address;
                    let ifa: *const ifaddrmsg = NLMSG_DATA!(alh) as *const ifaddrmsg;
                    let arta: *mut rtattr = IFA_RTA!(ifa) as *mut rtattr;

                    if (*alh).nlmsg_type != RTM_NEWADDR {
                        continue;
                    }

                    let tlen = NLMSG_SPACE!(mem::size_of::<ifaddrmsg>());
                    if (*alh).nlmsg_len < tlen {
                        info!(
                            sl!(),
                            "invalid nlmsg! nlmsg_len: {}, nlmsg_space: {}",
                            (*alh).nlmsg_len,
                            tlen
                        );
                        break;
                    }

                    let artalen = IFA_PAYLOAD!(alh) as u32;

                    if (*ifa).ifa_index as u32 == (*ifi).ifi_index as u32 {
                        // found target addresses
                        // parse attributes and fill out Interface
                        let addrs = parse_attrs(arta, artalen, (IFA_MAX + 1) as usize)?;
                        // fill address field of Interface
                        let mut one: IPAddress = IPAddress::default();
                        let mut tattr: *const rtattr = addrs[IFA_LOCAL as usize];
                        if addrs[IFA_ADDRESS as usize] as i64 != 0 {
                            tattr = addrs[IFA_ADDRESS as usize];
                        }

                        one.mask = format!("{}", (*ifa).ifa_prefixlen);
                        let a: *const u8 = RTA_DATA!(tattr) as *const u8;
                        let alen = RTA_PAYLOAD!(tattr);
                        one.family = IPFamily::v4;

                        if (*ifa).ifa_family == libc::AF_INET6 as u8 {
                            one.family = IPFamily::v6;
                        }

                        // only handle IPv4 for now
                        // if (*ifa).ifa_family == libc::AF_INET as u8{
                        one.address = format_address(a, alen as u32)?;
                        //}

                        ads.push(one);
                    }
                }

                iface.IPAddresses = RepeatedField::from_vec(ads);
                ifaces.push(iface);
            }
        }

        Ok(ifaces)
    }

    unsafe fn dump_all_links(&mut self) -> Result<(Vec<Vec<u8>>, Vec<*const nlmsghdr>)> {
        let mut v: Vec<u8> = vec![0; 2048];
        let p = v.as_mut_ptr() as *mut libc::c_char;
        let nlh: *mut nlmsghdr = p as *mut nlmsghdr;
        let ifi: *mut ifinfomsg = NLMSG_DATA!(nlh) as *mut ifinfomsg;

        (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifinfomsg>() as i32) as __u32;
        (*nlh).nlmsg_type = RTM_GETLINK;
        (*nlh).nlmsg_flags = (NLM_F_DUMP | NLM_F_REQUEST) as __u16;

        self.seq += 1;
        self.dump = self.seq;
        (*nlh).nlmsg_seq = self.seq;

        (*ifi).ifi_family = libc::AF_UNSPEC as u8;

        addattr32(nlh, IFLA_EXT_MASK, RTEXT_FILTER_VF);

        self.send_message(v.as_mut_slice())?;
        self.recv_dump_message()
    }

    unsafe fn dump_all_addresses(
        &mut self,
        ifindex: __u32,
    ) -> Result<(Vec<Vec<u8>>, Vec<*const nlmsghdr>)> {
        let mut v: Vec<u8> = vec![0; 2048];
        let p = v.as_mut_ptr() as *mut libc::c_char;
        let nlh: *mut nlmsghdr = p as *mut nlmsghdr;
        let ifa: *mut ifaddrmsg = NLMSG_DATA!(nlh) as *mut ifaddrmsg;

        (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifaddrmsg>());
        (*nlh).nlmsg_type = RTM_GETADDR;
        (*nlh).nlmsg_flags = NLM_F_DUMP | NLM_F_REQUEST;

        self.seq += 1;
        self.dump = self.seq;
        (*nlh).nlmsg_seq = self.seq;

        (*ifa).ifa_family = libc::AF_UNSPEC as u8;
        (*ifa).ifa_index = ifindex;

        self.send_message(v.as_mut_slice())?;

        self.recv_dump_message()
    }

    fn find_link_by_hwaddr(&mut self, hwaddr: &str) -> Result<ifinfomsg> {
        let mut hw: Vec<u8> = vec![0; 6];
        unsafe {
            //parse out hwaddr in request
            let p = hw.as_mut_ptr() as *mut u8;
            let (hw0, hw1, hw2, hw3, hw4, hw5) = scan_fmt!(hwaddr, "{x}:{x}:{x}:{x}:{x}:{x}", 
				[hex u8], [hex u8], [hex u8], [hex u8], [hex u8],
				[hex u8])?;

            hw[0] = hw0;
            hw[1] = hw1;
            hw[2] = hw2;
            hw[3] = hw3;
            hw[4] = hw4;
            hw[5] = hw5;

            // dump out all links
            let (_slv, lv) = self.dump_all_links()?;

            for link in &lv {
                let nlh: *const nlmsghdr = *link;
                let ifi: *const ifinfomsg = NLMSG_DATA!(nlh) as *const ifinfomsg;

                if (*nlh).nlmsg_type != RTM_NEWLINK && (*nlh).nlmsg_type != RTM_DELLINK {
                    continue;
                }

                if (*nlh).nlmsg_len < NLMSG_SPACE!(mem::size_of::<ifinfomsg>()) {
                    info!(
                        sl!(),
                        "invalid nlmsg! nlmsg_len: {}, nlmsg_space: {}",
                        (*nlh).nlmsg_len,
                        NLMSG_SPACE!(mem::size_of::<ifinfomsg>())
                    );
                    break;
                }

                let rta: *mut rtattr = IFLA_RTA!(ifi) as *mut rtattr;
                let rtalen = IFLA_PAYLOAD!(nlh) as u32;

                let attrs = parse_attrs(rta, rtalen, (IFLA_MAX + 1) as usize)?;

                // find the target ifinfomsg
                if attrs[IFLA_ADDRESS as usize] as i64 != 0 {
                    let a = RTA_DATA!(attrs[IFLA_ADDRESS as usize]) as *const libc::c_void;
                    if libc::memcmp(
                        p as *const libc::c_void,
                        a,
                        RTA_PAYLOAD!(attrs[IFLA_ADDRESS as usize]) as libc::size_t,
                    ) == 0
                    {
                        return Ok(ifinfomsg { ..*ifi });
                    }
                }
            }
        }

        return Err(ErrorKind::Nix(nix::Error::Sys(Errno::ENODEV)).into());
    }

    fn find_link_by_name(&mut self, name: &str) -> Result<ifinfomsg> {
        let mut v: Vec<u8> = vec![0; 2048];
        unsafe {
            let mut nlh: *mut nlmsghdr = v.as_mut_ptr() as *mut nlmsghdr;
            let mut ifi: *mut ifinfomsg = NLMSG_DATA!(nlh) as *mut ifinfomsg;

            (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifinfomsg>()) as __u32;
            (*nlh).nlmsg_type = RTM_GETLINK;
            (*nlh).nlmsg_flags = NLM_F_REQUEST;

            self.seq += 1;
            (*nlh).nlmsg_seq = self.seq;

            (*ifi).ifi_family = libc::AF_UNSPEC as u8;

            addattr_var(
                nlh,
                IFLA_IFNAME,
                name.as_ptr() as *const u8,
                (name.len() + 1),
            );

            addattr32(
                nlh,
                IFLA_EXT_MASK,
                RTEXT_FILTER_VF | RTEXT_FILTER_SKIP_STATS,
            );

            let mut retv = self.rtnl_talk(v.as_mut_slice(), true)?;

            nlh = retv.as_mut_ptr() as *mut nlmsghdr;
            ifi = NLMSG_DATA!(nlh) as *mut ifinfomsg;

            return Ok(ifinfomsg { ..*ifi });
        }
    }

    fn rtnl_talk(&mut self, data: &mut [u8], answer: bool) -> Result<Vec<u8>> {
        unsafe {
            let nlh: *mut nlmsghdr = data.as_mut_ptr() as *mut nlmsghdr;
            if !answer {
                (*nlh).nlmsg_flags |= NLM_F_ACK;
            }
        }

        self.send_message(data)?;
        unsafe {
            loop {
                let buf = self.recv_message()?;
                let mut msglen = buf.len() as u32;
                let mut nlh = buf.as_ptr() as *const nlmsghdr;

                while NLMSG_OK!(nlh, msglen) {
                    // not for us

                    if (*nlh).nlmsg_pid != self.local.nl_pid {
                        nlh = NLMSG_NEXT!(nlh, msglen);
                        continue;
                    }

                    if (*nlh).nlmsg_type == NLMSG_ERROR {
                        // error message, better to return
                        // error code in error messages

                        if (*nlh).nlmsg_len < NLMSG_LENGTH!(mem::size_of::<nlmsgerr>()) {
                            // truncated
                            return Err(
                                ErrorKind::ErrorCode("truncated message".to_string()).into()
                            );
                        }

                        let el: *const nlmsgerr = NLMSG_DATA!(nlh) as *const nlmsgerr;

                        // this is ack. -_-
                        if (*el).error == 0 {
                            return Ok(Vec::new());
                        }

                        return Err(
                            ErrorKind::Nix(nix::Error::Sys(Errno::from_i32(-(*el).error))).into(),
                        );
                    }

                    // goog message
                    if answer {
                        // need to copy out data

                        let mut d: Vec<u8> = vec![0; (*nlh).nlmsg_len as usize];
                        let dp: *mut libc::c_void = d.as_mut_ptr() as *mut libc::c_void;
                        libc::memcpy(
                            dp,
                            nlh as *const libc::c_void,
                            (*nlh).nlmsg_len as libc::size_t,
                        );
                        return Ok(d);
                    } else {
                        return Ok(Vec::new());
                    }
                }

                if !(NLMSG_OK!(nlh, msglen)) {
                    return Err(ErrorKind::Nix(nix::Error::Sys(Errno::EINVAL)).into());
                }
            }
        }
    }

    fn set_link_status(&mut self, ifinfo: &ifinfomsg, up: bool) -> Result<()> {
        let mut v: Vec<u8> = vec![0; 2048];
        unsafe {
            let p: *mut u8 = v.as_mut_ptr() as *mut u8;
            let mut nlh: *mut nlmsghdr = p as *mut nlmsghdr;
            let mut ifi: *mut ifinfomsg = NLMSG_DATA!(nlh) as *mut ifinfomsg;

            (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifinfomsg>() as u32) as __u32;
            (*nlh).nlmsg_type = RTM_NEWLINK;
            (*nlh).nlmsg_flags = NLM_F_REQUEST;

            self.seq += 1;
            (*nlh).nlmsg_seq = self.seq;

            (*ifi).ifi_family = ifinfo.ifi_family;
            (*ifi).ifi_type = ifinfo.ifi_type;
            (*ifi).ifi_index = ifinfo.ifi_index;

            (*ifi).ifi_change |= libc::IFF_UP as u32;

            if up {
                (*ifi).ifi_flags |= libc::IFF_UP as u32;
            } else {
                (*ifi).ifi_flags &= !libc::IFF_UP as u32;
            }
        }

        self.rtnl_talk(v.as_mut_slice(), false)?;

        Ok(())
    }

    fn delete_one_addr(&mut self, ifinfo: &ifinfomsg, addr: &RtIPAddr) -> Result<()> {
        let mut v: Vec<u8> = vec![0; 2048];
        unsafe {
            let p: *mut u8 = v.as_mut_ptr() as *mut u8;

            let mut nlh: *mut nlmsghdr = p as *mut nlmsghdr;
            let mut ifa: *mut ifaddrmsg = NLMSG_DATA!(nlh) as *mut ifaddrmsg;

            (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifaddrmsg>() as u32) as __u32;
            (*nlh).nlmsg_type = RTM_DELADDR;
            (*nlh).nlmsg_flags = NLM_F_REQUEST;

            self.seq += 1;
            (*nlh).nlmsg_seq = self.seq;

            (*ifa).ifa_family = addr.ip_family;
            (*ifa).ifa_prefixlen = addr.ip_mask;
            (*ifa).ifa_index = ifinfo.ifi_index as u32;

            addattr_var(
                nlh,
                IFA_ADDRESS,
                addr.addr.as_ptr() as *const u8,
                addr.addr.len(),
            );
        }

        // ignore EADDRNOTAVAIL here..
        self.rtnl_talk(v.as_mut_slice(), false)?;

        Ok(())
    }

    fn delete_all_addrs(&mut self, ifinfo: &ifinfomsg, addrs: &Vec<RtIPAddr>) -> Result<()> {
        for a in addrs {
            self.delete_one_addr(ifinfo, a)?;
        }

        Ok(())
    }

    fn get_link_addresses(&mut self, ifinfo: &ifinfomsg) -> Result<Vec<RtIPAddr>> {
        let mut del_addrs: Vec<RtIPAddr> = Vec::new();
        unsafe {
            let (_sav, av) = self.dump_all_addresses(ifinfo.ifi_index as __u32)?;

            for a in &av {
                let nlh: *const nlmsghdr = *a;
                let ifa: *const ifaddrmsg = NLMSG_DATA!(nlh) as *const ifaddrmsg;

                if (*nlh).nlmsg_type != RTM_NEWADDR {
                    continue;
                }

                let tlen = NLMSG_SPACE!(mem::size_of::<ifaddrmsg>());
                if (*nlh).nlmsg_len < tlen {
                    info!(
                        sl!(),
                        "invalid nlmsg! nlmsg_len: {}, nlmsg_space: {}",
                        (*nlh).nlmsg_len,
                        tlen
                    );
                    break;
                }

                if (*ifa).ifa_flags as u32 & IFA_F_SECONDARY != 0 {
                    continue;
                }

                let rta: *mut rtattr = IFA_RTA!(ifa) as *mut rtattr;
                let rtalen = IFA_PAYLOAD!(nlh) as u32;

                if ifinfo.ifi_index as u32 == (*ifa).ifa_index {
                    let attrs = parse_attrs(rta, rtalen, (IFA_MAX + 1) as usize)?;
                    let mut t: *const rtattr = attrs[IFA_LOCAL as usize];

                    if attrs[IFA_ADDRESS as usize] as i64 != 0 {
                        t = attrs[IFA_ADDRESS as usize];
                    }

                    let addr = getattr_var(t as *const rtattr);

                    del_addrs.push(RtIPAddr {
                        ip_family: (*ifa).ifa_family,
                        ip_mask: (*ifa).ifa_prefixlen,
                        addr,
                    });
                }
            }
        }

        Ok(del_addrs)
    }

    fn add_one_address(&mut self, ifinfo: &ifinfomsg, ip: &RtIPAddr) -> Result<()> {
        let mut v: Vec<u8> = vec![0; 2048];
        unsafe {
            let mut nlh: *mut nlmsghdr = v.as_mut_ptr() as *mut nlmsghdr;
            let mut ifa: *mut ifaddrmsg = NLMSG_DATA!(nlh) as *mut ifaddrmsg;

            (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifaddrmsg>() as u32) as __u32;
            (*nlh).nlmsg_type = RTM_NEWADDR;
            (*nlh).nlmsg_flags = NLM_F_REQUEST | NLM_F_CREATE | NLM_F_EXCL;
            self.seq += 1;
            (*nlh).nlmsg_seq = self.seq;

            (*ifa).ifa_family = ip.ip_family;
            (*ifa).ifa_prefixlen = ip.ip_mask;
            (*ifa).ifa_index = ifinfo.ifi_index as __u32;

            addattr_var(
                nlh,
                IFA_ADDRESS,
                ip.addr.as_ptr() as *const u8,
                ip.addr.len(),
            );
            // don't know why need IFA_LOCAL, without it
            // kernel returns -EINVAL...
            addattr_var(nlh, IFA_LOCAL, ip.addr.as_ptr() as *const u8, ip.addr.len());

            self.rtnl_talk(v.as_mut_slice(), false)?;
        }

        Ok(())
    }

    pub fn update_interface(&mut self, iface: &Interface) -> Result<Interface> {
        // the reliable way to find link is using hardware address
        // as filter. However, hardware filter might not be supported
        // by netlink, we may have to dump link list and the find the
        // target link. filter using name or family is supported, but
        // we cannot use that to find target link.
        // let's try if hardware address filter works. -_-

        let ifinfo = self.find_link_by_hwaddr(iface.hwAddr.as_str())?;

        // bring down interface if it is up

        if ifinfo.ifi_flags & libc::IFF_UP as u32 != 0 {
            self.set_link_status(&ifinfo, false)?;
        }

        // delete all addresses associated with the link
        let del_addrs: Vec<RtIPAddr> = self.get_link_addresses(&ifinfo)?;

        self.delete_all_addrs(&ifinfo, del_addrs.as_ref())?;

        // add new ip addresses in request
        for grpc_addr in &iface.IPAddresses {
            let rtip = RtIPAddr::from(grpc_addr.clone());
            self.add_one_address(&ifinfo, &rtip)?;
        }

        // set name, set mtu, IFF_NOARP. in one rtnl_talk.
        let mut v: Vec<u8> = vec![0; 2048];
        unsafe {
            let p: *mut u8 = v.as_mut_ptr() as *mut u8;
            let mut nlh: *mut nlmsghdr = p as *mut nlmsghdr;
            let mut ifi: *mut ifinfomsg = NLMSG_DATA!(nlh) as *mut ifinfomsg;

            (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifinfomsg>() as u32) as __u32;
            (*nlh).nlmsg_type = RTM_NEWLINK;
            (*nlh).nlmsg_flags = NLM_F_REQUEST;

            self.seq += 1;
            (*nlh).nlmsg_seq = self.seq;

            (*ifi).ifi_family = ifinfo.ifi_family;
            (*ifi).ifi_type = ifinfo.ifi_type;
            (*ifi).ifi_index = ifinfo.ifi_index;

            if iface.raw_flags & libc::IFF_NOARP as u32 != 0 {
                (*ifi).ifi_change |= libc::IFF_NOARP as u32;
                (*ifi).ifi_flags |= libc::IFF_NOARP as u32;
            }

            addattr32(nlh, IFLA_MTU, iface.mtu as u32);

            // if str is null terminated, use addattr_var
            // otherwise, use addattr_str
            addattr_var(
                nlh,
                IFLA_IFNAME,
                iface.name.as_ptr() as *const u8,
                iface.name.len(),
            );
            // addattr_str(nlh, IFLA_IFNAME, iface.name.as_str());
        }

        self.rtnl_talk(v.as_mut_slice(), false)?;

        let _ = self.set_link_status(&ifinfo, true);
        // test remove this link
        // let _ = self.remove_interface(iface)?;

        Ok(iface.clone())
        //return Err(ErrorKind::Nix(nix::Error::Sys(
        //	Errno::EOPNOTSUPP)).into());
    }

    fn remove_interface(&mut self, iface: &Interface) -> Result<Interface> {
        let ifinfo = self.find_link_by_hwaddr(iface.hwAddr.as_str())?;
        self.set_link_status(&ifinfo, false)?;

        // delete this link per request
        let mut v: Vec<u8> = vec![0; 2048];
        unsafe {
            let mut nlh: *mut nlmsghdr = v.as_mut_ptr() as *mut nlmsghdr;
            let mut ifi: *mut ifinfomsg = NLMSG_DATA!(nlh) as *mut ifinfomsg;
            // No attributes needed?
            (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifinfomsg>()) as __u32;
            (*nlh).nlmsg_type = RTM_DELLINK;
            (*nlh).nlmsg_flags = NLM_F_REQUEST;

            self.seq += 1;
            (*nlh).nlmsg_seq = self.seq;

            (*ifi).ifi_family = ifinfo.ifi_family;
            (*ifi).ifi_index = ifinfo.ifi_index;
            (*ifi).ifi_type = ifinfo.ifi_type;

            self.rtnl_talk(v.as_mut_slice(), false)?;
        }

        Ok(iface.clone())
    }

    fn get_name_by_index(&mut self, index: i32) -> Result<String> {
        let mut v: Vec<u8> = vec![0; 2048];
        let mut i = 0;
        unsafe {
            while i < 5 {
                i += 1;
                let mut nlh: *mut nlmsghdr = v.as_mut_ptr() as *mut nlmsghdr;
                let mut ifi: *mut ifinfomsg = NLMSG_DATA!(nlh) as *mut ifinfomsg;

                (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<ifinfomsg>()) as __u32;
                (*nlh).nlmsg_type = RTM_GETLINK;
                (*nlh).nlmsg_flags = NLM_F_REQUEST;

                self.seq += 1;
                (*nlh).nlmsg_seq = self.seq;

                (*ifi).ifi_index = index;

                addattr32(
                    nlh,
                    IFLA_EXT_MASK,
                    RTEXT_FILTER_VF | RTEXT_FILTER_SKIP_STATS,
                );

                let mut retv = self.rtnl_talk(v.as_mut_slice(), true)?;

                let nlh: *mut nlmsghdr = retv.as_mut_ptr() as *mut nlmsghdr;
                let ifi: *mut ifinfomsg = NLMSG_DATA!(nlh) as *mut ifinfomsg;

                if (*nlh).nlmsg_type != RTM_NEWLINK && (*nlh).nlmsg_type != RTM_DELLINK {
                    info!(sl!(), "wrong message!");
                    continue;
                }

                let tlen = NLMSG_SPACE!(mem::size_of::<ifinfomsg>());
                if (*nlh).nlmsg_len < tlen {
                    info!(sl!(), "corrupt message?");
                    continue;
                }

                let rta: *mut rtattr = IFLA_RTA!(ifi) as *mut rtattr;
                let rtalen = IFLA_PAYLOAD!(nlh) as u32;

                let attrs = parse_attrs(rta, rtalen, (IFLA_MAX + 1) as usize)?;

                let t = attrs[IFLA_IFNAME as usize];
                if t as i64 != 0 {
                    // we have a name
                    let tdata = getattr_var(t as *const rtattr);
                    return Ok(String::from_utf8(tdata)?);
                }
            }
        }

        Err(ErrorKind::ErrorCode("no name".to_string()).into())
    }

    pub fn list_routes(&mut self) -> Result<Vec<Route>> {
        // currently, only dump routes from main table for ipv4
        // ie, rtmsg.rtmsg_family = AF_INET, set RT_TABLE_MAIN
        // attribute in dump request
        // Fix Me: think about othe tables, ipv6..
        let mut rs: Vec<Route> = Vec::new();

        unsafe {
            let (_srv, rv) = self.dump_all_route_msgs()?;

            // parse out routes and store in rs
            for r in &rv {
                let nlh: *const nlmsghdr = *r;
                let rtm: *const rtmsg = NLMSG_DATA!(nlh) as *const rtmsg;

                if (*nlh).nlmsg_type != RTM_NEWROUTE && (*nlh).nlmsg_type != RTM_DELROUTE {
                    info!(sl!(), "not route message!");
                    continue;
                }

                let tlen = NLMSG_SPACE!(mem::size_of::<rtmsg>());
                if (*nlh).nlmsg_len < tlen {
                    info!(
                        sl!(),
                        "invalid nlmsg! nlmsg_len: {}, nlmsg_spae: {}",
                        (*nlh).nlmsg_len,
                        tlen
                    );
                    break;
                }

                let rta: *mut rtattr = RTM_RTA!(rtm) as *mut rtattr;

                if (*rtm).rtm_table != RT_TABLE_MAIN as u8 {
                    continue;
                }

                let rtalen = RTM_PAYLOAD!(nlh) as u32;

                let attrs = parse_attrs(rta, rtalen, (RTA_MAX + 1) as usize)?;

                let t = attrs[RTA_TABLE as usize];
                if t as i64 != 0 {
                    let table = getattr32(t);
                    if table != RT_TABLE_MAIN {
                        continue;
                    }
                }
                // find source, destination, gateway, scope, and
                // and device name

                let mut t = attrs[RTA_DST as usize];
                let mut rte: Route = Route::default();

                // destination
                if t as i64 != 0 {
                    let data: *const u8 = RTA_DATA!(t) as *const u8;
                    let len = RTA_PAYLOAD!(t) as u32;
                    rte.dest = format!("{}/{}", format_address(data, len)?, (*rtm).rtm_dst_len);
                }

                // gateway
                t = attrs[RTA_GATEWAY as usize];
                if t as i64 != 0 {
                    let data: *const u8 = RTA_DATA!(t) as *const u8;
                    let len = RTA_PAYLOAD!(t) as u32;
                    rte.gateway = format_address(data, len)?;

                    // for gateway, destination is 0.0.0.0
                    rte.dest = "0.0.0.0".to_string();
                }

                // source
                t = attrs[RTA_SRC as usize];

                if t as i64 == 0 {
                    t = attrs[RTA_PREFSRC as usize];
                }

                if t as i64 != 0 {
                    let data: *const u8 = RTA_DATA!(t) as *const u8;
                    let len = RTA_PAYLOAD!(t) as u32;

                    rte.source = format_address(data, len)?;

                    if (*rtm).rtm_src_len != 0 {
                        rte.source = format!("{}/{}", rte.source.as_str(), (*rtm).rtm_src_len);
                    }
                }

                // scope
                rte.scope = (*rtm).rtm_scope as u32;

                // oif
                t = attrs[RTA_OIF as usize];
                if t as i64 != 0 {
                    let data: *const i32 = RTA_DATA!(t) as *const i32;
                    assert_eq!(RTA_PAYLOAD!(t), 4);

                    /*

                    let mut n: Vec<u8> = vec![0; libc::IF_NAMESIZE];
                    let np: *mut libc::c_char = n.as_mut_ptr() as *mut libc::c_char;
                    let tn = libc::if_indextoname(*data as u32,
                        np);

                    if tn as i64 == 0 {
                        info!(sl!(), "no name?");
                    } else {
                        info!(sl!(), "name(indextoname): {}", String::from_utf8(n)?);
                    }
                    // std::process::exit(-1);
                    */

                    rte.device = self
                        .get_name_by_index(*data)
                        .unwrap_or("unknown".to_string());
                }

                rs.push(rte);
            }
        }

        Ok(rs)
    }

    unsafe fn dump_all_route_msgs(&mut self) -> Result<(Vec<Vec<u8>>, Vec<*const nlmsghdr>)> {
        let mut v: Vec<u8> = vec![0; 2048];
        let mut nlh: *mut nlmsghdr = v.as_mut_ptr() as *mut nlmsghdr;
        let mut rtm: *mut rtmsg = NLMSG_DATA!(nlh) as *mut rtmsg;

        (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<rtmsg>()) as u32;
        (*nlh).nlmsg_type = RTM_GETROUTE;
        (*nlh).nlmsg_flags = NLM_F_REQUEST | NLM_F_DUMP;

        self.seq += 1;
        self.dump = self.seq;
        (*nlh).nlmsg_seq = self.seq;

        (*rtm).rtm_family = libc::AF_INET as u8;
        (*rtm).rtm_table = RT_TABLE_MAIN as u8;

        addattr32(nlh, RTA_TABLE, RT_TABLE_MAIN);

        self.send_message(v.as_mut_slice())?;

        self.recv_dump_message()
    }

    fn get_all_routes(&mut self) -> Result<Vec<RtRoute>> {
        let mut rs: Vec<RtRoute> = Vec::new();

        unsafe {
            let (_srv, rv) = self.dump_all_route_msgs()?;

            for r in &rv {
                let nlh: *const nlmsghdr = *r;
                let rtm: *const rtmsg = NLMSG_DATA!(nlh) as *const rtmsg;

                if (*nlh).nlmsg_type != RTM_NEWROUTE && (*nlh).nlmsg_type != RTM_DELROUTE {
                    info!(sl!(), "not route message!");
                    continue;
                }

                let tlen = NLMSG_SPACE!(mem::size_of::<rtmsg>());
                if (*nlh).nlmsg_len < tlen {
                    info!(
                        sl!(),
                        "invalid nlmsg! nlmsg_len: {}, nlmsg_spae: {}",
                        (*nlh).nlmsg_len,
                        tlen
                    );
                    break;
                }

                if (*rtm).rtm_table != RT_TABLE_MAIN as u8 {
                    continue;
                }

                let rta: *mut rtattr = RTM_RTA!(rtm) as *mut rtattr;
                let rtalen = RTM_PAYLOAD!(nlh) as u32;

                let attrs = parse_attrs(rta, rtalen, (RTA_MAX + 1) as usize)?;

                let t = attrs[RTA_TABLE as usize];
                if t as i64 != 0 {
                    let table = getattr32(t);
                    if table != RT_TABLE_MAIN {
                        continue;
                    }
                }

                // find source, destination, gateway, scope, and
                // and device name

                let mut t = attrs[RTA_DST as usize];
                let mut rte: RtRoute = RtRoute::default();

                rte.dst_len = (*rtm).rtm_dst_len;
                rte.src_len = (*rtm).rtm_src_len;
                rte.dest = None;
                rte.protocol = (*rtm).rtm_protocol;
                // destination
                if t as i64 != 0 {
                    rte.dest = Some(getattr_var(t as *const rtattr));
                }

                // gateway
                t = attrs[RTA_GATEWAY as usize];
                if t as i64 != 0 {
                    rte.gateway = Some(getattr_var(t as *const rtattr));
                    if rte.dest.is_none() {
                        rte.dest = Some(vec![0 as u8; 4]);
                    }
                }

                // source
                t = attrs[RTA_SRC as usize];

                if t as i64 == 0 {
                    t = attrs[RTA_PREFSRC as usize];
                }

                if t as i64 != 0 {
                    rte.source = Some(getattr_var(t as *const rtattr));
                }

                // scope
                rte.scope = (*rtm).rtm_scope;

                // oif
                t = attrs[RTA_OIF as usize];
                if t as i64 != 0 {
                    rte.index = getattr32(t as *const rtattr) as i32;
                }

                rs.push(rte);
            }
        }

        Ok(rs)
    }

    fn delete_all_routes(&mut self, rs: &Vec<RtRoute>) -> Result<()> {
        for r in rs {
            let name = self.get_name_by_index(r.index)?;
            if name.as_str().contains("lo") || name.as_str().contains("::1") {
                continue;
            }

            if r.protocol == RTPROTO_KERNEL {
                continue;
            }

            self.delete_one_route(r)?;
        }

        Ok(())
    }

    fn add_one_route(&mut self, r: &RtRoute) -> Result<()> {
        let mut v: Vec<u8> = vec![0; 2048];

        unsafe {
            let mut nlh: *mut nlmsghdr = v.as_mut_ptr() as *mut nlmsghdr;
            let mut rtm: *mut rtmsg = NLMSG_DATA!(nlh) as *mut rtmsg;

            (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<rtmsg>()) as u32;
            (*nlh).nlmsg_type = RTM_NEWROUTE;
            (*nlh).nlmsg_flags = NLM_F_REQUEST | NLM_F_CREATE | NLM_F_EXCL;

            self.seq += 1;
            self.dump = self.seq;
            (*nlh).nlmsg_seq = self.seq;

            (*rtm).rtm_family = libc::AF_INET as u8;
            (*rtm).rtm_table = RT_TABLE_MAIN as u8;
            (*rtm).rtm_scope = RT_SCOPE_NOWHERE;
            (*rtm).rtm_protocol = RTPROTO_BOOT;
            (*rtm).rtm_scope = RT_SCOPE_UNIVERSE;
            (*rtm).rtm_type = RTN_UNICAST;

            (*rtm).rtm_dst_len = r.dst_len;
            (*rtm).rtm_src_len = r.src_len;
            (*rtm).rtm_scope = r.scope;

            if r.source.is_some() {
                let len = r.source.as_ref().unwrap().len();
                if r.src_len > 0 {
                    addattr_var(
                        nlh,
                        RTA_SRC,
                        r.source.as_ref().unwrap().as_ptr() as *const u8,
                        len,
                    );
                } else {
                    addattr_var(
                        nlh,
                        RTA_PREFSRC,
                        r.source.as_ref().unwrap().as_ptr() as *const u8,
                        len,
                    );
                }
            }

            if r.dest.is_some() {
                let len = r.dest.as_ref().unwrap().len();
                addattr_var(
                    nlh,
                    RTA_DST,
                    r.dest.as_ref().unwrap().as_ptr() as *const u8,
                    len,
                );
            }

            if r.gateway.is_some() {
                let len = r.gateway.as_ref().unwrap().len();
                addattr_var(
                    nlh,
                    RTA_GATEWAY,
                    r.gateway.as_ref().unwrap().as_ptr() as *const u8,
                    len,
                );
            }

            addattr32(nlh, RTA_OIF, r.index as u32);

            self.rtnl_talk(v.as_mut_slice(), false)?;
        }
        Ok(())
    }

    fn delete_one_route(&mut self, r: &RtRoute) -> Result<()> {
        info!(sl!(), "delete route");
        let mut v: Vec<u8> = vec![0; 2048];
        unsafe {
            let mut nlh: *mut nlmsghdr = v.as_mut_ptr() as *mut nlmsghdr;
            let mut rtm: *mut rtmsg = NLMSG_DATA!(nlh) as *mut rtmsg;

            (*nlh).nlmsg_len = NLMSG_LENGTH!(mem::size_of::<rtmsg>()) as u32;
            (*nlh).nlmsg_type = RTM_DELROUTE;
            (*nlh).nlmsg_flags = NLM_F_REQUEST;

            self.seq += 1;
            self.dump = self.seq;
            (*nlh).nlmsg_seq = self.seq;

            (*rtm).rtm_family = libc::AF_INET as u8;
            (*rtm).rtm_table = RT_TABLE_MAIN as u8;
            (*rtm).rtm_scope = RT_SCOPE_NOWHERE;

            (*rtm).rtm_dst_len = r.dst_len;
            (*rtm).rtm_src_len = r.src_len;
            (*rtm).rtm_scope = r.scope;

            if r.source.is_some() {
                let len = r.source.as_ref().unwrap().len();
                if r.src_len > 0 {
                    addattr_var(
                        nlh,
                        RTA_SRC,
                        r.source.as_ref().unwrap().as_ptr() as *const u8,
                        len,
                    );
                } else {
                    addattr_var(
                        nlh,
                        RTA_PREFSRC,
                        r.source.as_ref().unwrap().as_ptr() as *const u8,
                        len,
                    );
                }
            }

            if r.dest.is_some() {
                let len = r.dest.as_ref().unwrap().len();
                addattr_var(
                    nlh,
                    RTA_DST,
                    r.dest.as_ref().unwrap().as_ptr() as *const u8,
                    len,
                );
            }

            if r.gateway.is_some() {
                let len = r.gateway.as_ref().unwrap().len();
                addattr_var(
                    nlh,
                    RTA_GATEWAY,
                    r.gateway.as_ref().unwrap().as_ptr() as *const u8,
                    len,
                );
            }
            addattr32(nlh, RTA_OIF, r.index as u32);

            self.rtnl_talk(v.as_mut_slice(), false)?;
        }

        Ok(())
    }

    pub fn update_routes(&mut self, rt: &Vec<Route>) -> Result<Vec<Route>> {
        let rs = self.get_all_routes()?;
        self.delete_all_routes(&rs)?;

        for grpcroute in rt {
            if grpcroute.gateway.as_str() == "" {
                let r = RtRoute::from(grpcroute.clone());
                self.add_one_route(&r)?;
            }
        }

        for grpcroute in rt {
            if grpcroute.gateway.as_str() != "" {
                let r = RtRoute::from(grpcroute.clone());
                self.add_one_route(&r)?;
            }
        }

        Ok(rt.clone())
    }
    pub fn handle_localhost(&mut self) -> Result<()> {
        let ifi = self.find_link_by_name("lo")?;

        self.set_link_status(&ifi, true)
    }
}

unsafe fn parse_attrs(
    mut rta: *mut rtattr,
    mut rtalen: u32,
    max: usize,
) -> Result<Vec<*const rtattr>> {
    let mut attrs: Vec<*const rtattr> = vec![0 as *const rtattr; max as usize];

    while RTA_OK!(rta, rtalen) {
        let rtype = (*rta).rta_type as usize;

        if rtype < max && attrs[rtype] as i64 == 0 {
            attrs[rtype] = rta as *const rtattr;
        }

        rta = RTA_NEXT!(rta, rtalen)
    }

    Ok(attrs)
}

unsafe fn addattr_var(mut nlh: *mut nlmsghdr, cat: u16, data: *const u8, len: usize) {
    let mut rta: *mut rtattr = NLMSG_TAIL!(nlh) as *mut rtattr;
    let alen = RTA_LENGTH!(len) as u16;

    (*rta).rta_type = cat;
    (*rta).rta_len = alen;

    if len > 0 {
        libc::memcpy(
            RTA_DATA!(rta) as *mut libc::c_void,
            data as *const libc::c_void,
            len,
        );
    }

    (*nlh).nlmsg_len = NLMSG_ALIGN!((*nlh).nlmsg_len) + RTA_ALIGN!(alen);
}

unsafe fn addattr_str(mut nlh: *mut nlmsghdr, cat: u16, data: &str) {
    let mut rta: *mut rtattr = NLMSG_TAIL!(nlh) as *mut rtattr;
    let len = data.len();
    let alen = RTA_LENGTH!(len + 1) as u16;
    let tp: *mut libc::c_void = RTA_DATA!(rta) as *mut libc::c_void;

    (*rta).rta_type = cat;
    (*rta).rta_len = alen;

    libc::memcpy(
        tp,
        data.as_ptr() as *const libc::c_void,
        len as libc::size_t,
    );

    (*nlh).nlmsg_len = NLMSG_ALIGN!((*nlh).nlmsg_len) + RTA_ALIGN!(alen);
}

unsafe fn addattr_size(mut nlh: *mut nlmsghdr, cat: u16, val: u64, size: u8) {
    assert_eq!(size == 1 || size == 2 || size == 4 || size == 8, true);

    let mut rta: *mut rtattr = NLMSG_TAIL!(nlh) as *mut rtattr;
    (*rta).rta_type = cat;

    if size == 1 {
        let data: *mut u8 = RTA_DATA!(rta) as *mut u8;
        *data = val as u8;
        let len = RTA_LENGTH!(1) as u16;
        (*rta).rta_len = len;
    }

    if size == 2 {
        let data: *mut u16 = RTA_DATA!(rta) as *mut u16;
        *data = val as u16;
        let len = RTA_LENGTH!(2) as u16;
        (*rta).rta_len = len;
    }

    if size == 4 {
        let data: *mut u32 = RTA_DATA!(rta) as *mut u32;
        *data = val as u32;
        let len = RTA_LENGTH!(4) as u16;
        (*rta).rta_len = len;
    }

    if size == 8 {
        let data: *mut u64 = RTA_DATA!(rta) as *mut u64;
        *data = val as u64;
        let len = RTA_LENGTH!(8) as u16;
        (*rta).rta_len = len;
    }

    (*nlh).nlmsg_len = NLMSG_ALIGN!((*nlh).nlmsg_len) + RTA_ALIGN!((*rta).rta_len);
}

unsafe fn addattr8(nlh: *mut nlmsghdr, cat: u16, val: u8) {
    addattr_size(nlh, cat, val as u64, 1);
}

unsafe fn addattr16(nlh: *mut nlmsghdr, cat: u16, val: u16) {
    addattr_size(nlh, cat, val as u64, 2);
}

unsafe fn addattr32(nlh: *mut nlmsghdr, cat: u16, val: u32) {
    addattr_size(nlh, cat, val as u64, 4);
}

unsafe fn addattr64(nlh: *mut nlmsghdr, cat: u16, val: u64) {
    addattr_size(nlh, cat, val, 8);
}

unsafe fn getattr_var(rta: *const rtattr) -> Vec<u8> {
    assert_ne!(rta as i64, 0);
    let data: *const libc::c_void = RTA_DATA!(rta) as *const libc::c_void;
    let alen: usize = RTA_PAYLOAD!(rta) as usize;

    let mut v: Vec<u8> = vec![0; alen];
    let tp: *mut libc::c_void = v.as_mut_ptr() as *mut libc::c_void;

    libc::memcpy(tp, data, alen as libc::size_t);

    v
}

unsafe fn getattr_size(rta: *const rtattr) -> u64 {
    let alen: usize = RTA_PAYLOAD!(rta) as usize;
    assert!(alen == 1 || alen == 2 || alen == 4 || alen == 8);
    let tp: *const u8 = RTA_DATA!(rta) as *const u8;

    if alen == 1 {
        let data: *const u8 = tp as *const u8;
        return *data as u64;
    }

    if alen == 2 {
        let data: *const u16 = tp as *const u16;
        return *data as u64;
    }

    if alen == 4 {
        let data: *const u32 = tp as *const u32;
        return *data as u64;
    }

    if alen == 8 {
        let data: *const u64 = tp as *const u64;
        return *data;
    }

    panic!("impossible!");
}

unsafe fn getattr8(rta: *const rtattr) -> u8 {
    let alen = RTA_PAYLOAD!(rta);
    assert!(alen == 1);
    getattr_size(rta) as u8
}

unsafe fn getattr16(rta: *const rtattr) -> u16 {
    let alen = RTA_PAYLOAD!(rta);
    assert!(alen == 2);
    getattr_size(rta) as u16
}

unsafe fn getattr32(rta: *const rtattr) -> u32 {
    let alen = RTA_PAYLOAD!(rta);
    assert!(alen == 4);
    getattr_size(rta) as u32
}

unsafe fn getattr64(rta: *const rtattr) -> u64 {
    let alen = RTA_PAYLOAD!(rta);
    assert!(alen == 8);
    getattr_size(rta)
}

unsafe fn format_address(addr: *const u8, len: u32) -> Result<String> {
    let mut a: String;
    if len == 4 {
        // ipv4
        let mut i = 1;
        let mut p = addr as i64;

        a = format!("{}", *(p as *const u8));
        while i < len {
            p += 1;
            i += 1;
            a.push_str(format!(".{}", *(p as *const u8)).as_str());
        }

        return Ok(a);
    }

    if len == 6 {
        // hwaddr
        let mut i = 1;
        let mut p = addr as i64;

        a = format!("{:0<2X}", *(p as *const u8));
        while i < len {
            p += 1;
            i += 1;
            a.push_str(format!(":{:0<2X}", *(p as *const u8)).as_str());
        }

        return Ok(a);
    }

    if len == 16 {
        // ipv6
        let p = addr as *const u8 as *const libc::c_void;
        let mut ar: [u8; 16] = [0; 16];
        let mut v: Vec<u8> = vec![0; 16];
        let dp: *mut libc::c_void = v.as_mut_ptr() as *mut libc::c_void;
        libc::memcpy(dp, p, 16);

        ar.copy_from_slice(v.as_slice());

        return Ok(Ipv6Addr::from(ar).to_string());

        /*
            let l = len / 2;

            a = format!("{:0<4X}", *(p as *const u16));

            while i < l {
                p += 2;
                i += 1;
                a.push_str(format!(":{:0<4X}", *(p as *const u16)).as_str());
            }
        */
    }

    return Err(ErrorKind::Nix(nix::Error::Sys(Errno::EINVAL)).into());
}

impl Drop for RtnlHandle {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

pub struct RtRoute {
    pub dest: Option<Vec<u8>>,
    pub source: Option<Vec<u8>>,
    pub gateway: Option<Vec<u8>>,
    pub index: i32,
    pub scope: u8,
    pub dst_len: u8,
    pub src_len: u8,
    pub protocol: u8,
}

impl Default for RtRoute {
    fn default() -> Self {
        unsafe { mem::zeroed::<Self>() }
    }
}

fn parse_cidripv4(s: &str) -> Result<(Vec<u8>, u8)> {
    let (a0, a1, a2, a3, len) = scan_fmt!(s, "{}.{}.{}.{}/{}", u8, u8, u8, u8, u8)?;
    let ip: Vec<u8> = vec![a0, a1, a2, a3];
    Ok((ip, len))
}

fn parse_ipv4(s: &str) -> Result<Vec<u8>> {
    let (a0, a1, a2, a3) = scan_fmt!(s, "{}.{}.{}.{}", u8, u8, u8, u8)?;
    let ip: Vec<u8> = vec![a0, a1, a2, a3];

    Ok(ip)
}

fn parse_ipaddr(s: &str) -> Result<Vec<u8>> {
    if let Ok(v6) = Ipv6Addr::from_str(s) {
        return Ok(Vec::from(v6.octets().as_ref()));
    }

    // v4
    Ok(Vec::from(Ipv4Addr::from_str(s)?.octets().as_ref()))
}

fn parse_cider(s: &str) -> Result<(Vec<u8>, u8)> {
    let (addr, mask) = if s.contains("/") {
        scan_fmt!(s, "{}/{}", String, u8)?
    } else {
        (s.to_string(), 0)
    };

    Ok((parse_ipaddr(addr.as_str())?, mask))
}

impl From<Route> for RtRoute {
    fn from(r: Route) -> Self {
        // only handle ipv4

        let index = {
            let mut rh = RtnlHandle::new(NETLINK_ROUTE, 0).unwrap();
            rh.find_link_by_name(r.device.as_str()).unwrap().ifi_index
        };

        let (dest, dst_len) = if r.dest.is_empty() {
            (Some(vec![0 as u8; 4]), 0)
        } else {
            let (dst, mask) = parse_cider(r.dest.as_str()).unwrap();
            (Some(dst), mask)
        };

        let (source, src_len) = if r.source.is_empty() {
            (None, 0)
        } else {
            let (src, mask) = parse_cider(r.source.as_str()).unwrap();
            (Some(src), mask)
        };

        let gateway = if r.gateway.is_empty() {
            None
        } else {
            Some(parse_ipaddr(r.gateway.as_str()).unwrap())
        };

        /*
                let (dest, dst_len) = if gateway.is_some() {
                    (vec![0 as u8; 4], 0)
                } else {
                    (tdest, tdst_len)
                };
        */
        Self {
            dest,
            source,
            src_len,
            dst_len,
            index,
            gateway,
            scope: r.scope as u8,
            protocol: RTPROTO_UNSPEC,
        }
    }
}

pub struct RtIPAddr {
    pub ip_family: __u8,
    pub ip_mask: __u8,
    pub addr: Vec<u8>,
}

impl From<IPAddress> for RtIPAddr {
    fn from(ipi: IPAddress) -> Self {
        let ip_family = if ipi.family == IPFamily::v4 {
            libc::AF_INET
        } else {
            libc::AF_INET6
        } as __u8;

        let ip_mask = scan_fmt!(ipi.mask.as_str(), "{}", u8).unwrap();

        let addr = parse_ipaddr(ipi.address.as_ref()).unwrap();

        Self {
            ip_family,
            ip_mask,
            addr,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::netlink::{nlmsghdr, NLMSG_ALIGNTO, RTA_ALIGNTO, RTM_BASE};
    use libc;
    use std::mem;
    #[test]
    fn test_macro() {
        println!("{}", RTA_ALIGN!(10));
        assert_eq!(RTA_ALIGN!(6), 8);
        assert_eq!(RTM_FAM!(36), 5);
        assert_eq!(
            NLMSG_HDRLEN!(),
            NLMSG_ALIGN!(mem::size_of::<nlmsghdr>() as libc::c_uint)
        );
    }
}
