#![allow(unused)]

use libc::c_int as int;

pub const TCA_ROOT_UNSPEC: int = 0;
pub const TCA_ROOT_TAB: int = 1;
pub const TCA_ROOT_FLAGS: int = 2;
pub const TCA_ROOT_COUNT: int = 3;
pub const TCA_ROOT_TIME_DELTA: int = 4;

pub const EM_NONE: u32 = 0;
pub const EM_M32: u32 = 1;
pub const EM_SPARC: u32 = 2;
pub const EM_386: u32 = 3;
pub const EM_68K: u32 = 4;
pub const EM_88K: u32 = 5;
pub const EM_486: u32 = 6;
pub const EM_860: u32 = 7;
pub const EM_MIPS: u32 = 8;
pub const EM_MIPS_RS3_LE: u32 = 10;
pub const EM_MIPS_RS4_BE: u32 = 10;
pub const EM_PARISC: u32 = 15;
pub const EM_SPARC32PLUS: u32 = 18;
pub const EM_PPC: u32 = 20;
pub const EM_PPC64: u32 = 21;
pub const EM_SPU: u32 = 23;
pub const EM_ARM: u32 = 40;
pub const EM_SH: u32 = 42;
pub const EM_SPARCV9: u32 = 43;
pub const EM_H8_300: u32 = 46;
pub const EM_IA_64: u32 = 50;
pub const EM_X86_64: u32 = 62;
pub const EM_S390: u32 = 22;
pub const EM_CRIS: u32 = 76;
pub const EM_M32R: u32 = 88;
pub const EM_MN10300: u32 = 89;
pub const EM_OPENRISC: u32 = 92;
pub const EM_BLACKFIN: u32 = 106;
pub const EM_ALTERA_NIOS2: u32 = 113;
pub const EM_TI_C6000: u32 = 140;
pub const EM_AARCH64: u32 = 183;
pub const EM_TILEPRO: u32 = 188;
pub const EM_MICROBLAZE: u32 = 189;
pub const EM_TILEGX: u32 = 191;
pub const EM_BPF: u32 = 247;
pub const EM_FRV: u32 = 21569;
pub const EM_ALPHA: u32 = 36902;
pub const EM_CYGNUS_M32R: u32 = 36929;
pub const EM_S390_OLD: u32 = 41872;
pub const EM_CYGNUS_MN10300: u32 = 48879;

pub const NLMSGERR_ATTR_UNUSED: int = 0;
pub const NLMSGERR_ATTR_MSG: int = 1;
pub const NLMSGERR_ATTR_OFFS: int = 2;
pub const NLMSGERR_ATTR_COOKIE: int = 3;
pub const NLMSGERR_ATTR_MAX: int = 3;

pub const NL_MMAP_STATUS_UNUSED: int = 0;
pub const NL_MMAP_STATUS_RESERVED: int = 1;
pub const NL_MMAP_STATUS_VALID: int = 2;
pub const NL_MMAP_STATUS_COPY: int = 3;
pub const NL_MMAP_STATUS_SKIP: int = 4;

pub const NETLINK_UNCONNECTED: int = 0;
pub const NETLINK_CONNECTED: int = 1;

pub const IN6_ADDR_GEN_MODE_EUI64: int = 0;
pub const IN6_ADDR_GEN_MODE_NONE: int = 1;
pub const IN6_ADDR_GEN_MODE_STABLE_PRIVACY: int = 2;
pub const IN6_ADDR_GEN_MODE_RANDOM: int = 3;

pub const BRIDGE_MODE_UNSPEC: int = 0;
pub const BRIDGE_MODE_HAIRPIN: int = 1;

