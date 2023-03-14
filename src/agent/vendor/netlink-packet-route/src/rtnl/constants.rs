pub use netlink_packet_core::constants::*;

pub const RTM_BASE: u16 = 16;
pub const RTM_NEWLINK: u16 = 16;
pub const RTM_DELLINK: u16 = 17;
pub const RTM_GETLINK: u16 = 18;
pub const RTM_SETLINK: u16 = 19;
pub const RTM_NEWADDR: u16 = 20;
pub const RTM_DELADDR: u16 = 21;
pub const RTM_GETADDR: u16 = 22;
pub const RTM_NEWROUTE: u16 = 24;
pub const RTM_DELROUTE: u16 = 25;
pub const RTM_GETROUTE: u16 = 26;
pub const RTM_NEWNEIGH: u16 = 28;
pub const RTM_DELNEIGH: u16 = 29;
pub const RTM_GETNEIGH: u16 = 30;
pub const RTM_NEWRULE: u16 = 32;
pub const RTM_DELRULE: u16 = 33;
pub const RTM_GETRULE: u16 = 34;
pub const RTM_NEWQDISC: u16 = 36;
pub const RTM_DELQDISC: u16 = 37;
pub const RTM_GETQDISC: u16 = 38;
pub const RTM_NEWTCLASS: u16 = 40;
pub const RTM_DELTCLASS: u16 = 41;
pub const RTM_GETTCLASS: u16 = 42;
pub const RTM_NEWTFILTER: u16 = 44;
pub const RTM_DELTFILTER: u16 = 45;
pub const RTM_GETTFILTER: u16 = 46;
pub const RTM_NEWACTION: u16 = 48;
pub const RTM_DELACTION: u16 = 49;
pub const RTM_GETACTION: u16 = 50;
pub const RTM_NEWPREFIX: u16 = 52;
pub const RTM_GETMULTICAST: u16 = 58;
pub const RTM_GETANYCAST: u16 = 62;
pub const RTM_NEWNEIGHTBL: u16 = 64;
pub const RTM_GETNEIGHTBL: u16 = 66;
pub const RTM_SETNEIGHTBL: u16 = 67;
pub const RTM_NEWNDUSEROPT: u16 = 68;
pub const RTM_NEWADDRLABEL: u16 = 72;
pub const RTM_DELADDRLABEL: u16 = 73;
pub const RTM_GETADDRLABEL: u16 = 74;
pub const RTM_GETDCB: u16 = 78;
pub const RTM_SETDCB: u16 = 79;
pub const RTM_NEWNETCONF: u16 = 80;
pub const RTM_DELNETCONF: u16 = 81;
pub const RTM_GETNETCONF: u16 = 82;
pub const RTM_NEWMDB: u16 = 84;
pub const RTM_DELMDB: u16 = 85;
pub const RTM_GETMDB: u16 = 86;
pub const RTM_NEWNSID: u16 = 88;
pub const RTM_DELNSID: u16 = 89;
pub const RTM_GETNSID: u16 = 90;
pub const RTM_NEWSTATS: u16 = 92;
pub const RTM_GETSTATS: u16 = 94;
pub const RTM_NEWCACHEREPORT: u16 = 96;
pub const RTM_NEWCHAIN: u16 = 100;
pub const RTM_DELCHAIN: u16 = 101;
pub const RTM_GETCHAIN: u16 = 102;
pub const RTM_NEWLINKPROP: u16 = 108;
pub const RTM_DELLINKPROP: u16 = 109;

/// Unknown route
pub const RTN_UNSPEC: u8 = 0;
/// A gateway or direct route
pub const RTN_UNICAST: u8 = 1;
/// A local interface route
pub const RTN_LOCAL: u8 = 2;
/// A local broadcast route (sent as a broadcast)
pub const RTN_BROADCAST: u8 = 3;
/// A local broadcast route (sent as a unicast)
pub const RTN_ANYCAST: u8 = 4;
/// A multicast route
pub const RTN_MULTICAST: u8 = 5;
/// A packet dropping route
pub const RTN_BLACKHOLE: u8 = 6;
/// An unreachable destination
pub const RTN_UNREACHABLE: u8 = 7;
/// A packet rejection route
pub const RTN_PROHIBIT: u8 = 8;
/// Continue routing lookup in another table
pub const RTN_THROW: u8 = 9;
/// A network address translation rule
pub const RTN_NAT: u8 = 10;
/// Refer to an external resolver (not implemented)
pub const RTN_XRESOLVE: u8 = 11;

/// Unknown
pub const RTPROT_UNSPEC: u8 = 0;
/// Route was learnt by an ICMP redirect
pub const RTPROT_REDIRECT: u8 = 1;
/// Route was learnt by the kernel
pub const RTPROT_KERNEL: u8 = 2;
/// Route was learnt during boot
pub const RTPROT_BOOT: u8 = 3;
/// Route was set statically
pub const RTPROT_STATIC: u8 = 4;
pub const RTPROT_GATED: u8 = 8;
pub const RTPROT_RA: u8 = 9;
pub const RTPROT_MRT: u8 = 10;
pub const RTPROT_ZEBRA: u8 = 11;
pub const RTPROT_BIRD: u8 = 12;
pub const RTPROT_DNROUTED: u8 = 13;
pub const RTPROT_XORP: u8 = 14;
pub const RTPROT_NTK: u8 = 15;
pub const RTPROT_DHCP: u8 = 16;
pub const RTPROT_MROUTED: u8 = 17;
pub const RTPROT_BABEL: u8 = 42;

/// The destination is globally valid.
pub const RT_SCOPE_UNIVERSE: u8 = 0;
/// (IPv6 only) the destination is site local, i.e. it is valid inside this site.  This is for interior
/// routes in the local autonomous system
pub const RT_SCOPE_SITE: u8 = 200;
/// The destination is link local
pub const RT_SCOPE_LINK: u8 = 253;
/// The destination is valid only on this host
pub const RT_SCOPE_HOST: u8 = 254;
/// Destination doesn't exist
pub const RT_SCOPE_NOWHERE: u8 = 255;

/// An unspecified routing table
pub const RT_TABLE_UNSPEC: u8 = 0;

/// A route table introduced for compatibility with old software which do not support table IDs
/// greater than 255. See commit `709772e6e065` in the kernel:
///
/// ```no_rust
/// commit 709772e6e06564ed94ba740de70185ac3d792773
/// Author: Krzysztof Piotr Oledzki <ole@ans.pl>
/// Date:   Tue Jun 10 15:44:49 2008 -0700
///
///     net: Fix routing tables with id > 255 for legacy software
///
///     Most legacy software do not like tables > 255 as rtm_table is u8
///     so tb_id is sent &0xff and it is possible to mismatch for example
///     table 510 with table 254 (main).
///
///     This patch introduces RT_TABLE_COMPAT=252 so the code uses it if
///     tb_id > 255. It makes such old applications happy, new
///     ones are still able to use RTA_TABLE to get a proper table id.
///
///     Signed-off-by: Krzysztof Piotr Oledzki <ole@ans.pl>
///     Acked-by: Patrick McHardy <kaber@trash.net>
///     Signed-off-by: David S. Miller <davem@davemloft.net>
/// ```
pub const RT_TABLE_COMPAT: u8 = 252;

/// The default routing table.
///
/// The default table is empty and has little use. It has been kept when the current incarnation of
/// advanced routing has been introduced in Linux 2.1.68 after a first tentative using "classes" in
/// Linux 2.1.15.
/// # Source
///
/// This documentation is taken from [Vincent Bernat's excellent
/// blog](https://vincent.bernat.ch/en/blog/2017-ipv4-route-lookup-linux#builtin-tables)
pub const RT_TABLE_DEFAULT: u8 = 253;

/// The main routing table.
///
/// By default, apart from the local ones which are added to the local table, routes that are added
/// to this table.
pub const RT_TABLE_MAIN: u8 = 254;

