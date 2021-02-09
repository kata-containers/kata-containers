// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! This file contains the core definitions for the Winsock2 specification that can be used by
//! both user-mode and kernel mode modules.
pub type ADDRESS_FAMILY = ::USHORT;
pub const AF_UNSPEC: ::c_int = 0;
pub const AF_UNIX: ::c_int = 1;
pub const AF_INET: ::c_int = 2;
pub const AF_IMPLINK: ::c_int = 3;
pub const AF_PUP: ::c_int = 4;
pub const AF_CHAOS: ::c_int = 5;
pub const AF_NS: ::c_int = 6;
pub const AF_IPX: ::c_int = AF_NS;
pub const AF_ISO: ::c_int = 7;
pub const AF_OSI: ::c_int = AF_ISO;
pub const AF_ECMA: ::c_int = 8;
pub const AF_DATAKIT: ::c_int = 9;
pub const AF_CCITT: ::c_int = 10;
pub const AF_SNA: ::c_int = 11;
pub const AF_DECnet: ::c_int = 12;
pub const AF_DLI: ::c_int = 13;
pub const AF_LAT: ::c_int = 14;
pub const AF_HYLINK: ::c_int = 15;
pub const AF_APPLETALK: ::c_int = 16;
pub const AF_NETBIOS: ::c_int = 17;
pub const AF_VOICEVIEW: ::c_int = 18;
pub const AF_FIREFOX: ::c_int = 19;
pub const AF_UNKNOWN1: ::c_int = 20;
pub const AF_BAN: ::c_int = 21;
pub const AF_ATM: ::c_int = 22;
pub const AF_INET6: ::c_int = 23;
pub const AF_CLUSTER: ::c_int = 24;
pub const AF_12844: ::c_int = 25;
pub const AF_IRDA: ::c_int = 26;
pub const AF_NETDES: ::c_int = 28;
pub const AF_TCNPROCESS: ::c_int = 29;
pub const AF_TCNMESSAGE: ::c_int = 30;
pub const AF_ICLFXBM: ::c_int = 31;
pub const AF_BTH: ::c_int = 32;
pub const AF_LINK: ::c_int = 33;
pub const AF_MAX: ::c_int = 34;
pub const SOCK_STREAM: ::c_int = 1;
pub const SOCK_DGRAM: ::c_int = 2;
pub const SOCK_RAW: ::c_int = 3;
pub const SOCK_RDM: ::c_int = 4;
pub const SOCK_SEQPACKET: ::c_int = 5;
pub const SOL_SOCKET: ::c_int = 0xffff;
pub const SO_DEBUG: ::c_int = 0x0001;
pub const SO_ACCEPTCONN: ::c_int = 0x0002;
pub const SO_REUSEADDR: ::c_int = 0x0004;
pub const SO_KEEPALIVE: ::c_int = 0x0008;
pub const SO_DONTROUTE: ::c_int = 0x0010;
pub const SO_BROADCAST: ::c_int = 0x0020;
pub const SO_USELOOPBACK: ::c_int = 0x0040;
pub const SO_LINGER: ::c_int = 0x0080;
pub const SO_OOBINLINE: ::c_int = 0x0100;
pub const SO_DONTLINGER: ::c_int = !SO_LINGER;
pub const SO_EXCLUSIVEADDRUSE: ::c_int = !SO_REUSEADDR;
pub const SO_SNDBUF: ::c_int = 0x1001;
pub const SO_RCVBUF: ::c_int = 0x1002;
pub const SO_SNDLOWAT: ::c_int = 0x1003;
pub const SO_RCVLOWAT: ::c_int = 0x1004;
pub const SO_SNDTIMEO: ::c_int = 0x1005;
pub const SO_RCVTIMEO: ::c_int = 0x1006;
pub const SO_ERROR: ::c_int = 0x1007;
pub const SO_TYPE: ::c_int = 0x1008;
pub const SO_BSP_STATE: ::c_int = 0x1009;
pub const SO_GROUP_ID: ::c_int = 0x2001;
pub const SO_GROUP_PRIORITY: ::c_int = 0x2002;
pub const SO_MAX_MSG_SIZE: ::c_int = 0x2003;
pub const SO_CONDITIONAL_ACCEPT: ::c_int = 0x3002;
pub const SO_PAUSE_ACCEPT: ::c_int = 0x3003;
pub const SO_COMPARTMENT_ID: ::c_int = 0x3004;
pub const SO_RANDOMIZE_PORT: ::c_int = 0x3005;
pub const SO_PORT_SCALABILITY: ::c_int = 0x3006;
pub const WSK_SO_BASE: ::c_int = 0x4000;
pub const TCP_NODELAY: ::c_int = 0x0001;
STRUCT!{struct SOCKADDR {
    sa_family: ADDRESS_FAMILY,
    sa_data: [::CHAR; 14],
}}
pub type PSOCKADDR = *mut SOCKADDR;
pub type LPSOCKADDR = *mut SOCKADDR;
STRUCT!{struct SOCKET_ADDRESS {
    lpSockaddr: LPSOCKADDR,
    iSockaddrLength: ::INT,
}}
pub type PSOCKET_ADDRESS = *mut SOCKET_ADDRESS;
pub type LPSOCKET_ADDRESS = *mut SOCKET_ADDRESS;
STRUCT!{nodebug struct SOCKET_ADDRESS_LIST {
    iAddressCount: ::INT,
    Address: [SOCKET_ADDRESS; 0],
}}
pub type PSOCKET_ADDRESS_LIST = *mut SOCKET_ADDRESS_LIST;
pub type LPSOCKET_ADDRESS_LIST = *mut SOCKET_ADDRESS_LIST;
STRUCT!{struct CSADDR_INFO {
    LocalAddr: SOCKET_ADDRESS,
    RemoteAddr: SOCKET_ADDRESS,
    iSocketType: ::INT,
    iProtocol: ::INT,
}}
pub type PCSADDR_INFO = *mut CSADDR_INFO;
pub type LPCSADDR_INFO = *mut CSADDR_INFO;
STRUCT!{nodebug struct SOCKADDR_STORAGE_LH {
    ss_family: ADDRESS_FAMILY,
    __ss_pad1: [::CHAR; 6],
    __ss_align: ::__int64,
    __ss_pad2: [::CHAR; 112],
}}
pub type PSOCKADDR_STORAGE_LH = *mut SOCKADDR_STORAGE_LH;
pub type LPSOCKADDR_STORAGE_LH = *mut SOCKADDR_STORAGE_LH;
STRUCT!{nodebug struct SOCKADDR_STORAGE_XP {
    ss_family: ::c_short,
    __ss_pad1: [::CHAR; 6],
    __ss_align: ::__int64,
    __ss_pad2: [::CHAR; 112],
}}
pub type PSOCKADDR_STORAGE_XP = *mut SOCKADDR_STORAGE_XP;
pub type LPSOCKADDR_STORAGE_XP = *mut SOCKADDR_STORAGE_XP;
pub type SOCKADDR_STORAGE = SOCKADDR_STORAGE_LH;
pub type PSOCKADDR_STORAGE = *mut SOCKADDR_STORAGE;
pub type LPSOCKADDR_STORAGE = *mut SOCKADDR_STORAGE;
STRUCT!{struct SOCKET_PROCESSOR_AFFINITY {
    Processor: ::PROCESSOR_NUMBER,
    NumaNodeId: ::USHORT,
    Reserved: ::USHORT,
}}
pub type PSOCKET_PROCESSOR_AFFINITY = *mut SOCKET_PROCESSOR_AFFINITY;
pub const IOC_UNIX: ::DWORD = 0x00000000;
pub const IOC_WS2: ::DWORD = 0x08000000;
pub const IOC_PROTOCOL: ::DWORD = 0x10000000;
pub const IOC_VENDOR: ::DWORD = 0x18000000;
pub const IOC_WSK: ::DWORD = IOC_WS2 | 0x07000000;
macro_rules! _WSAIO { ($x:expr, $y:expr) => { IOC_VOID | $x | $y } }
macro_rules! _WSAIOR { ($x:expr, $y:expr) => { IOC_OUT | $x | $y } }
macro_rules! _WSAIOW { ($x:expr, $y:expr) => { IOC_IN | $x | $y } }
macro_rules! _WSAIORW { ($x:expr, $y:expr) => { IOC_INOUT | $x | $y } }
pub const SIO_ASSOCIATE_HANDLE: ::DWORD = _WSAIOW!(IOC_WS2, 1);
pub const SIO_ENABLE_CIRCULAR_QUEUEING: ::DWORD = _WSAIO!(IOC_WS2, 2);
pub const SIO_FIND_ROUTE: ::DWORD = _WSAIOR!(IOC_WS2, 3);
pub const SIO_FLUSH: ::DWORD = _WSAIO!(IOC_WS2, 4);
pub const SIO_GET_BROADCAST_ADDRESS: ::DWORD = _WSAIOR!(IOC_WS2, 5);
pub const SIO_GET_EXTENSION_FUNCTION_POINTER: ::DWORD = _WSAIORW!(IOC_WS2, 6);
pub const SIO_GET_QOS: ::DWORD = _WSAIORW!(IOC_WS2, 7);
pub const SIO_GET_GROUP_QOS: ::DWORD = _WSAIORW!(IOC_WS2, 8);
pub const SIO_MULTIPOINT_LOOPBACK: ::DWORD = _WSAIOW!(IOC_WS2, 9);
pub const SIO_MULTICAST_SCOPE: ::DWORD = _WSAIOW!(IOC_WS2, 10);
pub const SIO_SET_QOS: ::DWORD = _WSAIOW!(IOC_WS2, 11);
pub const SIO_SET_GROUP_QOS: ::DWORD = _WSAIOW!(IOC_WS2, 12);
pub const SIO_TRANSLATE_HANDLE: ::DWORD = _WSAIORW!(IOC_WS2, 13);
pub const SIO_ROUTING_INTERFACE_QUERY: ::DWORD = _WSAIORW!(IOC_WS2, 20);
pub const SIO_ROUTING_INTERFACE_CHANGE: ::DWORD = _WSAIOW!(IOC_WS2, 21);
pub const SIO_ADDRESS_LIST_QUERY: ::DWORD = _WSAIOR!(IOC_WS2, 22);
pub const SIO_ADDRESS_LIST_CHANGE: ::DWORD = _WSAIO!(IOC_WS2, 23);
pub const SIO_QUERY_TARGET_PNP_HANDLE: ::DWORD = _WSAIOR!(IOC_WS2, 24);
pub const SIO_QUERY_RSS_PROCESSOR_INFO: ::DWORD = _WSAIOR!(IOC_WS2, 37);
pub const SIO_ADDRESS_LIST_SORT: ::DWORD = _WSAIORW!(IOC_WS2, 25);
pub const SIO_RESERVED_1: ::DWORD = _WSAIOW!(IOC_WS2, 26);
pub const SIO_RESERVED_2: ::DWORD = _WSAIOW!(IOC_WS2, 33);
pub const SIO_GET_MULTIPLE_EXTENSION_FUNCTION_POINTER: ::DWORD = _WSAIORW!(IOC_WS2, 36);
pub const IPPROTO_IP: ::c_int = 0;
ENUM!{enum IPPROTO {
    IPPROTO_HOPOPTS = 0,  // IPv6 Hop-by-Hop options
    IPPROTO_ICMP = 1,
    IPPROTO_IGMP = 2,
    IPPROTO_GGP = 3,
    IPPROTO_IPV4 = 4,
    IPPROTO_ST = 5,
    IPPROTO_TCP = 6,
    IPPROTO_CBT = 7,
    IPPROTO_EGP = 8,
    IPPROTO_IGP = 9,
    IPPROTO_PUP = 12,
    IPPROTO_UDP = 17,
    IPPROTO_IDP = 22,
    IPPROTO_RDP = 27,
    IPPROTO_IPV6 = 41, // IPv6 header
    IPPROTO_ROUTING = 43, // IPv6 Routing header
    IPPROTO_FRAGMENT = 44, // IPv6 fragmentation header
    IPPROTO_ESP = 50, // encapsulating security payload
    IPPROTO_AH = 51, // authentication header
    IPPROTO_ICMPV6 = 58, // ICMPv6
    IPPROTO_NONE = 59, // IPv6 no next header
    IPPROTO_DSTOPTS = 60, // IPv6 Destination options
    IPPROTO_ND = 77,
    IPPROTO_ICLFXBM = 78,
    IPPROTO_PIM = 103,
    IPPROTO_PGM = 113,
    IPPROTO_L2TP = 115,
    IPPROTO_SCTP = 132,
    IPPROTO_RAW = 255,
    IPPROTO_MAX = 256,
    IPPROTO_RESERVED_RAW  = 257,
    IPPROTO_RESERVED_IPSEC = 258,
    IPPROTO_RESERVED_IPSECOFFLOAD = 259,
    IPPROTO_RESERVED_WNV = 260,
    IPPROTO_RESERVED_MAX = 261,
}}
pub type PIPPROTO = *mut IPPROTO;
STRUCT!{struct SOCKADDR_IN {
    sin_family: ADDRESS_FAMILY,
    sin_port: ::USHORT,
    sin_addr: ::IN_ADDR,
    sin_zero: [::CHAR; 8],
}}
pub type PSOCKADDR_IN = *mut SOCKADDR_IN;
//645
pub const IOCPARM_MASK: ::DWORD = 0x7f;
pub const IOC_VOID: ::DWORD = 0x20000000;
pub const IOC_OUT: ::DWORD = 0x40000000;
pub const IOC_IN: ::DWORD = 0x80000000;
pub const IOC_INOUT: ::DWORD = IOC_IN | IOC_OUT;
STRUCT!{struct WSABUF {
    len: ::ULONG,
    buf: *mut ::CHAR,
}}
pub type LPWSABUF = *mut WSABUF;
STRUCT!{struct WSAMSG {
    name: LPSOCKADDR,
    namelen: ::INT,
    lpBuffers: LPWSABUF,
    dwBufferCount: ::ULONG,
    Control: WSABUF,
    dwFlags: ::ULONG,
}}
pub type PWSAMSG = *mut WSAMSG;
pub type LPWSAMSG = *mut WSAMSG;
STRUCT!{struct ADDRINFOA {
    ai_flags: ::c_int,
    ai_family: ::c_int,
    ai_socktype: ::c_int,
    ai_protocol: ::c_int,
    ai_addrlen: ::size_t,
    ai_canonname: *mut ::c_char,
    ai_addr: *mut SOCKADDR,
    ai_next: *mut ADDRINFOA,
}}
pub type PADDRINFOA = *mut ADDRINFOA;
STRUCT!{struct ADDRINFOW {
    ai_flags: ::c_int,
    ai_family: ::c_int,
    ai_socktype: ::c_int,
    ai_protocol: ::c_int,
    ai_addrlen: ::size_t,
    ai_canonname: ::PWSTR,
    ai_addr: *mut SOCKADDR,
    ai_next: *mut ADDRINFOW,
}}
pub type PADDRINFOW = *mut ADDRINFOW;
STRUCT!{struct ADDRINFOEXA {
    ai_flags: ::c_int,
    ai_family: ::c_int,
    ai_socktype: ::c_int,
    ai_protocol: ::c_int,
    ai_addrlen: ::size_t,
    ai_canonname: *mut ::c_char,
    ai_addr: *mut SOCKADDR,
    ai_blob: *mut ::c_void,
    ai_bloblen: ::size_t,
    ai_provider: ::LPGUID,
    ai_next: *mut ADDRINFOEXW,
}}
pub type PADDRINFOEXA = *mut ADDRINFOEXA;
pub type LPADDRINFOEXA = *mut ADDRINFOEXA;
STRUCT!{struct ADDRINFOEXW {
    ai_flags: ::c_int,
    ai_family: ::c_int,
    ai_socktype: ::c_int,
    ai_protocol: ::c_int,
    ai_addrlen: ::size_t,
    ai_canonname: ::PWSTR,
    ai_addr: *mut SOCKADDR,
    ai_blob: *mut ::c_void,
    ai_bloblen: ::size_t,
    ai_provider: ::LPGUID,
    ai_next: *mut ADDRINFOEXW,
}}
pub type PADDRINFOEXW = *mut ADDRINFOEXW;
pub type LPADDRINFOEXW = *mut ADDRINFOEXW;