pub const IFLA_BRPORT_UNSPEC: int = 0;
pub const IFLA_BRPORT_STATE: int = 1;
pub const IFLA_BRPORT_PRIORITY: int = 2;
pub const IFLA_BRPORT_COST: int = 3;
pub const IFLA_BRPORT_MODE: int = 4;
pub const IFLA_BRPORT_GUARD: int = 5;
pub const IFLA_BRPORT_PROTECT: int = 6;
pub const IFLA_BRPORT_FAST_LEAVE: int = 7;
pub const IFLA_BRPORT_LEARNING: int = 8;
pub const IFLA_BRPORT_UNICAST_FLOOD: int = 9;
pub const IFLA_BRPORT_PROXYARP: int = 10;
pub const IFLA_BRPORT_LEARNING_SYNC: int = 11;
pub const IFLA_BRPORT_PROXYARP_WIFI: int = 12;
pub const IFLA_BRPORT_ROOT_ID: int = 13;
pub const IFLA_BRPORT_BRIDGE_ID: int = 14;
pub const IFLA_BRPORT_DESIGNATED_PORT: int = 15;
pub const IFLA_BRPORT_DESIGNATED_COST: int = 16;
pub const IFLA_BRPORT_ID: int = 17;
pub const IFLA_BRPORT_NO: int = 18;
pub const IFLA_BRPORT_TOPOLOGY_CHANGE_ACK: int = 19;
pub const IFLA_BRPORT_CONFIG_PENDING: int = 20;
pub const IFLA_BRPORT_MESSAGE_AGE_TIMER: int = 21;
pub const IFLA_BRPORT_FORWARD_DELAY_TIMER: int = 22;
pub const IFLA_BRPORT_HOLD_TIMER: int = 23;
pub const IFLA_BRPORT_FLUSH: int = 24;
pub const IFLA_BRPORT_MULTICAST_ROUTER: int = 25;
pub const IFLA_BRPORT_PAD: int = 26;
pub const IFLA_BRPORT_MCAST_FLOOD: int = 27;
pub const IFLA_BRPORT_MCAST_TO_UCAST: int = 28;
pub const IFLA_BRPORT_VLAN_TUNNEL: int = 29;
pub const IFLA_BRPORT_BCAST_FLOOD: int = 30;
pub const IFLA_BRPORT_GROUP_FWD_MASK: int = 31;
pub const IFLA_BRPORT_NEIGH_SUPPRESS: int = 32;

pub const IFLA_VLAN_QOS_UNSPEC: int = 0;
pub const IFLA_VLAN_QOS_MAPPING: int = 1;

pub const IFLA_MACVLAN_UNSPEC: int = 0;
pub const IFLA_MACVLAN_MODE: int = 1;
pub const IFLA_MACVLAN_FLAGS: int = 2;
pub const IFLA_MACVLAN_MACADDR_MODE: int = 3;
pub const IFLA_MACVLAN_MACADDR: int = 4;
pub const IFLA_MACVLAN_MACADDR_DATA: int = 5;
pub const IFLA_MACVLAN_MACADDR_COUNT: int = 6;

pub const MACVLAN_MODE_PRIVATE: int = 1;
pub const MACVLAN_MODE_VEPA: int = 2;
pub const MACVLAN_MODE_BRIDGE: int = 4;
pub const MACVLAN_MODE_PASSTHRU: int = 8;
pub const MACVLAN_MODE_SOURCE: int = 16;

pub const MACVLAN_MACADDR_ADD: int = 0;
pub const MACVLAN_MACADDR_DEL: int = 1;
pub const MACVLAN_MACADDR_FLUSH: int = 2;
pub const MACVLAN_MACADDR_SET: int = 3;

pub const IFLA_VRF_UNSPEC: int = 0;
pub const IFLA_VRF_TABLE: int = 1;

pub const IFLA_VRF_PORT_UNSPEC: int = 0;
pub const IFLA_VRF_PORT_TABLE: int = 1;

pub const IFLA_MACSEC_UNSPEC: int = 0;
pub const IFLA_MACSEC_SCI: int = 1;
pub const IFLA_MACSEC_PORT: int = 2;
pub const IFLA_MACSEC_ICV_LEN: int = 3;
pub const IFLA_MACSEC_CIPHER_SUITE: int = 4;
pub const IFLA_MACSEC_WINDOW: int = 5;
pub const IFLA_MACSEC_ENCODING_SA: int = 6;
pub const IFLA_MACSEC_ENCRYPT: int = 7;
pub const IFLA_MACSEC_PROTECT: int = 8;
pub const IFLA_MACSEC_INC_SCI: int = 9;
pub const IFLA_MACSEC_ES: int = 10;
pub const IFLA_MACSEC_SCB: int = 11;
pub const IFLA_MACSEC_REPLAY_PROTECT: int = 12;
pub const IFLA_MACSEC_VALIDATION: int = 13;
pub const IFLA_MACSEC_PAD: int = 14;