/// The local table.
///
/// This table is populated automatically by the kernel when addresses are configured.
///
/// On a machine that has `192.168.44.211/24` configured on `wlp58s0`, `iproute2` shows the following routes in the local table:
///
/// ```no_rust
/// $ ip route show table local
///
/// broadcast 127.0.0.0 dev lo proto kernel scope link src 127.0.0.1
/// local 127.0.0.0/8 dev lo proto kernel scope host src 127.0.0.1
/// local 127.0.0.1 dev lo proto kernel scope host src 127.0.0.1
/// broadcast 127.255.255.255 dev lo proto kernel scope link src 127.0.0.1
///
/// broadcast 192.168.44.0 dev wlp58s0 proto kernel scope link src 192.168.44.211
/// local 192.168.44.211 dev wlp58s0 proto kernel scope host src 192.168.44.211
/// broadcast 192.168.44.255 dev wlp58s0 proto kernel scope link src 192.168.44.211
/// ```
///
/// When the IP address `192.168.44.211` was configured on the `wlp58s0` interface, the kernel
/// automatically added the appropriate routes:
///
/// - a route for `192.168.44.211` for local unicast delivery to the IP address
/// - a route for `192.168.44.255` for broadcast delivery to the broadcast address
/// - a route for `192.168.44.0` for broadcast delivery to the network address
///
/// When `127.0.0.1` was configured on the loopback interface, the same kind of routes were added to
/// the local table. However, a loopback address receives a special treatment and the kernel also
/// adds the whole subnet to the local table.
///
/// Note that this is similar for IPv6:
///
/// ```no_rust
/// $ ip -6 route show table local
/// local ::1 dev lo proto kernel metric 0 pref medium
/// local fe80::7de1:4914:99b7:aa28 dev wlp58s0 proto kernel metric 0 pref medium
/// ff00::/8 dev wlp58s0 metric 256 pref medium
/// ```
///
/// # Source
///
/// This documentation is adapted from [Vincent Bernat's excellent
/// blog](https://vincent.bernat.ch/en/blog/2017-ipv4-route-lookup-linux#builtin-tables)
pub const RT_TABLE_LOCAL: u8 = 255;

/// If the route changes, notify the user via rtnetlink
pub const RTM_F_NOTIFY: u32 = 256;
/// This route is cloned. Cloned routes are routes coming from the cache instead of the FIB. For
/// IPv4, the cache was removed in Linux 3.6 (see [IPv4 route lookup on Linux] for more information
/// about IPv4 routing)
///
/// [IPv4 route lookup on Linux]: https://vincent.bernat.ch/en/blog/2017-ipv4-route-lookup-linux
pub const RTM_F_CLONED: u32 = 512;
/// Multipath equalizer (not yet implemented)
pub const RTM_F_EQUALIZE: u32 = 1024;
/// Prefix addresses
pub const RTM_F_PREFIX: u32 = 2048;
/// Show the table from which the lookup result comes. Note that before commit `c36ba6603a11`, Linux
/// would always hardcode [`RouteMessageHeader.table`] (known as `rtmsg.rtm_table` in the kernel) to
/// `RT_TABLE_MAIN`.
///
/// [`RouteMessageHeader.table`]: ../struct.RouteMessageHeader.html#structfield.table
pub const RTM_F_LOOKUP_TABLE: u32 = 4096;
/// Return the full FIB lookup match (see commit `b61798130f1be5bff08712308126c2d7ebe390ef`)
pub const RTM_F_FIB_MATCH: u32 = 8192;

pub const AF_UNSPEC: u16 = libc::AF_UNSPEC as u16;
pub const AF_UNIX: u16 = libc::AF_UNIX as u16;
// pub const AF_LOCAL: u16 = libc::AF_LOCAL as u16;
pub const AF_INET: u16 = libc::AF_INET as u16;
pub const AF_AX25: u16 = libc::AF_AX25 as u16;
pub const AF_IPX: u16 = libc::AF_IPX as u16;
pub const AF_APPLETALK: u16 = libc::AF_APPLETALK as u16;
pub const AF_NETROM: u16 = libc::AF_NETROM as u16;
pub const AF_BRIDGE: u16 = libc::AF_BRIDGE as u16;
pub const AF_ATMPVC: u16 = libc::AF_ATMPVC as u16;
pub const AF_X25: u16 = libc::AF_X25 as u16;
pub const AF_INET6: u16 = libc::AF_INET6 as u16;
pub const AF_ROSE: u16 = libc::AF_ROSE as u16;
pub const AF_DECNET: u16 = libc::AF_DECnet as u16;
pub const AF_NETBEUI: u16 = libc::AF_NETBEUI as u16;
pub const AF_SECURITY: u16 = libc::AF_SECURITY as u16;
pub const AF_KEY: u16 = libc::AF_KEY as u16;
pub const AF_NETLINK: u16 = libc::AF_NETLINK as u16;
// pub const AF_ROUTE: u16 = libc::AF_ROUTE as u16;
pub const AF_PACKET: u16 = libc::AF_PACKET as u16;
pub const AF_ASH: u16 = libc::AF_ASH as u16;
pub const AF_ECONET: u16 = libc::AF_ECONET as u16;
pub const AF_ATMSVC: u16 = libc::AF_ATMSVC as u16;
pub const AF_RDS: u16 = libc::AF_RDS as u16;
pub const AF_SNA: u16 = libc::AF_SNA as u16;
pub const AF_IRDA: u16 = libc::AF_IRDA as u16;
pub const AF_PPPOX: u16 = libc::AF_PPPOX as u16;
pub const AF_WANPIPE: u16 = libc::AF_WANPIPE as u16;
pub const AF_LLC: u16 = libc::AF_LLC as u16;
pub const AF_CAN: u16 = libc::AF_CAN as u16;
pub const AF_TIPC: u16 = libc::AF_TIPC as u16;
pub const AF_BLUETOOTH: u16 = libc::AF_BLUETOOTH as u16;
pub const AF_IUCV: u16 = libc::AF_IUCV as u16;
pub const AF_RXRPC: u16 = libc::AF_RXRPC as u16;
pub const AF_ISDN: u16 = libc::AF_ISDN as u16;
pub const AF_PHONET: u16 = libc::AF_PHONET as u16;
pub const AF_IEEE802154: u16 = libc::AF_IEEE802154 as u16;
pub const AF_CAIF: u16 = libc::AF_CAIF as u16;
pub const AF_ALG: u16 = libc::AF_ALG as u16;

pub const NETNSA_NONE: u16 = 0;
pub const NETNSA_NSID: u16 = 1;
pub const NETNSA_PID: u16 = 2;
pub const NETNSA_FD: u16 = 3;
pub const NETNSA_NSID_NOT_ASSIGNED: i32 = -1;

/// Neighbour cache entry state: the neighbour has not (yet) been resolved
pub const NUD_INCOMPLETE: u16 = 1;
/// Neighbour cache entry state: the neighbour entry is valid until its lifetime expires
pub const NUD_REACHABLE: u16 = 2;
/// Neighbour cache entry state: the neighbour entry is valid but suspicious
pub const NUD_STALE: u16 = 4;
/// Neighbour cache entry state: the validation of this entry is currently delayed
pub const NUD_DELAY: u16 = 8;
/// Neighbour cache entry state: the neighbour entry is being probed
pub const NUD_PROBE: u16 = 16;
/// Neighbour cache entry state: the validation of this entry has failed
pub const NUD_FAILED: u16 = 32;
/// Neighbour cache entry state: entry is valid and the kernel will not try to validate or refresh
/// it.
pub const NUD_NOARP: u16 = 64;
/// Neighbour cache entry state: entry is valid forever and can only be removed explicitly from
/// userspace.
pub const NUD_PERMANENT: u16 = 128;
/// Neighbour cache entry state: pseudo state for fresh entries or before deleting entries
pub const NUD_NONE: u16 = 0;

// Neighbour cache entry flags
pub const NTF_USE: u8 = 1;
pub const NTF_SELF: u8 = 2;
pub const NTF_MASTER: u8 = 4;
pub const NTF_PROXY: u8 = 8;
pub const NTF_EXT_LEARNED: u8 = 16;
pub const NTF_OFFLOADED: u8 = 32;
pub const NTF_ROUTER: u8 = 128;

pub const TCA_UNSPEC: u16 = 0;
pub const TCA_KIND: u16 = 1;
pub const TCA_OPTIONS: u16 = 2;
pub const TCA_STATS: u16 = 3;
pub const TCA_XSTATS: u16 = 4;
pub const TCA_RATE: u16 = 5;
pub const TCA_FCNT: u16 = 6;
pub const TCA_STATS2: u16 = 7;
pub const TCA_STAB: u16 = 8;
pub const TCA_PAD: u16 = 9;
pub const TCA_DUMP_INVISIBLE: u16 = 10;
pub const TCA_CHAIN: u16 = 11;
pub const TCA_HW_OFFLOAD: u16 = 12;
pub const TCA_INGRESS_BLOCK: u16 = 13;
pub const TCA_EGRESS_BLOCK: u16 = 14;
pub const TCA_STATS_UNSPEC: u16 = 0;
pub const TCA_STATS_BASIC: u16 = 1;
pub const TCA_STATS_RATE_EST: u16 = 2;
pub const TCA_STATS_QUEUE: u16 = 3;
pub const TCA_STATS_APP: u16 = 4;
pub const TCA_STATS_RATE_EST64: u16 = 5;
pub const TCA_STATS_PAD: u16 = 6;
pub const TCA_STATS_BASIC_HW: u16 = 7;

pub const NDTA_UNSPEC: u16 = 0;
pub const NDTA_NAME: u16 = 1;
pub const NDTA_THRESH1: u16 = 2;
pub const NDTA_THRESH2: u16 = 3;
pub const NDTA_THRESH3: u16 = 4;
pub const NDTA_CONFIG: u16 = 5;
pub const NDTA_PARMS: u16 = 6;
pub const NDTA_STATS: u16 = 7;
pub const NDTA_GC_INTERVAL: u16 = 8;
pub const NDTA_PAD: u16 = 9;

pub const RTA_UNSPEC: u16 = 0;
pub const RTA_DST: u16 = 1;
pub const RTA_SRC: u16 = 2;
pub const RTA_IIF: u16 = 3;
pub const RTA_OIF: u16 = 4;
pub const RTA_GATEWAY: u16 = 5;
pub const RTA_PRIORITY: u16 = 6;
pub const RTA_PREFSRC: u16 = 7;
pub const RTA_METRICS: u16 = 8;
pub const RTA_MULTIPATH: u16 = 9;
pub const RTA_PROTOINFO: u16 = 10;
pub const RTA_FLOW: u16 = 11;
pub const RTA_CACHEINFO: u16 = 12;
pub const RTA_SESSION: u16 = 13;
pub const RTA_MP_ALGO: u16 = 14;
pub const RTA_TABLE: u16 = 15;
pub const RTA_MARK: u16 = 16;
pub const RTA_MFC_STATS: u16 = 17;
pub const RTA_VIA: u16 = 18;
pub const RTA_NEWDST: u16 = 19;
pub const RTA_PREF: u16 = 20;
pub const RTA_ENCAP_TYPE: u16 = 21;
pub const RTA_ENCAP: u16 = 22;
pub const RTA_EXPIRES: u16 = 23;
pub const RTA_PAD: u16 = 24;
pub const RTA_UID: u16 = 25;
pub const RTA_TTL_PROPAGATE: u16 = 26;

pub const RTAX_UNSPEC: u16 = 0;
pub const RTAX_LOCK: u16 = 1;
pub const RTAX_MTU: u16 = 2;
pub const RTAX_WINDOW: u16 = 3;
pub const RTAX_RTT: u16 = 4;
pub const RTAX_RTTVAR: u16 = 5;
pub const RTAX_SSTHRESH: u16 = 6;
pub const RTAX_CWND: u16 = 7;
pub const RTAX_ADVMSS: u16 = 8;
pub const RTAX_REORDERING: u16 = 9;
pub const RTAX_HOPLIMIT: u16 = 10;
pub const RTAX_INITCWND: u16 = 11;
pub const RTAX_FEATURES: u16 = 12;
pub const RTAX_RTO_MIN: u16 = 13;
pub const RTAX_INITRWND: u16 = 14;
pub const RTAX_QUICKACK: u16 = 15;
pub const RTAX_CC_ALGO: u16 = 16;
pub const RTAX_FASTOPEN_NO_COOKIE: u16 = 17;

pub const IFLA_INFO_UNSPEC: u16 = 0;
pub const IFLA_INFO_KIND: u16 = 1;
pub const IFLA_INFO_DATA: u16 = 2;
pub const IFLA_INFO_XSTATS: u16 = 3;
pub const IFLA_INFO_SLAVE_KIND: u16 = 4;
pub const IFLA_INFO_SLAVE_DATA: u16 = 5;
pub const IFLA_BR_UNSPEC: u16 = 0;
pub const IFLA_BR_FORWARD_DELAY: u16 = 1;
pub const IFLA_BR_HELLO_TIME: u16 = 2;
pub const IFLA_BR_MAX_AGE: u16 = 3;
pub const IFLA_BR_AGEING_TIME: u16 = 4;
pub const IFLA_BR_STP_STATE: u16 = 5;
pub const IFLA_BR_PRIORITY: u16 = 6;
pub const IFLA_BR_VLAN_FILTERING: u16 = 7;
pub const IFLA_BR_VLAN_PROTOCOL: u16 = 8;
pub const IFLA_BR_GROUP_FWD_MASK: u16 = 9;
pub const IFLA_BR_ROOT_ID: u16 = 10;
pub const IFLA_BR_BRIDGE_ID: u16 = 11;
pub const IFLA_BR_ROOT_PORT: u16 = 12;
pub const IFLA_BR_ROOT_PATH_COST: u16 = 13;
pub const IFLA_BR_TOPOLOGY_CHANGE: u16 = 14;
pub const IFLA_BR_TOPOLOGY_CHANGE_DETECTED: u16 = 15;
pub const IFLA_BR_HELLO_TIMER: u16 = 16;
pub const IFLA_BR_TCN_TIMER: u16 = 17;
pub const IFLA_BR_TOPOLOGY_CHANGE_TIMER: u16 = 18;
pub const IFLA_BR_GC_TIMER: u16 = 19;
pub const IFLA_BR_GROUP_ADDR: u16 = 20;
pub const IFLA_BR_FDB_FLUSH: u16 = 21;
pub const IFLA_BR_MCAST_ROUTER: u16 = 22;
pub const IFLA_BR_MCAST_SNOOPING: u16 = 23;
pub const IFLA_BR_MCAST_QUERY_USE_IFADDR: u16 = 24;
pub const IFLA_BR_MCAST_QUERIER: u16 = 25;
pub const IFLA_BR_MCAST_HASH_ELASTICITY: u16 = 26;
pub const IFLA_BR_MCAST_HASH_MAX: u16 = 27;
pub const IFLA_BR_MCAST_LAST_MEMBER_CNT: u16 = 28;
pub const IFLA_BR_MCAST_STARTUP_QUERY_CNT: u16 = 29;
pub const IFLA_BR_MCAST_LAST_MEMBER_INTVL: u16 = 30;
pub const IFLA_BR_MCAST_MEMBERSHIP_INTVL: u16 = 31;
pub const IFLA_BR_MCAST_QUERIER_INTVL: u16 = 32;
pub const IFLA_BR_MCAST_QUERY_INTVL: u16 = 33;
pub const IFLA_BR_MCAST_QUERY_RESPONSE_INTVL: u16 = 34;
pub const IFLA_BR_MCAST_STARTUP_QUERY_INTVL: u16 = 35;
pub const IFLA_BR_NF_CALL_IPTABLES: u16 = 36;
pub const IFLA_BR_NF_CALL_IP6TABLES: u16 = 37;
pub const IFLA_BR_NF_CALL_ARPTABLES: u16 = 38;
pub const IFLA_BR_VLAN_DEFAULT_PVID: u16 = 39;
pub const IFLA_BR_PAD: u16 = 40;
pub const IFLA_BR_VLAN_STATS_ENABLED: u16 = 41;
pub const IFLA_BR_MCAST_STATS_ENABLED: u16 = 42;
pub const IFLA_BR_MCAST_IGMP_VERSION: u16 = 43;
pub const IFLA_BR_MCAST_MLD_VERSION: u16 = 44;
pub const IFLA_BR_VLAN_STATS_PER_PORT: u16 = 45;
pub const IFLA_BR_MULTI_BOOLOPT: u16 = 46;
pub const IFLA_MACVLAN_UNSPEC: u16 = 0;
pub const IFLA_MACVLAN_MODE: u16 = 1;
pub const IFLA_MACVLAN_FLAGS: u16 = 2;
pub const IFLA_MACVLAN_MACADDR_MODE: u16 = 3;
pub const IFLA_MACVLAN_MACADDR: u16 = 4;
pub const IFLA_MACVLAN_MACADDR_DATA: u16 = 5;
pub const IFLA_MACVLAN_MACADDR_COUNT: u16 = 6;
pub const IFLA_VLAN_UNSPEC: u16 = 0;
pub const IFLA_VLAN_ID: u16 = 1;
pub const IFLA_VLAN_FLAGS: u16 = 2;
pub const IFLA_VLAN_EGRESS_QOS: u16 = 3;
pub const IFLA_VLAN_INGRESS_QOS: u16 = 4;
pub const IFLA_VLAN_PROTOCOL: u16 = 5;
pub const IFLA_VRF_UNSPEC: u16 = 0;
pub const IFLA_VRF_TABLE: u16 = 1;
pub const IFLA_IPVLAN_UNSPEC: u16 = 0;
pub const IFLA_IPVLAN_MODE: u16 = 1;
pub const IFLA_IPVLAN_FLAGS: u16 = 2;
pub const IFLA_IPOIB_UNSPEC: u16 = 0;
pub const IFLA_IPOIB_PKEY: u16 = 1;
pub const IFLA_IPOIB_MODE: u16 = 2;
pub const IFLA_IPOIB_UMCAST: u16 = 3;
pub const VETH_INFO_UNSPEC: u16 = 0;
pub const VETH_INFO_PEER: u16 = 1;