pub const MACSEC_VALIDATE_DISABLED: int = 0;
pub const MACSEC_VALIDATE_CHECK: int = 1;
pub const MACSEC_VALIDATE_STRICT: int = 2;
pub const MACSEC_VALIDATE_MAX: int = 2;

pub const IFLA_IPVLAN_UNSPEC: int = 0;
pub const IFLA_IPVLAN_MODE: int = 1;
pub const IFLA_IPVLAN_FLAGS: int = 2;

pub const IPVLAN_MODE_L2: int = 0;
pub const IPVLAN_MODE_L3: int = 1;
pub const IPVLAN_MODE_L3S: int = 2;
pub const IPVLAN_MODE_MAX: int = 3;

pub const IFLA_VXLAN_UNSPEC: int = 0;
pub const IFLA_VXLAN_ID: int = 1;
pub const IFLA_VXLAN_GROUP: int = 2;
pub const IFLA_VXLAN_LINK: int = 3;
pub const IFLA_VXLAN_LOCAL: int = 4;
pub const IFLA_VXLAN_TTL: int = 5;
pub const IFLA_VXLAN_TOS: int = 6;
pub const IFLA_VXLAN_LEARNING: int = 7;
pub const IFLA_VXLAN_AGEING: int = 8;
pub const IFLA_VXLAN_LIMIT: int = 9;
pub const IFLA_VXLAN_PORT_RANGE: int = 10;
pub const IFLA_VXLAN_PROXY: int = 11;
pub const IFLA_VXLAN_RSC: int = 12;
pub const IFLA_VXLAN_L2MISS: int = 13;
pub const IFLA_VXLAN_L3MISS: int = 14;
pub const IFLA_VXLAN_PORT: int = 15;
pub const IFLA_VXLAN_GROUP6: int = 16;
pub const IFLA_VXLAN_LOCAL6: int = 17;
pub const IFLA_VXLAN_UDP_CSUM: int = 18;
pub const IFLA_VXLAN_UDP_ZERO_CSUM6_TX: int = 19;
pub const IFLA_VXLAN_UDP_ZERO_CSUM6_RX: int = 20;
pub const IFLA_VXLAN_REMCSUM_TX: int = 21;
pub const IFLA_VXLAN_REMCSUM_RX: int = 22;
pub const IFLA_VXLAN_GBP: int = 23;
pub const IFLA_VXLAN_REMCSUM_NOPARTIAL: int = 24;
pub const IFLA_VXLAN_COLLECT_METADATA: int = 25;
pub const IFLA_VXLAN_LABEL: int = 26;
pub const IFLA_VXLAN_GPE: int = 27;

pub const IFLA_GENEVE_UNSPEC: int = 0;
pub const IFLA_GENEVE_ID: int = 1;
pub const IFLA_GENEVE_REMOTE: int = 2;
pub const IFLA_GENEVE_TTL: int = 3;
pub const IFLA_GENEVE_TOS: int = 4;
pub const IFLA_GENEVE_PORT: int = 5;
pub const IFLA_GENEVE_COLLECT_METADATA: int = 6;
pub const IFLA_GENEVE_REMOTE6: int = 7;
pub const IFLA_GENEVE_UDP_CSUM: int = 8;
pub const IFLA_GENEVE_UDP_ZERO_CSUM6_TX: int = 9;
pub const IFLA_GENEVE_UDP_ZERO_CSUM6_RX: int = 10;
pub const IFLA_GENEVE_LABEL: int = 11;

pub const IFLA_PPP_UNSPEC: int = 0;
pub const IFLA_PPP_DEV_FD: int = 1;

pub const GTP_ROLE_GGSN: int = 0;
pub const GTP_ROLE_SGSN: int = 1;