pub const ARPHRD_NETROM: u16 = 0;
pub const ARPHRD_ETHER: u16 = 1;
pub const ARPHRD_EETHER: u16 = 2;
pub const ARPHRD_AX25: u16 = 3;
pub const ARPHRD_PRONET: u16 = 4;
pub const ARPHRD_CHAOS: u16 = 5;
pub const ARPHRD_IEEE802: u16 = 6;
pub const ARPHRD_ARCNET: u16 = 7;
pub const ARPHRD_APPLETLK: u16 = 8;
pub const ARPHRD_DLCI: u16 = 15;
pub const ARPHRD_ATM: u16 = 19;
pub const ARPHRD_METRICOM: u16 = 23;
pub const ARPHRD_IEEE1394: u16 = 24;
pub const ARPHRD_EUI64: u16 = 27;
pub const ARPHRD_INFINIBAND: u16 = 32;
pub const ARPHRD_SLIP: u16 = 256;
pub const ARPHRD_CSLIP: u16 = 257;
pub const ARPHRD_SLIP6: u16 = 258;
pub const ARPHRD_CSLIP6: u16 = 259;
pub const ARPHRD_RSRVD: u16 = 260;
pub const ARPHRD_ADAPT: u16 = 264;
pub const ARPHRD_ROSE: u16 = 270;
pub const ARPHRD_X25: u16 = 271;
pub const ARPHRD_HWX25: u16 = 272;
pub const ARPHRD_CAN: u16 = 280;
pub const ARPHRD_PPP: u16 = 512;
pub const ARPHRD_CISCO: u16 = 513;
pub const ARPHRD_HDLC: u16 = 513;
pub const ARPHRD_LAPB: u16 = 516;
pub const ARPHRD_DDCMP: u16 = 517;
pub const ARPHRD_RAWHDLC: u16 = 518;
pub const ARPHRD_RAWIP: u16 = 519;
pub const ARPHRD_TUNNEL: u16 = 768;
pub const ARPHRD_TUNNEL6: u16 = 769;
pub const ARPHRD_FRAD: u16 = 770;
pub const ARPHRD_SKIP: u16 = 771;
pub const ARPHRD_LOOPBACK: u16 = 772;
pub const ARPHRD_LOCALTLK: u16 = 773;
pub const ARPHRD_FDDI: u16 = 774;
pub const ARPHRD_BIF: u16 = 775;
pub const ARPHRD_SIT: u16 = 776;
pub const ARPHRD_IPDDP: u16 = 777;
pub const ARPHRD_IPGRE: u16 = 778;
pub const ARPHRD_PIMREG: u16 = 779;
pub const ARPHRD_HIPPI: u16 = 780;
pub const ARPHRD_ASH: u16 = 781;
pub const ARPHRD_ECONET: u16 = 782;
pub const ARPHRD_IRDA: u16 = 783;
pub const ARPHRD_FCPP: u16 = 784;
pub const ARPHRD_FCAL: u16 = 785;
pub const ARPHRD_FCPL: u16 = 786;
pub const ARPHRD_FCFABRIC: u16 = 787;
pub const ARPHRD_IEEE802_TR: u16 = 800;
pub const ARPHRD_IEEE80211: u16 = 801;
pub const ARPHRD_IEEE80211_PRISM: u16 = 802;
pub const ARPHRD_IEEE80211_RADIOTAP: u16 = 803;
pub const ARPHRD_IEEE802154: u16 = 804;
pub const ARPHRD_IEEE802154_MONITOR: u16 = 805;
pub const ARPHRD_PHONET: u16 = 820;
pub const ARPHRD_PHONET_PIPE: u16 = 821;
pub const ARPHRD_CAIF: u16 = 822;
pub const ARPHRD_IP6GRE: u16 = 823;
pub const ARPHRD_NETLINK: u16 = 824;
pub const ARPHRD_6LOWPAN: u16 = 825;
pub const ARPHRD_VSOCKMON: u16 = 826;
pub const ARPHRD_VOID: u16 = 65535;
pub const ARPHRD_NONE: u16 = 65534;

pub const IFA_UNSPEC: u16 = 0;
pub const IFA_ADDRESS: u16 = 1;
pub const IFA_LOCAL: u16 = 2;
pub const IFA_LABEL: u16 = 3;
pub const IFA_BROADCAST: u16 = 4;
pub const IFA_ANYCAST: u16 = 5;
pub const IFA_CACHEINFO: u16 = 6;
pub const IFA_MULTICAST: u16 = 7;
pub const IFA_FLAGS: u16 = 8;

pub const IFLA_UNSPEC: u16 = 0;
pub const IFLA_ADDRESS: u16 = 1;
pub const IFLA_BROADCAST: u16 = 2;
pub const IFLA_IFNAME: u16 = 3;
pub const IFLA_MTU: u16 = 4;
pub const IFLA_LINK: u16 = 5;
pub const IFLA_QDISC: u16 = 6;
pub const IFLA_STATS: u16 = 7;
pub const IFLA_COST: u16 = 8;
pub const IFLA_PRIORITY: u16 = 9;
pub const IFLA_MASTER: u16 = 10;
pub const IFLA_WIRELESS: u16 = 11;
pub const IFLA_PROTINFO: u16 = 12;
pub const IFLA_TXQLEN: u16 = 13;
pub const IFLA_MAP: u16 = 14;
pub const IFLA_WEIGHT: u16 = 15;
pub const IFLA_OPERSTATE: u16 = 16;
pub const IFLA_LINKMODE: u16 = 17;
pub const IFLA_LINKINFO: u16 = 18;
pub const IFLA_NET_NS_PID: u16 = 19;
pub const IFLA_IFALIAS: u16 = 20;
pub const IFLA_NUM_VF: u16 = 21;
pub const IFLA_VFINFO_LIST: u16 = 22;
pub const IFLA_STATS64: u16 = 23;
pub const IFLA_VF_PORTS: u16 = 24;
pub const IFLA_PORT_SELF: u16 = 25;
pub const IFLA_AF_SPEC: u16 = 26;
pub const IFLA_GROUP: u16 = 27;
pub const IFLA_NET_NS_FD: u16 = 28;
pub const IFLA_EXT_MASK: u16 = 29;
pub const IFLA_PROMISCUITY: u16 = 30;
pub const IFLA_NUM_TX_QUEUES: u16 = 31;
pub const IFLA_NUM_RX_QUEUES: u16 = 32;
pub const IFLA_CARRIER: u16 = 33;
pub const IFLA_PHYS_PORT_ID: u16 = 34;
pub const IFLA_CARRIER_CHANGES: u16 = 35;
pub const IFLA_PHYS_SWITCH_ID: u16 = 36;
pub const IFLA_LINK_NETNSID: u16 = 37;
pub const IFLA_PHYS_PORT_NAME: u16 = 38;
pub const IFLA_PROTO_DOWN: u16 = 39;
pub const IFLA_GSO_MAX_SEGS: u16 = 40;
pub const IFLA_GSO_MAX_SIZE: u16 = 41;
pub const IFLA_PAD: u16 = 42;
pub const IFLA_XDP: u16 = 43;
pub const IFLA_EVENT: u16 = 44;
pub const IFLA_NEW_NETNSID: u16 = 45;
pub const IFLA_IF_NETNSID: u16 = 46;
pub const IFLA_CARRIER_UP_COUNT: u16 = 47;
pub const IFLA_CARRIER_DOWN_COUNT: u16 = 48;
pub const IFLA_NEW_IFINDEX: u16 = 49;
pub const IFLA_MIN_MTU: u16 = 50;
pub const IFLA_MAX_MTU: u16 = 51;
pub const IFLA_PROP_LIST: u16 = 52;
pub const IFLA_ALT_IFNAME: u16 = 53;
pub const IFLA_PERM_ADDRESS: u16 = 54;
pub const IFLA_PROTO_DOWN_REASON: u16 = 55;
pub const IFLA_INET_UNSPEC: u16 = 0;
pub const IFLA_INET_CONF: u16 = 1;
pub const IFLA_INET6_UNSPEC: u16 = 0;
pub const IFLA_INET6_FLAGS: u16 = 1;
pub const IFLA_INET6_CONF: u16 = 2;
pub const IFLA_INET6_STATS: u16 = 3;
// pub const IFLA_INET6_MCAST: u16 = 4;
pub const IFLA_INET6_CACHEINFO: u16 = 5;
pub const IFLA_INET6_ICMP6STATS: u16 = 6;
pub const IFLA_INET6_TOKEN: u16 = 7;
pub const IFLA_INET6_ADDR_GEN_MODE: u16 = 8;

/// Link is up (administratively).
pub const IFF_UP: u32 = libc::IFF_UP as u32;
/// Link is up and carrier is OK (RFC2863 OPER_UP)
pub const IFF_RUNNING: u32 = libc::IFF_RUNNING as u32;
/// Link layer is operational
pub const IFF_LOWER_UP: u32 = libc::IFF_LOWER_UP as u32;
/// Driver signals IFF_DORMANT
pub const IFF_DORMANT: u32 = libc::IFF_DORMANT as u32;
/// Link supports broadcasting
pub const IFF_BROADCAST: u32 = libc::IFF_BROADCAST as u32;
/// Link supports multicasting
pub const IFF_MULTICAST: u32 = libc::IFF_MULTICAST as u32;
/// Link supports multicast routing
pub const IFF_ALLMULTI: u32 = libc::IFF_ALLMULTI as u32;
/// Tell driver to do debugging (currently unused)
pub const IFF_DEBUG: u32 = libc::IFF_DEBUG as u32;
/// Link loopback network
pub const IFF_LOOPBACK: u32 = libc::IFF_LOOPBACK as u32;
/// u32erface is point-to-point link
pub const IFF_POINTOPOINT: u32 = libc::IFF_POINTOPOINT as u32;
/// ARP is not supported
pub const IFF_NOARP: u32 = libc::IFF_NOARP as u32;
/// Receive all packets.
pub const IFF_PROMISC: u32 = libc::IFF_PROMISC as u32;
/// Master of a load balancer (bonding)
pub const IFF_MASTER: u32 = libc::IFF_MASTER as u32;
/// Slave of a load balancer
pub const IFF_SLAVE: u32 = libc::IFF_SLAVE as u32;
/// Link selects port automatically (only used by ARM ethernet)
pub const IFF_PORTSEL: u32 = libc::IFF_PORTSEL as u32;
/// Driver supports setting media type (only used by ARM ethernet)
pub const IFF_AUTOMEDIA: u32 = libc::IFF_AUTOMEDIA as u32;
// /// Echo sent packets (testing feature, CAN only)
// pub const IFF_ECHO: u32 = libc::IFF_ECHO as u32;
// /// Dialup device with changing addresses (unused, BSD compatibility)
// pub const IFF_DYNAMIC: u32 = libc::IFF_DYNAMIC as u32;
// /// Avoid use of trailers (unused, BSD compatibility)
// pub const IFF_NOTRAILERS: u32 = libc::IFF_NOTRAILERS as u32;

pub const IF_OPER_UNKNOWN: u8 = 0;
pub const IF_OPER_NOTPRESENT: u8 = 1;
pub const IF_OPER_DOWN: u8 = 2;
pub const IF_OPER_LOWERLAYERDOWN: u8 = 3;
pub const IF_OPER_TESTING: u8 = 4;
pub const IF_OPER_DORMANT: u8 = 5;
pub const IF_OPER_UP: u8 = 6;

/// Neighbour cache entry type: unknown type
pub const NDA_UNSPEC: u16 = 0;
/// Neighbour cache entry type: entry for a network layer destination
/// address
pub const NDA_DST: u16 = 1;
/// Neighbour cache entry type: entry for a link layer destination
/// address
pub const NDA_LLADDR: u16 = 2;
/// Neighbour cache entry type: entry for cache statistics
pub const NDA_CACHEINFO: u16 = 3;
pub const NDA_PROBES: u16 = 4;
pub const NDA_VLAN: u16 = 5;
pub const NDA_PORT: u16 = 6;
pub const NDA_VNI: u16 = 7;
pub const NDA_IFINDEX: u16 = 8;
pub const NDA_MASTER: u16 = 9;
pub const NDA_LINK_NETNSID: u16 = 10;
pub const NDA_SRC_VNI: u16 = 11;

/// see `https://github.com/torvalds/linux/blob/master/include/uapi/linux/fib_rules.h`

pub const FR_ACT_UNSPEC: u8 = 0;
/// Pass to fixed table
pub const FR_ACT_TO_TBL: u8 = 1;
/// Jump to another rule
pub const FR_ACT_GOTO: u8 = 2;
/// No operation
pub const FR_ACT_NOP: u8 = 3;
pub const FR_ACT_RES3: u8 = 4;
pub const FR_ACT_RES4: u8 = 5;
/// Drop without notification
pub const FR_ACT_BLACKHOLE: u8 = 6;
/// Drop with `ENETUNREACH`
pub const FR_ACT_UNREACHABLE: u8 = 7;
/// Drop with `EACCES`
pub const FR_ACT_PROHIBIT: u8 = 8;

pub const FRA_UNSPEC: u16 = 0;
/// Destination address
pub const FRA_DST: u16 = 1;
/// Source address
pub const FRA_SRC: u16 = 2;
/// Interface name
pub const FRA_IIFNAME: u16 = 3;
/// Target to jump to
pub const FRA_GOTO: u16 = 4;

pub const FRA_UNUSED2: u16 = 5;

/// priority/preference
pub const FRA_PRIORITY: u16 = 6;

pub const FRA_UNUSED3: u16 = 7;
pub const FRA_UNUSED4: u16 = 8;
pub const FRA_UNUSED5: u16 = 9;

/// mark
pub const FRA_FWMARK: u16 = 10;
/// flow/class id
pub const FRA_FLOW: u16 = 11;
pub const FRA_TUN_ID: u16 = 12;
pub const FRA_SUPPRESS_IFGROUP: u16 = 13;
pub const FRA_SUPPRESS_PREFIXLEN: u16 = 14;
/// Extended table id
pub const FRA_TABLE: u16 = 15;
/// mask for netfilter mark
pub const FRA_FWMASK: u16 = 16;
pub const FRA_OIFNAME: u16 = 17;
pub const FRA_PAD: u16 = 18;
/// iif or oif is l3mdev goto its table
pub const FRA_L3MDEV: u16 = 19;
/// UID range
pub const FRA_UID_RANGE: u16 = 20;
/// Originator of the rule
pub const FRA_PROTOCOL: u16 = 21;
/// IP protocol
pub const FRA_IP_PROTO: u16 = 22;
/// Source port
pub const FRA_SPORT_RANGE: u16 = 23;
/// Destination port
pub const FRA_DPORT_RANGE: u16 = 24;

pub const FIB_RULE_PERMANENT: u32 = 1;
pub const FIB_RULE_INVERT: u32 = 2;
pub const FIB_RULE_UNRESOLVED: u32 = 4;
pub const FIB_RULE_IIF_DETACHED: u32 = 8;
pub const FIB_RULE_DEV_DETACHED: u32 = FIB_RULE_IIF_DETACHED;
pub const FIB_RULE_OIF_DETACHED: u32 = 10;
/// try to find source address in routing lookups
pub const FIB_RULE_FIND_SADDR: u32 = 10000;

// pub const MACVLAN_FLAG_NOPROMISC: int = 1;
// pub const IPVLAN_F_PRIVATE: int = 1;
// pub const IPVLAN_F_VEPA: int = 2;
// pub const MAX_VLAN_LIST_LEN: int = 1;
// pub const PORT_PROFILE_MAX: int = 40;
// pub const PORT_UUID_MAX: int = 16;
// pub const PORT_SELF_VF: int = -1;
// pub const XDP_FLAGS_UPDATE_IF_NOEXIST: int = 1;
// pub const XDP_FLAGS_SKB_MODE: int = 2;
// pub const XDP_FLAGS_DRV_MODE: int = 4;
// pub const XDP_FLAGS_HW_MODE: int = 8;
// pub const XDP_FLAGS_MODES: int = 14;
// pub const XDP_FLAGS_MASK: int = 15;

pub const IFA_F_SECONDARY: u32 = 1;
pub const IFA_F_TEMPORARY: u32 = 1;
pub const IFA_F_NODAD: u32 = 2;
pub const IFA_F_OPTIMISTIC: u32 = 4;
pub const IFA_F_DADFAILED: u32 = 8;
pub const IFA_F_HOMEADDRESS: u32 = 16;
pub const IFA_F_DEPRECATED: u32 = 32;
pub const IFA_F_TENTATIVE: u32 = 64;
pub const IFA_F_PERMANENT: u32 = 128;
pub const IFA_F_MANAGETEMPADDR: u32 = 256;
pub const IFA_F_NOPREFIXROUTE: u32 = 512;
pub const IFA_F_MCAUTOJOIN: u32 = 1024;
pub const IFA_F_STABLE_PRIVACY: u32 = 2048;