pub const IFLA_GTP_UNSPEC: int = 0;
pub const IFLA_GTP_FD0: int = 1;
pub const IFLA_GTP_FD1: int = 2;
pub const IFLA_GTP_PDP_HASHSIZE: int = 3;
pub const IFLA_GTP_ROLE: int = 4;

pub const IFLA_BOND_UNSPEC: int = 0;
pub const IFLA_BOND_MODE: int = 1;
pub const IFLA_BOND_ACTIVE_SLAVE: int = 2;
pub const IFLA_BOND_MIIMON: int = 3;
pub const IFLA_BOND_UPDELAY: int = 4;
pub const IFLA_BOND_DOWNDELAY: int = 5;
pub const IFLA_BOND_USE_CARRIER: int = 6;
pub const IFLA_BOND_ARP_INTERVAL: int = 7;
pub const IFLA_BOND_ARP_IP_TARGET: int = 8;
pub const IFLA_BOND_ARP_VALIDATE: int = 9;
pub const IFLA_BOND_ARP_ALL_TARGETS: int = 10;
pub const IFLA_BOND_PRIMARY: int = 11;
pub const IFLA_BOND_PRIMARY_RESELECT: int = 12;
pub const IFLA_BOND_FAIL_OVER_MAC: int = 13;
pub const IFLA_BOND_XMIT_HASH_POLICY: int = 14;
pub const IFLA_BOND_RESEND_IGMP: int = 15;
pub const IFLA_BOND_NUM_PEER_NOTIF: int = 16;
pub const IFLA_BOND_ALL_SLAVES_ACTIVE: int = 17;
pub const IFLA_BOND_MIN_LINKS: int = 18;
pub const IFLA_BOND_LP_INTERVAL: int = 19;
pub const IFLA_BOND_PACKETS_PER_SLAVE: int = 20;
pub const IFLA_BOND_AD_LACP_RATE: int = 21;
pub const IFLA_BOND_AD_SELECT: int = 22;
pub const IFLA_BOND_AD_INFO: int = 23;
pub const IFLA_BOND_AD_ACTOR_SYS_PRIO: int = 24;
pub const IFLA_BOND_AD_USER_PORT_KEY: int = 25;
pub const IFLA_BOND_AD_ACTOR_SYSTEM: int = 26;
pub const IFLA_BOND_TLB_DYNAMIC_LB: int = 27;

pub const IFLA_BOND_AD_INFO_UNSPEC: int = 0;
pub const IFLA_BOND_AD_INFO_AGGREGATOR: int = 1;
pub const IFLA_BOND_AD_INFO_NUM_PORTS: int = 2;
pub const IFLA_BOND_AD_INFO_ACTOR_KEY: int = 3;
pub const IFLA_BOND_AD_INFO_PARTNER_KEY: int = 4;
pub const IFLA_BOND_AD_INFO_PARTNER_MAC: int = 5;

pub const IFLA_BOND_SLAVE_UNSPEC: int = 0;
pub const IFLA_BOND_SLAVE_STATE: int = 1;
pub const IFLA_BOND_SLAVE_MII_STATUS: int = 2;
pub const IFLA_BOND_SLAVE_LINK_FAILURE_COUNT: int = 3;
pub const IFLA_BOND_SLAVE_PERM_HWADDR: int = 4;
pub const IFLA_BOND_SLAVE_QUEUE_ID: int = 5;
pub const IFLA_BOND_SLAVE_AD_AGGREGATOR_ID: int = 6;
pub const IFLA_BOND_SLAVE_AD_ACTOR_OPER_PORT_STATE: int = 7;
pub const IFLA_BOND_SLAVE_AD_PARTNER_OPER_PORT_STATE: int = 8;

pub const IFLA_VF_INFO_UNSPEC: int = 0;
pub const IFLA_VF_INFO: int = 1;