// pub const RTNL_FAMILY_IPMR: int = 128;
// pub const RTNL_FAMILY_IP6MR: int = 129;
// pub const RTNL_FAMILY_MAX: int = 129;
// pub const RTA_ALIGNTO: int = 4;
//
// pub const RTNH_F_DEAD: int = 1;
// pub const RTNH_F_PERVASIVE: int = 2;
// pub const RTNH_F_ONLINK: int = 4;
// pub const RTNH_F_OFFLOAD: int = 8;
// pub const RTNH_F_LINKDOWN: int = 16;
// pub const RTNH_F_UNRESOLVED: int = 32;
// pub const RTNH_COMPARE_MASK: int = 25;
// pub const RTNH_ALIGNTO: int = 4;
// pub const RTNETLINK_HAVE_PEERINFO: int = 1;
// pub const RTAX_FEATURE_ECN: int = 1;
// pub const RTAX_FEATURE_SACK: int = 2;
// pub const RTAX_FEATURE_TIMESTAMP: int = 4;
// pub const RTAX_FEATURE_ALLFRAG: int = 8;
// pub const RTAX_FEATURE_MASK: int = 15;
// #[allow(overflowing_literals)]
// pub const TCM_IFINDEX_MAGIC_BLOCK: int = 0xffff_ffff;
// pub const TCA_FLAG_LARGE_DUMP_ON: int = 1;

pub const RTEXT_FILTER_VF: u32 = 1;
pub const RTEXT_FILTER_BRVLAN: u32 = 2;
pub const RTEXT_FILTER_BRVLAN_COMPRESSED: u32 = 4;
pub const RTEXT_FILTER_SKIP_STATS: u32 = 8;

// pub const ARPOP_REQUEST: int = 1;
// pub const ARPOP_REPLY: int = 2;
//
// pub const IN6_ADDR_GEN_MODE_EUI64: int = 0;
// pub const IN6_ADDR_GEN_MODE_NONE: int = 1;
// pub const IN6_ADDR_GEN_MODE_STABLE_PRIVACY: int = 2;
// pub const IN6_ADDR_GEN_MODE_RANDOM: int = 3;
//
// pub const BRIDGE_MODE_UNSPEC: int = 0;
// pub const BRIDGE_MODE_HAIRPIN: int = 1;
//
// pub const IFLA_BRPORT_UNSPEC: int = 0;
// pub const IFLA_BRPORT_STATE: int = 1;
// pub const IFLA_BRPORT_PRIORITY: int = 2;
// pub const IFLA_BRPORT_COST: int = 3;
// pub const IFLA_BRPORT_MODE: int = 4;
// pub const IFLA_BRPORT_GUARD: int = 5;
// pub const IFLA_BRPORT_PROTECT: int = 6;
// pub const IFLA_BRPORT_FAST_LEAVE: int = 7;
// pub const IFLA_BRPORT_LEARNING: int = 8;
// pub const IFLA_BRPORT_UNICAST_FLOOD: int = 9;
// pub const IFLA_BRPORT_PROXYARP: int = 10;
// pub const IFLA_BRPORT_LEARNING_SYNC: int = 11;
// pub const IFLA_BRPORT_PROXYARP_WIFI: int = 12;
// pub const IFLA_BRPORT_ROOT_ID: int = 13;
// pub const IFLA_BRPORT_BRIDGE_ID: int = 14;
// pub const IFLA_BRPORT_DESIGNATED_PORT: int = 15;
// pub const IFLA_BRPORT_DESIGNATED_COST: int = 16;
// pub const IFLA_BRPORT_ID: int = 17;
// pub const IFLA_BRPORT_NO: int = 18;
// pub const IFLA_BRPORT_TOPOLOGY_CHANGE_ACK: int = 19;
// pub const IFLA_BRPORT_CONFIG_PENDING: int = 20;
// pub const IFLA_BRPORT_MESSAGE_AGE_TIMER: int = 21;
// pub const IFLA_BRPORT_FORWARD_DELAY_TIMER: int = 22;
// pub const IFLA_BRPORT_HOLD_TIMER: int = 23;
// pub const IFLA_BRPORT_FLUSH: int = 24;
// pub const IFLA_BRPORT_MULTICAST_ROUTER: int = 25;
// pub const IFLA_BRPORT_PAD: int = 26;
// pub const IFLA_BRPORT_MCAST_FLOOD: int = 27;
// pub const IFLA_BRPORT_MCAST_TO_UCAST: int = 28;
// pub const IFLA_BRPORT_VLAN_TUNNEL: int = 29;
// pub const IFLA_BRPORT_BCAST_FLOOD: int = 30;
// pub const IFLA_BRPORT_GROUP_FWD_MASK: int = 31;
// pub const IFLA_BRPORT_NEIGH_SUPPRESS: int = 32;
//
// pub const IFLA_VLAN_QOS_UNSPEC: int = 0;
// pub const IFLA_VLAN_QOS_MAPPING: int = 1;
//
// pub const IFLA_MACVLAN_UNSPEC: int = 0;
// pub const IFLA_MACVLAN_MODE: int = 1;
// pub const IFLA_MACVLAN_FLAGS: int = 2;
// pub const IFLA_MACVLAN_MACADDR_MODE: int = 3;
// pub const IFLA_MACVLAN_MACADDR: int = 4;
// pub const IFLA_MACVLAN_MACADDR_DATA: int = 5;
// pub const IFLA_MACVLAN_MACADDR_COUNT: int = 6;
//
// pub const MACVLAN_MODE_PRIVATE: int = 1;
// pub const MACVLAN_MODE_VEPA: int = 2;
// pub const MACVLAN_MODE_BRIDGE: int = 4;
// pub const MACVLAN_MODE_PASSTHRU: int = 8;
// pub const MACVLAN_MODE_SOURCE: int = 16;
//
// pub const MACVLAN_MACADDR_ADD: int = 0;
// pub const MACVLAN_MACADDR_DEL: int = 1;
// pub const MACVLAN_MACADDR_FLUSH: int = 2;
// pub const MACVLAN_MACADDR_SET: int = 3;
//
// pub const IFLA_VRF_UNSPEC: int = 0;
// pub const IFLA_VRF_TABLE: int = 1;
//
// pub const IFLA_VRF_PORT_UNSPEC: int = 0;
// pub const IFLA_VRF_PORT_TABLE: int = 1;
//
// pub const IFLA_MACSEC_UNSPEC: int = 0;
// pub const IFLA_MACSEC_SCI: int = 1;
// pub const IFLA_MACSEC_PORT: int = 2;
// pub const IFLA_MACSEC_ICV_LEN: int = 3;
// pub const IFLA_MACSEC_CIPHER_SUITE: int = 4;
// pub const IFLA_MACSEC_WINDOW: int = 5;
// pub const IFLA_MACSEC_ENCODING_SA: int = 6;
// pub const IFLA_MACSEC_ENCRYPT: int = 7;
// pub const IFLA_MACSEC_PROTECT: int = 8;
// pub const IFLA_MACSEC_INC_SCI: int = 9;
// pub const IFLA_MACSEC_ES: int = 10;
// pub const IFLA_MACSEC_SCB: int = 11;
// pub const IFLA_MACSEC_REPLAY_PROTECT: int = 12;
// pub const IFLA_MACSEC_VALIDATION: int = 13;
// pub const IFLA_MACSEC_PAD: int = 14;
//
// pub const MACSEC_VALIDATE_DISABLED: int = 0;
// pub const MACSEC_VALIDATE_CHECK: int = 1;
// pub const MACSEC_VALIDATE_STRICT: int = 2;
// pub const MACSEC_VALIDATE_MAX: int = 2;
//
// pub const IFLA_IPVLAN_UNSPEC: int = 0;
// pub const IFLA_IPVLAN_MODE: int = 1;
// pub const IFLA_IPVLAN_FLAGS: int = 2;
//
// pub const IPVLAN_MODE_L2: int = 0;
// pub const IPVLAN_MODE_L3: int = 1;
// pub const IPVLAN_MODE_L3S: int = 2;
// pub const IPVLAN_MODE_MAX: int = 3;
//
// FROM https://elixir.bootlin.com/linux/v5.9.8/source/include/uapi/linux/if_link.h#L531
pub const IFLA_VXLAN_UNSPEC: u16 = 0;
pub const IFLA_VXLAN_ID: u16 = 1;
pub const IFLA_VXLAN_GROUP: u16 = 2;
pub const IFLA_VXLAN_LINK: u16 = 3;
pub const IFLA_VXLAN_LOCAL: u16 = 4;
pub const IFLA_VXLAN_TTL: u16 = 5;
pub const IFLA_VXLAN_TOS: u16 = 6;
pub const IFLA_VXLAN_LEARNING: u16 = 7;
pub const IFLA_VXLAN_AGEING: u16 = 8;
pub const IFLA_VXLAN_LIMIT: u16 = 9;
pub const IFLA_VXLAN_PORT_RANGE: u16 = 10;
pub const IFLA_VXLAN_PROXY: u16 = 11;
pub const IFLA_VXLAN_RSC: u16 = 12;
pub const IFLA_VXLAN_L2MISS: u16 = 13;
pub const IFLA_VXLAN_L3MISS: u16 = 14;
pub const IFLA_VXLAN_PORT: u16 = 15;
pub const IFLA_VXLAN_GROUP6: u16 = 16;
pub const IFLA_VXLAN_LOCAL6: u16 = 17;
pub const IFLA_VXLAN_UDP_CSUM: u16 = 18;
pub const IFLA_VXLAN_UDP_ZERO_CSUM6_TX: u16 = 19;
pub const IFLA_VXLAN_UDP_ZERO_CSUM6_RX: u16 = 20;
pub const IFLA_VXLAN_REMCSUM_TX: u16 = 21;
pub const IFLA_VXLAN_REMCSUM_RX: u16 = 22;
pub const IFLA_VXLAN_GBP: u16 = 23;
pub const IFLA_VXLAN_REMCSUM_NOPARTIAL: u16 = 24;
pub const IFLA_VXLAN_COLLECT_METADATA: u16 = 25;
pub const IFLA_VXLAN_LABEL: u16 = 26;
pub const IFLA_VXLAN_GPE: u16 = 27;
pub const IFLA_VXLAN_TTL_INHERIT: u16 = 28;
pub const IFLA_VXLAN_DF: u16 = 29;
pub const __IFLA_VXLAN_MAX: u16 = 30;
//
// pub const IFLA_GENEVE_UNSPEC: int = 0;
// pub const IFLA_GENEVE_ID: int = 1;
// pub const IFLA_GENEVE_REMOTE: int = 2;
// pub const IFLA_GENEVE_TTL: int = 3;
// pub const IFLA_GENEVE_TOS: int = 4;
// pub const IFLA_GENEVE_PORT: int = 5;
// pub const IFLA_GENEVE_COLLECT_METADATA: int = 6;
// pub const IFLA_GENEVE_REMOTE6: int = 7;
// pub const IFLA_GENEVE_UDP_CSUM: int = 8;
// pub const IFLA_GENEVE_UDP_ZERO_CSUM6_TX: int = 9;
// pub const IFLA_GENEVE_UDP_ZERO_CSUM6_RX: int = 10;
// pub const IFLA_GENEVE_LABEL: int = 11;
//
// pub const IFLA_PPP_UNSPEC: int = 0;
// pub const IFLA_PPP_DEV_FD: int = 1;
//
// pub const GTP_ROLE_GGSN: int = 0;
// pub const GTP_ROLE_SGSN: int = 1;
//
// pub const IFLA_GTP_UNSPEC: int = 0;
// pub const IFLA_GTP_FD0: int = 1;
// pub const IFLA_GTP_FD1: int = 2;
// pub const IFLA_GTP_PDP_HASHSIZE: int = 3;
// pub const IFLA_GTP_ROLE: int = 4;
//
// pub const IFLA_BOND_UNSPEC: int = 0;
// pub const IFLA_BOND_MODE: int = 1;
// pub const IFLA_BOND_ACTIVE_SLAVE: int = 2;
// pub const IFLA_BOND_MIIMON: int = 3;
// pub const IFLA_BOND_UPDELAY: int = 4;
// pub const IFLA_BOND_DOWNDELAY: int = 5;
// pub const IFLA_BOND_USE_CARRIER: int = 6;
// pub const IFLA_BOND_ARP_INTERVAL: int = 7;
// pub const IFLA_BOND_ARP_IP_TARGET: int = 8;
// pub const IFLA_BOND_ARP_VALIDATE: int = 9;
// pub const IFLA_BOND_ARP_ALL_TARGETS: int = 10;
// pub const IFLA_BOND_PRIMARY: int = 11;
// pub const IFLA_BOND_PRIMARY_RESELECT: int = 12;
// pub const IFLA_BOND_FAIL_OVER_MAC: int = 13;
// pub const IFLA_BOND_XMIT_HASH_POLICY: int = 14;
// pub const IFLA_BOND_RESEND_IGMP: int = 15;
// pub const IFLA_BOND_NUM_PEER_NOTIF: int = 16;
// pub const IFLA_BOND_ALL_SLAVES_ACTIVE: int = 17;
// pub const IFLA_BOND_MIN_LINKS: int = 18;
// pub const IFLA_BOND_LP_INTERVAL: int = 19;
// pub const IFLA_BOND_PACKETS_PER_SLAVE: int = 20;
// pub const IFLA_BOND_AD_LACP_RATE: int = 21;
// pub const IFLA_BOND_AD_SELECT: int = 22;
// pub const IFLA_BOND_AD_INFO: int = 23;
// pub const IFLA_BOND_AD_ACTOR_SYS_PRIO: int = 24;
// pub const IFLA_BOND_AD_USER_PORT_KEY: int = 25;
// pub const IFLA_BOND_AD_ACTOR_SYSTEM: int = 26;
// pub const IFLA_BOND_TLB_DYNAMIC_LB: int = 27;
//
// pub const IFLA_BOND_AD_INFO_UNSPEC: int = 0;
// pub const IFLA_BOND_AD_INFO_AGGREGATOR: int = 1;
// pub const IFLA_BOND_AD_INFO_NUM_PORTS: int = 2;
// pub const IFLA_BOND_AD_INFO_ACTOR_KEY: int = 3;
// pub const IFLA_BOND_AD_INFO_PARTNER_KEY: int = 4;
// pub const IFLA_BOND_AD_INFO_PARTNER_MAC: int = 5;
//
// pub const IFLA_BOND_SLAVE_UNSPEC: int = 0;
// pub const IFLA_BOND_SLAVE_STATE: int = 1;
// pub const IFLA_BOND_SLAVE_MII_STATUS: int = 2;
// pub const IFLA_BOND_SLAVE_LINK_FAILURE_COUNT: int = 3;
// pub const IFLA_BOND_SLAVE_PERM_HWADDR: int = 4;
// pub const IFLA_BOND_SLAVE_QUEUE_ID: int = 5;
// pub const IFLA_BOND_SLAVE_AD_AGGREGATOR_ID: int = 6;
// pub const IFLA_BOND_SLAVE_AD_ACTOR_OPER_PORT_STATE: int = 7;
// pub const IFLA_BOND_SLAVE_AD_PARTNER_OPER_PORT_STATE: int = 8;
//
// pub const IFLA_VF_INFO_UNSPEC: int = 0;
// pub const IFLA_VF_INFO: int = 1;
//
// pub const IFLA_VF_UNSPEC: int = 0;
// pub const IFLA_VF_MAC: int = 1;
// pub const IFLA_VF_VLAN: int = 2;
// pub const IFLA_VF_TX_RATE: int = 3;
// pub const IFLA_VF_SPOOFCHK: int = 4;
// pub const IFLA_VF_LINK_STATE: int = 5;
// pub const IFLA_VF_RATE: int = 6;
// pub const IFLA_VF_RSS_QUERY_EN: int = 7;
// pub const IFLA_VF_STATS: int = 8;
// pub const IFLA_VF_TRUST: int = 9;
// pub const IFLA_VF_IB_NODE_GUID: int = 10;
// pub const IFLA_VF_IB_PORT_GUID: int = 11;
// pub const IFLA_VF_VLAN_LIST: int = 12;
//
// pub const IFLA_VF_VLAN_INFO_UNSPEC: int = 0;
// pub const IFLA_VF_VLAN_INFO: int = 1;
//
// pub const NDUSEROPT_UNSPEC: int = 0;
// pub const NDUSEROPT_SRCADDR: int = 1;
//
pub const RTNLGRP_NONE: u32 = 0;
pub const RTNLGRP_LINK: u32 = 1;
pub const RTNLGRP_NOTIFY: u32 = 2;
pub const RTNLGRP_NEIGH: u32 = 3;
pub const RTNLGRP_TC: u32 = 4;
pub const RTNLGRP_IPV4_IFADDR: u32 = 5;
pub const RTNLGRP_IPV4_MROUTE: u32 = 6;
pub const RTNLGRP_IPV4_ROUTE: u32 = 7;
pub const RTNLGRP_IPV4_RULE: u32 = 8;
pub const RTNLGRP_IPV6_IFADDR: u32 = 9;
pub const RTNLGRP_IPV6_MROUTE: u32 = 10;
pub const RTNLGRP_IPV6_ROUTE: u32 = 11;
pub const RTNLGRP_IPV6_IFINFO: u32 = 12;
pub const RTNLGRP_DECNET_IFADDR: u32 = 13;
pub const RTNLGRP_NOP2: u32 = 14;
pub const RTNLGRP_DECNET_ROUTE: u32 = 15;
pub const RTNLGRP_DECNET_RULE: u32 = 16;
pub const RTNLGRP_NOP4: u32 = 17;
pub const RTNLGRP_IPV6_PREFIX: u32 = 18;
pub const RTNLGRP_IPV6_RULE: u32 = 19;
pub const RTNLGRP_ND_USEROPT: u32 = 20;
pub const RTNLGRP_PHONET_IFADDR: u32 = 21;
pub const RTNLGRP_PHONET_ROUTE: u32 = 22;
pub const RTNLGRP_DCB: u32 = 23;
pub const RTNLGRP_IPV4_NETCONF: u32 = 24;
pub const RTNLGRP_IPV6_NETCONF: u32 = 25;
pub const RTNLGRP_MDB: u32 = 26;
pub const RTNLGRP_MPLS_ROUTE: u32 = 27;
pub const RTNLGRP_NSID: u32 = 28;
pub const RTNLGRP_MPLS_NETCONF: u32 = 29;
pub const RTNLGRP_IPV4_MROUTE_R: u32 = 30;
pub const RTNLGRP_IPV6_MROUTE_R: u32 = 31;
//
// pub const IFLA_VF_LINK_STATE_AUTO: int = 0;
// pub const IFLA_VF_LINK_STATE_ENABLE: int = 1;
// pub const IFLA_VF_LINK_STATE_DISABLE: int = 2;
//
// pub const IFLA_VF_STATS_RX_PACKETS: int = 0;
// pub const IFLA_VF_STATS_TX_PACKETS: int = 1;
// pub const IFLA_VF_STATS_RX_BYTES: int = 2;
// pub const IFLA_VF_STATS_TX_BYTES: int = 3;
// pub const IFLA_VF_STATS_BROADCAST: int = 4;
// pub const IFLA_VF_STATS_MULTICAST: int = 5;
// pub const IFLA_VF_STATS_PAD: int = 6;
// pub const IFLA_VF_STATS_RX_DROPPED: int = 7;
// pub const IFLA_VF_STATS_TX_DROPPED: int = 8;
//
// pub const IFLA_VF_PORT_UNSPEC: int = 0;
// pub const IFLA_VF_PORT: int = 1;
//
// pub const IFLA_PORT_UNSPEC: int = 0;
// pub const IFLA_PORT_VF: int = 1;
// pub const IFLA_PORT_PROFILE: int = 2;
// pub const IFLA_PORT_VSI_TYPE: int = 3;
// pub const IFLA_PORT_INSTANCE_UUID: int = 4;
// pub const IFLA_PORT_HOST_UUID: int = 5;
// pub const IFLA_PORT_REQUEST: int = 6;
// pub const IFLA_PORT_RESPONSE: int = 7;
//
// pub const PORT_REQUEST_PREASSOCIATE: int = 0;
// pub const PORT_REQUEST_PREASSOCIATE_RR: int = 1;
// pub const PORT_REQUEST_ASSOCIATE: int = 2;
// pub const PORT_REQUEST_DISASSOCIATE: int = 3;
//
// pub const PORT_VDP_RESPONSE_SUCCESS: int = 0;
// pub const PORT_VDP_RESPONSE_INVALID_FORMAT: int = 1;
// pub const PORT_VDP_RESPONSE_INSUFFICIENT_RESOURCES: int = 2;
// pub const PORT_VDP_RESPONSE_UNUSED_VTID: int = 3;
// pub const PORT_VDP_RESPONSE_VTID_VIOLATION: int = 4;
// pub const PORT_VDP_RESPONSE_VTID_VERSION_VIOALTION: int = 5;
// pub const PORT_VDP_RESPONSE_OUT_OF_SYNC: int = 6;
// pub const PORT_PROFILE_RESPONSE_SUCCESS: int = 256;
// pub const PORT_PROFILE_RESPONSE_INPROGRESS: int = 257;
// pub const PORT_PROFILE_RESPONSE_INVALID: int = 258;
// pub const PORT_PROFILE_RESPONSE_BADSTATE: int = 259;
// pub const PORT_PROFILE_RESPONSE_INSUFFICIENT_RESOURCES: int = 260;
// pub const PORT_PROFILE_RESPONSE_ERROR: int = 261;
//
// pub const IFLA_IPOIB_UNSPEC: int = 0;
// pub const IFLA_IPOIB_PKEY: int = 1;
// pub const IFLA_IPOIB_MODE: int = 2;
// pub const IFLA_IPOIB_UMCAST: int = 3;
//
// pub const IPOIB_MODE_DATAGRAM: int = 0;
// pub const IPOIB_MODE_CONNECTED: int = 1;
//
// pub const IFLA_HSR_UNSPEC: int = 0;
// pub const IFLA_HSR_SLAVE1: int = 1;
// pub const IFLA_HSR_SLAVE2: int = 2;
// pub const IFLA_HSR_MULTICAST_SPEC: int = 3;
// pub const IFLA_HSR_SUPERVISION_ADDR: int = 4;
// pub const IFLA_HSR_SEQ_NR: int = 5;
// pub const IFLA_HSR_VERSION: int = 6;
//
// pub const IFLA_STATS_UNSPEC: int = 0;
// pub const IFLA_STATS_LINK_64: int = 1;
// pub const IFLA_STATS_LINK_XSTATS: int = 2;
// pub const IFLA_STATS_LINK_XSTATS_SLAVE: int = 3;
// pub const IFLA_STATS_LINK_OFFLOAD_XSTATS: int = 4;
// pub const IFLA_STATS_AF_SPEC: int = 5;
//
// pub const LINK_XSTATS_TYPE_UNSPEC: int = 0;
// pub const LINK_XSTATS_TYPE_BRIDGE: int = 1;
//
// pub const IFLA_OFFLOAD_XSTATS_UNSPEC: int = 0;
// pub const IFLA_OFFLOAD_XSTATS_CPU_HIT: int = 1;
//
// pub const XDP_ATTACHED_NONE: int = 0;
// pub const XDP_ATTACHED_DRV: int = 1;
// pub const XDP_ATTACHED_SKB: int = 2;
// pub const XDP_ATTACHED_HW: int = 3;