pub const IFLA_VF_UNSPEC: int = 0;
pub const IFLA_VF_MAC: int = 1;
pub const IFLA_VF_VLAN: int = 2;
pub const IFLA_VF_TX_RATE: int = 3;
pub const IFLA_VF_SPOOFCHK: int = 4;
pub const IFLA_VF_LINK_STATE: int = 5;
pub const IFLA_VF_RATE: int = 6;
pub const IFLA_VF_RSS_QUERY_EN: int = 7;
pub const IFLA_VF_STATS: int = 8;
pub const IFLA_VF_TRUST: int = 9;
pub const IFLA_VF_IB_NODE_GUID: int = 10;
pub const IFLA_VF_IB_PORT_GUID: int = 11;
pub const IFLA_VF_VLAN_LIST: int = 12;

pub const IFLA_VF_VLAN_INFO_UNSPEC: int = 0;
pub const IFLA_VF_VLAN_INFO: int = 1;

pub const NDUSEROPT_UNSPEC: int = 0;
pub const NDUSEROPT_SRCADDR: int = 1;

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

pub const IFLA_VF_LINK_STATE_AUTO: int = 0;
pub const IFLA_VF_LINK_STATE_ENABLE: int = 1;
pub const IFLA_VF_LINK_STATE_DISABLE: int = 2;

pub const IFLA_VF_STATS_RX_PACKETS: int = 0;
pub const IFLA_VF_STATS_TX_PACKETS: int = 1;
pub const IFLA_VF_STATS_RX_BYTES: int = 2;
pub const IFLA_VF_STATS_TX_BYTES: int = 3;
pub const IFLA_VF_STATS_BROADCAST: int = 4;
pub const IFLA_VF_STATS_MULTICAST: int = 5;
pub const IFLA_VF_STATS_PAD: int = 6;
pub const IFLA_VF_STATS_RX_DROPPED: int = 7;
pub const IFLA_VF_STATS_TX_DROPPED: int = 8;

pub const IFLA_VF_PORT_UNSPEC: int = 0;
pub const IFLA_VF_PORT: int = 1;

pub const IFLA_PORT_UNSPEC: int = 0;
pub const IFLA_PORT_VF: int = 1;
pub const IFLA_PORT_PROFILE: int = 2;
pub const IFLA_PORT_VSI_TYPE: int = 3;
pub const IFLA_PORT_INSTANCE_UUID: int = 4;
pub const IFLA_PORT_HOST_UUID: int = 5;
pub const IFLA_PORT_REQUEST: int = 6;
pub const IFLA_PORT_RESPONSE: int = 7;

pub const PORT_REQUEST_PREASSOCIATE: int = 0;
pub const PORT_REQUEST_PREASSOCIATE_RR: int = 1;
pub const PORT_REQUEST_ASSOCIATE: int = 2;
pub const PORT_REQUEST_DISASSOCIATE: int = 3;

pub const PORT_VDP_RESPONSE_SUCCESS: int = 0;
pub const PORT_VDP_RESPONSE_INVALID_FORMAT: int = 1;
pub const PORT_VDP_RESPONSE_INSUFFICIENT_RESOURCES: int = 2;
pub const PORT_VDP_RESPONSE_UNUSED_VTID: int = 3;
pub const PORT_VDP_RESPONSE_VTID_VIOLATION: int = 4;
pub const PORT_VDP_RESPONSE_VTID_VERSION_VIOALTION: int = 5;
pub const PORT_VDP_RESPONSE_OUT_OF_SYNC: int = 6;
pub const PORT_PROFILE_RESPONSE_SUCCESS: int = 256;
pub const PORT_PROFILE_RESPONSE_INPROGRESS: int = 257;
pub const PORT_PROFILE_RESPONSE_INVALID: int = 258;
pub const PORT_PROFILE_RESPONSE_BADSTATE: int = 259;
pub const PORT_PROFILE_RESPONSE_INSUFFICIENT_RESOURCES: int = 260;
pub const PORT_PROFILE_RESPONSE_ERROR: int = 261;

pub const IFLA_IPOIB_UNSPEC: int = 0;
pub const IFLA_IPOIB_PKEY: int = 1;
pub const IFLA_IPOIB_MODE: int = 2;
pub const IFLA_IPOIB_UMCAST: int = 3;

pub const IPOIB_MODE_DATAGRAM: int = 0;
pub const IPOIB_MODE_CONNECTED: int = 1;