pub const IFLA_XDP_UNSPEC: u32 = 0;
pub const IFLA_XDP_FD: u32 = 1;
pub const IFLA_XDP_ATTACHED: u32 = 2;
pub const IFLA_XDP_FLAGS: u32 = 3;
pub const IFLA_XDP_PROG_ID: u32 = 4;

// pub const IFLA_EVENT_NONE: int = 0;
// pub const IFLA_EVENT_REBOOT: int = 1;
// pub const IFLA_EVENT_FEATURES: int = 2;
// pub const IFLA_EVENT_BONDING_FAILOVER: int = 3;
// pub const IFLA_EVENT_NOTIFY_PEERS: int = 4;
// pub const IFLA_EVENT_IGMP_RESEND: int = 5;
// pub const IFLA_EVENT_BONDING_OPTIONS: int = 6;
//
// pub const NDTPA_UNSPEC: int = 0;
// pub const NDTPA_IFINDEX: int = 1;
// pub const NDTPA_REFCNT: int = 2;
// pub const NDTPA_REACHABLE_TIME: int = 3;
// pub const NDTPA_BASE_REACHABLE_TIME: int = 4;
// pub const NDTPA_RETRANS_TIME: int = 5;
// pub const NDTPA_GC_STALETIME: int = 6;
// pub const NDTPA_DELAY_PROBE_TIME: int = 7;
// pub const NDTPA_QUEUE_LEN: int = 8;
// pub const NDTPA_APP_PROBES: int = 9;
// pub const NDTPA_UCAST_PROBES: int = 10;
// pub const NDTPA_MCAST_PROBES: int = 11;
// pub const NDTPA_ANYCAST_DELAY: int = 12;
// pub const NDTPA_PROXY_DELAY: int = 13;
// pub const NDTPA_PROXY_QLEN: int = 14;
// pub const NDTPA_LOCKTIME: int = 15;
// pub const NDTPA_QUEUE_LENBYTES: int = 16;
// pub const NDTPA_MCAST_REPROBES: int = 17;
// pub const NDTPA_PAD: int = 18;
//
// #[allow(overflowing_literals)]
// pub const RT_TABLE_MAX: int = 0xffff_ffff;
//
// pub const PREFIX_UNSPEC: int = 0;
// pub const PREFIX_ADDRESS: int = 1;
// pub const PREFIX_CACHEINFO: int = 2;