pub const IFLA_HSR_UNSPEC: int = 0;
pub const IFLA_HSR_SLAVE1: int = 1;
pub const IFLA_HSR_SLAVE2: int = 2;
pub const IFLA_HSR_MULTICAST_SPEC: int = 3;
pub const IFLA_HSR_SUPERVISION_ADDR: int = 4;
pub const IFLA_HSR_SEQ_NR: int = 5;
pub const IFLA_HSR_VERSION: int = 6;

pub const IFLA_STATS_UNSPEC: int = 0;
pub const IFLA_STATS_LINK_64: int = 1;
pub const IFLA_STATS_LINK_XSTATS: int = 2;
pub const IFLA_STATS_LINK_XSTATS_SLAVE: int = 3;
pub const IFLA_STATS_LINK_OFFLOAD_XSTATS: int = 4;
pub const IFLA_STATS_AF_SPEC: int = 5;

pub const LINK_XSTATS_TYPE_UNSPEC: int = 0;
pub const LINK_XSTATS_TYPE_BRIDGE: int = 1;

pub const IFLA_OFFLOAD_XSTATS_UNSPEC: int = 0;
pub const IFLA_OFFLOAD_XSTATS_CPU_HIT: int = 1;

pub const XDP_ATTACHED_NONE: int = 0;
pub const XDP_ATTACHED_DRV: int = 1;
pub const XDP_ATTACHED_SKB: int = 2;
pub const XDP_ATTACHED_HW: int = 3;

pub const IFLA_XDP_UNSPEC: int = 0;
pub const IFLA_XDP_FD: int = 1;
pub const IFLA_XDP_ATTACHED: int = 2;
pub const IFLA_XDP_FLAGS: int = 3;
pub const IFLA_XDP_PROG_ID: int = 4;

pub const IFLA_EVENT_NONE: int = 0;
pub const IFLA_EVENT_REBOOT: int = 1;
pub const IFLA_EVENT_FEATURES: int = 2;
pub const IFLA_EVENT_BONDING_FAILOVER: int = 3;
pub const IFLA_EVENT_NOTIFY_PEERS: int = 4;
pub const IFLA_EVENT_IGMP_RESEND: int = 5;
pub const IFLA_EVENT_BONDING_OPTIONS: int = 6;

pub const NDTPA_UNSPEC: int = 0;
pub const NDTPA_IFINDEX: int = 1;
pub const NDTPA_REFCNT: int = 2;
pub const NDTPA_REACHABLE_TIME: int = 3;
pub const NDTPA_BASE_REACHABLE_TIME: int = 4;
pub const NDTPA_RETRANS_TIME: int = 5;
pub const NDTPA_GC_STALETIME: int = 6;
pub const NDTPA_DELAY_PROBE_TIME: int = 7;
pub const NDTPA_QUEUE_LEN: int = 8;
pub const NDTPA_APP_PROBES: int = 9;
pub const NDTPA_UCAST_PROBES: int = 10;
pub const NDTPA_MCAST_PROBES: int = 11;
pub const NDTPA_ANYCAST_DELAY: int = 12;
pub const NDTPA_PROXY_DELAY: int = 13;
pub const NDTPA_PROXY_QLEN: int = 14;
pub const NDTPA_LOCKTIME: int = 15;
pub const NDTPA_QUEUE_LENBYTES: int = 16;
pub const NDTPA_MCAST_REPROBES: int = 17;
pub const NDTPA_PAD: int = 18;

#[allow(overflowing_literals)]
pub const RT_TABLE_MAX: int = 0xffff_ffff;

pub const PREFIX_UNSPEC: int = 0;
pub const PREFIX_ADDRESS: int = 1;
pub const PREFIX_CACHEINFO: int = 2;

pub const __BITS_PER_LONG: int = 64;
pub const __FD_SETSIZE: int = 1024;
pub const SI_LOAD_SHIFT: int = 16;
pub const _K_SS_MAXSIZE: int = 128;
pub const NETLINK_SMC: int = 22;
pub const NETLINK_INET_DIAG: int = 4;
pub const MAX_LINKS: int = 32;

pub const NLMSG_MIN_TYPE: int = 16;
pub const NETLINK_ADD_MEMBERSHIP: int = 1;
pub const NETLINK_DROP_MEMBERSHIP: int = 2;
pub const NETLINK_PKTINFO: int = 3;
pub const NETLINK_BROADCAST_ERROR: int = 4;
pub const NETLINK_NO_ENOBUFS: int = 5;
pub const NETLINK_RX_RING: int = 6;
pub const NETLINK_TX_RING: int = 7;
pub const NETLINK_LISTEN_ALL_NSID: int = 8;
pub const NETLINK_LIST_MEMBERSHIPS: int = 9;
pub const NETLINK_CAP_ACK: int = 10;
pub const NETLINK_EXT_ACK: int = 11;
pub const NL_MMAP_MSG_ALIGNMENT: int = 4;
pub const NET_MAJOR: int = 36;

pub const MACVLAN_FLAG_NOPROMISC: int = 1;
pub const IPVLAN_F_PRIVATE: int = 1;
pub const IPVLAN_F_VEPA: int = 2;
pub const MAX_VLAN_LIST_LEN: int = 1;
pub const PORT_PROFILE_MAX: int = 40;
pub const PORT_UUID_MAX: int = 16;
pub const PORT_SELF_VF: int = -1;
pub const XDP_FLAGS_UPDATE_IF_NOEXIST: int = 1;
pub const XDP_FLAGS_SKB_MODE: int = 2;
pub const XDP_FLAGS_DRV_MODE: int = 4;
pub const XDP_FLAGS_HW_MODE: int = 8;
pub const XDP_FLAGS_MODES: int = 14;
pub const XDP_FLAGS_MASK: int = 15;
pub const IFA_F_SECONDARY: int = 1;
pub const IFA_F_TEMPORARY: int = 1;
pub const IFA_F_NODAD: int = 2;
pub const IFA_F_OPTIMISTIC: int = 4;
pub const IFA_F_DADFAILED: int = 8;
pub const IFA_F_HOMEADDRESS: int = 16;
pub const IFA_F_DEPRECATED: int = 32;
pub const IFA_F_TENTATIVE: int = 64;
pub const IFA_F_PERMANENT: int = 128;
pub const IFA_F_MANAGETEMPADDR: int = 256;
pub const IFA_F_NOPREFIXROUTE: int = 512;
pub const IFA_F_MCAUTOJOIN: int = 1024;
pub const IFA_F_STABLE_PRIVACY: int = 2048;
pub const RTNL_FAMILY_IPMR: int = 128;
pub const RTNL_FAMILY_IP6MR: int = 129;
pub const RTNL_FAMILY_MAX: int = 129;
pub const RTA_ALIGNTO: int = 4;

pub const RTNH_F_DEAD: int = 1;
pub const RTNH_F_PERVASIVE: int = 2;
pub const RTNH_F_ONLINK: int = 4;
pub const RTNH_F_OFFLOAD: int = 8;
pub const RTNH_F_LINKDOWN: int = 16;
pub const RTNH_F_UNRESOLVED: int = 32;
pub const RTNH_COMPARE_MASK: int = 25;
pub const RTNH_ALIGNTO: int = 4;
pub const RTNETLINK_HAVE_PEERINFO: int = 1;
pub const RTAX_FEATURE_ECN: int = 1;
pub const RTAX_FEATURE_SACK: int = 2;
pub const RTAX_FEATURE_TIMESTAMP: int = 4;
pub const RTAX_FEATURE_ALLFRAG: int = 8;
pub const RTAX_FEATURE_MASK: int = 15;
#[allow(overflowing_literals)]
pub const TCM_IFINDEX_MAGIC_BLOCK: int = 0xffff_ffff;
pub const TCA_FLAG_LARGE_DUMP_ON: int = 1;
pub const RTEXT_FILTER_VF: u32 = 1;
pub const RTEXT_FILTER_BRVLAN: u32 = 2;
pub const RTEXT_FILTER_BRVLAN_COMPRESSED: u32 = 4;
pub const RTEXT_FILTER_SKIP_STATS: u32 = 8;
pub const ARPOP_REQUEST: int = 1;
pub const ARPOP_REPLY: int = 2;
