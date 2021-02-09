// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! definitions to be used with the WinSock 2 DLL and WinSock 2 applications.
//!
//! This header file corresponds to version 2.2.x of the WinSock API specification.
pub const WINSOCK_VERSION: ::WORD = 2 | (2 << 8);
pub type u_char = ::c_uchar;
pub type u_short = ::c_ushort;
pub type u_int = ::c_uint;
pub type u_long = ::c_ulong;
pub type u_int64 = ::__uint64;
pub type SOCKET = ::UINT_PTR;
pub type GROUP = ::c_uint;
pub const FD_SETSIZE: usize = 64;
pub const FD_MAX_EVENTS: usize = 10;
STRUCT!{nodebug struct fd_set {
    fd_count: u_int,
    fd_array: [SOCKET; FD_SETSIZE],
}}
STRUCT!{struct timeval {
    tv_sec: ::c_long,
    tv_usec: ::c_long,
}}
STRUCT!{struct hostent {
    h_name: *mut ::c_char,
    h_aliases: *mut *mut ::c_char,
    h_addrtype: ::c_short,
    h_length: ::c_short,
    h_addr_list: *mut *mut ::c_char,
}}
STRUCT!{struct netent {
    n_name: *mut ::c_char,
    n_aliases: *mut *mut ::c_char,
    n_addrtype: ::c_short,
    n_net: u_long,
}}
#[cfg(target_arch="x86")]
STRUCT!{struct servent {
    s_name: *mut ::c_char,
    s_aliases: *mut *mut ::c_char,
    s_port: ::c_short,
    s_proto: *mut ::c_char,
}}
#[cfg(target_arch="x86_64")]
STRUCT!{struct servent {
    s_name: *mut ::c_char,
    s_aliases: *mut *mut ::c_char,
    s_proto: *mut ::c_char,
    s_port: ::c_short,
}}
STRUCT!{struct protoent {
    p_name: *mut ::c_char,
    p_aliases: *mut *mut ::c_char,
    p_proto: ::c_short,
}}
pub const WSADESCRIPTION_LEN: usize = 256;
pub const WSASYS_STATUS_LEN: usize = 128;
#[cfg(target_arch="x86")]
STRUCT!{nodebug struct WSADATA {
    wVersion: ::WORD,
    wHighVersion: ::WORD,
    szDescription: [::c_char; WSADESCRIPTION_LEN + 1],
    szSystemStatus: [::c_char; WSASYS_STATUS_LEN + 1],
    iMaxSockets: ::c_ushort,
    iMaxUdpDg: ::c_ushort,
    lpVendorInfo: *mut ::c_char,
}}
#[cfg(target_arch="x86_64")]
STRUCT!{nodebug struct WSADATA {
    wVersion: ::WORD,
    wHighVersion: ::WORD,
    iMaxSockets: ::c_ushort,
    iMaxUdpDg: ::c_ushort,
    lpVendorInfo: *mut ::c_char,
    szDescription: [::c_char; WSADESCRIPTION_LEN + 1],
    szSystemStatus: [::c_char; WSASYS_STATUS_LEN + 1],
}}
pub type LPWSADATA = *mut WSADATA;
//391
pub const INVALID_SOCKET: SOCKET = !0;
pub const SOCKET_ERROR: ::c_int = -1;
STRUCT!{struct sockproto {
    sp_family: u_short,
    sp_protocol: u_short,
}}
pub const PF_UNSPEC: ::c_int = ::AF_UNSPEC;
pub const PF_UNIX: ::c_int = ::AF_UNIX;
pub const PF_INET: ::c_int = ::AF_INET;
pub const PF_IMPLINK: ::c_int = ::AF_IMPLINK;
pub const PF_PUP: ::c_int = ::AF_PUP;
pub const PF_CHAOS: ::c_int = ::AF_CHAOS;
pub const PF_NS: ::c_int = ::AF_NS;
pub const PF_IPX: ::c_int = ::AF_IPX;
pub const PF_ISO: ::c_int = ::AF_ISO;
pub const PF_OSI: ::c_int = ::AF_OSI;
pub const PF_ECMA: ::c_int = ::AF_ECMA;
pub const PF_DATAKIT: ::c_int = ::AF_DATAKIT;
pub const PF_CCITT: ::c_int = ::AF_CCITT;
pub const PF_SNA: ::c_int = ::AF_SNA;
pub const PF_DECnet: ::c_int = ::AF_DECnet;
pub const PF_DLI: ::c_int = ::AF_DLI;
pub const PF_LAT: ::c_int = ::AF_LAT;
pub const PF_HYLINK: ::c_int = ::AF_HYLINK;
pub const PF_APPLETALK: ::c_int = ::AF_APPLETALK;
pub const PF_VOICEVIEW: ::c_int = ::AF_VOICEVIEW;
pub const PF_FIREFOX: ::c_int = ::AF_FIREFOX;
pub const PF_UNKNOWN1: ::c_int = ::AF_UNKNOWN1;
pub const PF_BAN: ::c_int = ::AF_BAN;
pub const PF_ATM: ::c_int = ::AF_ATM;
pub const PF_INET6: ::c_int = ::AF_INET6;
pub const PF_BTH: ::c_int = ::AF_BTH;
pub const PF_MAX: ::c_int = ::AF_MAX;
STRUCT!{struct linger {
    l_onoff: u_short,
    l_linger: u_short,
}}
pub const SOMAXCONN: ::c_int = 0x7fffffff;
pub type WSAEVENT = ::HANDLE;
pub type LPWSAEVENT = ::LPHANDLE;
pub type WSAOVERLAPPED = ::OVERLAPPED;
pub type LPWSAOVERLAPPED = *mut ::OVERLAPPED;
pub const WSA_IO_PENDING: ::DWORD = ::ERROR_IO_PENDING;
pub const WSA_IO_INCOMPLETE: ::DWORD = ::ERROR_IO_INCOMPLETE;
pub const WSA_INVALID_HANDLE: ::DWORD = ::ERROR_INVALID_HANDLE;
pub const WSA_INVALID_PARAMETER: ::DWORD = ::ERROR_INVALID_PARAMETER;
pub const WSA_NOT_ENOUGH_MEMORY: ::DWORD = ::ERROR_NOT_ENOUGH_MEMORY;
pub const WSA_OPERATION_ABORTED: ::DWORD = ::ERROR_OPERATION_ABORTED;
STRUCT!{struct QOS {
    SendingFlowspec: ::FLOWSPEC,
    FLOWSPEC: ::FLOWSPEC,
    ProviderSpecific: ::WSABUF,
}}
pub type LPQOS = *mut QOS;
STRUCT!{struct WSANETWORKEVENTS {
    lNetworkEvents: ::c_long,
    iErrorCode: [::c_int; FD_MAX_EVENTS],
}}
pub type LPWSANETWORKEVENTS = *mut WSANETWORKEVENTS;
pub const MAX_PROTOCOL_CHAIN: usize = 7;
STRUCT!{struct WSAPROTOCOLCHAIN {
    ChainLen: ::c_int,
    ChainEntries: [::DWORD; MAX_PROTOCOL_CHAIN],
}}
pub type LPWSAPROTOCOLCHAIN = *mut WSAPROTOCOLCHAIN;
pub const WSAPROTOCOL_LEN: usize = 255;
STRUCT!{nodebug struct WSAPROTOCOL_INFOA {
    dwServiceFlags1: ::DWORD,
    dwServiceFlags2: ::DWORD,
    dwServiceFlags3: ::DWORD,
    dwServiceFlags4: ::DWORD,
    dwServiceFlags5: ::DWORD,
    ProviderId: ::GUID,
    dwCatalogEntryId: ::DWORD,
    ProtocolChain: WSAPROTOCOLCHAIN,
    iVersion: ::c_int,
    iAddressFamily: ::c_int,
    iMaxSockAddr: ::c_int,
    iMinSockAddr: ::c_int,
    iSocketType: ::c_int,
    iProtocol: ::c_int,
    iProtocolMaxOffset: ::c_int,
    iNetworkByteOrder: ::c_int,
    iSecurityScheme: ::c_int,
    dwMessageSize: ::DWORD,
    dwProviderReserved: ::DWORD,
    szProtocol: [::CHAR; WSAPROTOCOL_LEN + 1],
}}
pub type LPWSAPROTOCOL_INFOA = *mut WSAPROTOCOL_INFOA;
STRUCT!{nodebug struct WSAPROTOCOL_INFOW {
    dwServiceFlags1: ::DWORD,
    dwServiceFlags2: ::DWORD,
    dwServiceFlags3: ::DWORD,
    dwServiceFlags4: ::DWORD,
    dwServiceFlags5: ::DWORD,
    ProviderId: ::GUID,
    dwCatalogEntryId: ::DWORD,
    ProtocolChain: WSAPROTOCOLCHAIN,
    iVersion: ::c_int,
    iAddressFamily: ::c_int,
    iMaxSockAddr: ::c_int,
    iMinSockAddr: ::c_int,
    iSocketType: ::c_int,
    iProtocol: ::c_int,
    iProtocolMaxOffset: ::c_int,
    iNetworkByteOrder: ::c_int,
    iSecurityScheme: ::c_int,
    dwMessageSize: ::DWORD,
    dwProviderReserved: ::DWORD,
    szProtocol: [::WCHAR; WSAPROTOCOL_LEN + 1],
}}
pub type LPWSAPROTOCOL_INFOW = *mut WSAPROTOCOL_INFOW;
pub type LPCONDITIONPROC = Option<unsafe extern "system" fn(
    lpCallerId: ::LPWSABUF, lpCallerData: ::LPWSABUF, lpSQOS: LPQOS, lpGQOS: LPQOS,
    lpCalleeId: ::LPWSABUF, lpCalleeData: ::LPWSABUF, g: *mut GROUP, dwCallbackData: ::DWORD,
) -> ::c_int>;
pub type LPWSAOVERLAPPED_COMPLETION_ROUTINE = Option<unsafe extern "system" fn(
    dwError: ::DWORD, cbTransferred: ::DWORD, lpOverlapped: LPWSAOVERLAPPED, dwFlags: ::DWORD,
)>;
ENUM!{enum WSACOMPLETIONTYPE {
    NSP_NOTIFY_IMMEDIATELY = 0,
    NSP_NOTIFY_HWND,
    NSP_NOTIFY_EVENT,
    NSP_NOTIFY_PORT,
    NSP_NOTIFY_APC,
}}
pub type PWSACOMPLETIONTYPE = *mut WSACOMPLETIONTYPE;
pub type LPWSACOMPLETIONTYPE = *mut WSACOMPLETIONTYPE;
STRUCT!{struct WSACOMPLETION_WindowMessage {
    hWnd: ::HWND,
    uMsg: ::UINT,
    context: ::WPARAM,
}}
STRUCT!{struct WSACOMPLETION_Event {
    lpOverlapped: LPWSAOVERLAPPED,
}}
STRUCT!{nodebug struct WSACOMPLETION_Apc {
    lpOverlapped: LPWSAOVERLAPPED,
    lpfnCompletionProc: LPWSAOVERLAPPED_COMPLETION_ROUTINE,
}}
STRUCT!{struct WSACOMPLETION_Port {
    lpOverlapped: LPWSAOVERLAPPED,
    hPort: ::HANDLE,
    Key: ::ULONG_PTR,
}}
#[cfg(target_arch="x86")]
STRUCT!{struct WSACOMPLETION {
    Type: WSACOMPLETIONTYPE,
    Parameters: [u8; 12],
}}
#[cfg(target_arch="x86_64")]
STRUCT!{struct WSACOMPLETION {
    Type: WSACOMPLETIONTYPE,
    Parameters: [u8; 24],
}}
UNION!(WSACOMPLETION, Parameters, WindowMessage, WindowMessage_mut, WSACOMPLETION_WindowMessage);
UNION!(WSACOMPLETION, Parameters, Event, Event_mut, WSACOMPLETION_Event);
UNION!(WSACOMPLETION, Parameters, Apc, Apc_mut, WSACOMPLETION_Apc);
UNION!(WSACOMPLETION, Parameters, Port, Port_mut, WSACOMPLETION_Port);
pub type PWSACOMPLETION = *mut WSACOMPLETION;
pub type LPWSACOMPLETION = *mut WSACOMPLETION;
STRUCT!{struct AFPROTOCOLS {
    iAddressFamily: ::INT,
    iProtocol: ::INT,
}}
pub type PAFPROTOCOLS = *mut AFPROTOCOLS;
pub type LPAFPROTOCOLS = *mut AFPROTOCOLS;
ENUM!{enum WSAECOMPARATOR {
    COMP_EQUAL = 0,
    COMP_NOTLESS,
}}
pub type PWSAECOMPARATOR = *mut WSAECOMPARATOR;
pub type LPWSAECOMPARATOR = *mut WSAECOMPARATOR;
STRUCT!{struct WSAVERSION {
    dwVersion: ::DWORD,
    ecHow: WSAECOMPARATOR,
}}
pub type PWSAVERSION = *mut WSAVERSION;
pub type LPWSAVERSION = *mut WSAVERSION;
STRUCT!{struct WSAQUERYSETA {
    dwSize: ::DWORD,
    lpszServiceInstanceName: ::LPSTR,
    lpServiceClassId: ::LPGUID,
    lpVersion: LPWSAVERSION,
    lpszComment: ::LPSTR,
    dwNameSpace: ::DWORD,
    lpNSProviderId: ::LPGUID,
    lpszContext: ::LPSTR,
    dwNumberOfProtocols: ::DWORD,
    lpafpProtocols: LPAFPROTOCOLS,
    lpszQueryString: ::LPSTR,
    dwNumberOfCsAddrs: ::DWORD,
    lpcsaBuffer: ::LPCSADDR_INFO,
    dwOutputFlags: ::DWORD,
    lpBlob: ::LPBLOB,
}}
pub type PWSAQUERYSETA = *mut WSAQUERYSETA;
pub type LPWSAQUERYSETA = *mut WSAQUERYSETA;
STRUCT!{struct WSAQUERYSETW {
    dwSize: ::DWORD,
    lpszServiceInstanceName: ::LPWSTR,
    lpServiceClassId: ::LPGUID,
    lpVersion: LPWSAVERSION,
    lpszComment: ::LPWSTR,
    dwNameSpace: ::DWORD,
    lpNSProviderId: ::LPGUID,
    lpszContext: ::LPWSTR,
    dwNumberOfProtocols: ::DWORD,
    lpafpProtocols: LPAFPROTOCOLS,
    lpszQueryString: ::LPWSTR,
    dwNumberOfCsAddrs: ::DWORD,
    lpcsaBuffer: ::LPCSADDR_INFO,
    dwOutputFlags: ::DWORD,
    lpBlob: ::LPBLOB,
}}
pub type PWSAQUERYSETW = *mut WSAQUERYSETW;
pub type LPWSAQUERYSETW = *mut WSAQUERYSETW;
STRUCT!{struct WSAQUERYSET2A {
    dwSize: ::DWORD,
    lpszServiceInstanceName: ::LPSTR,
    lpVersion: LPWSAVERSION,
    lpszComment: ::LPSTR,
    dwNameSpace: ::DWORD,
    lpNSProviderId: ::LPGUID,
    lpszContext: ::LPSTR,
    dwNumberOfProtocols: ::DWORD,
    lpafpProtocols: LPAFPROTOCOLS,
    lpszQueryString: ::LPSTR,
    dwNumberOfCsAddrs: ::DWORD,
    lpcsaBuffer: ::LPCSADDR_INFO,
    dwOutputFlags: ::DWORD,
    lpBlob: ::LPBLOB,
}}
pub type PWSAQUERYSET2A = *mut WSAQUERYSET2A;
pub type LPWSAQUERYSET2A = *mut WSAQUERYSET2A;
STRUCT!{struct WSAQUERYSET2W {
    dwSize: ::DWORD,
    lpszServiceInstanceName: ::LPWSTR,
    lpVersion: LPWSAVERSION,
    lpszComment: ::LPWSTR,
    dwNameSpace: ::DWORD,
    lpNSProviderId: ::LPGUID,
    lpszContext: ::LPWSTR,
    dwNumberOfProtocols: ::DWORD,
    lpafpProtocols: LPAFPROTOCOLS,
    lpszQueryString: ::LPWSTR,
    dwNumberOfCsAddrs: ::DWORD,
    lpcsaBuffer: ::LPCSADDR_INFO,
    dwOutputFlags: ::DWORD,
    lpBlob: ::LPBLOB,
}}
pub type PWSAQUERYSET2W = *mut WSAQUERYSET2W;
pub type LPWSAQUERYSET2W = *mut WSAQUERYSET2W;
ENUM!{enum WSAESETSERVICEOP {
    RNRSERVICE_REGISTER = 0,
    RNRSERVICE_DEREGISTER,
    RNRSERVICE_DELETE,
}}
pub type PWSAESETSERVICEOP = *mut WSAESETSERVICEOP;
pub type LPWSAESETSERVICEOP = *mut WSAESETSERVICEOP;
STRUCT!{struct WSANSCLASSINFOA {
    lpszName: ::LPSTR,
    dwNameSpace: ::DWORD,
    dwValueType: ::DWORD,
    dwValueSize: ::DWORD,
    lpValue: ::LPVOID,
}}
pub type PWSANSCLASSINFOA = *mut WSANSCLASSINFOA;
pub type LPWSANSCLASSINFOA = *mut WSANSCLASSINFOA;
STRUCT!{struct WSANSCLASSINFOW {
    lpszName: ::LPWSTR,
    dwNameSpace: ::DWORD,
    dwValueType: ::DWORD,
    dwValueSize: ::DWORD,
    lpValue: ::LPVOID,
}}
pub type PWSANSCLASSINFOW = *mut WSANSCLASSINFOW;
pub type LPWSANSCLASSINFOW = *mut WSANSCLASSINFOW;
STRUCT!{struct WSASERVICECLASSINFOA {
    lpServiceClassId: ::LPGUID,
    lpszServiceClassName: ::LPSTR,
    dwCount: ::DWORD,
    lpClassInfos: LPWSANSCLASSINFOA,
}}
pub type PWSASERVICECLASSINFOA = *mut WSASERVICECLASSINFOA;
pub type LPWSASERVICECLASSINFOA = *mut WSASERVICECLASSINFOA;
STRUCT!{struct WSASERVICECLASSINFOW {
    lpServiceClassId: ::LPGUID,
    lpszServiceClassName: ::LPWSTR,
    dwCount: ::DWORD,
    lpClassInfos: LPWSANSCLASSINFOW,
}}
pub type PWSASERVICECLASSINFOW = *mut WSASERVICECLASSINFOW;
pub type LPWSASERVICECLASSINFOW = *mut WSASERVICECLASSINFOW;
STRUCT!{struct WSANAMESPACE_INFOA {
    NSProviderId: ::GUID,
    dwNameSpace: ::DWORD,
    fActive: ::BOOL,
    dwVersion: ::DWORD,
    lpszIdentifier: ::LPSTR,
}}
pub type PWSANAMESPACE_INFOA = *mut WSANAMESPACE_INFOA;
pub type LPWSANAMESPACE_INFOA = *mut WSANAMESPACE_INFOA;
STRUCT!{struct WSANAMESPACE_INFOW {
    NSProviderId: ::GUID,
    dwNameSpace: ::DWORD,
    fActive: ::BOOL,
    dwVersion: ::DWORD,
    lpszIdentifier: ::LPWSTR,
}}
pub type PWSANAMESPACE_INFOW = *mut WSANAMESPACE_INFOW;
pub type LPWSANAMESPACE_INFOW = *mut WSANAMESPACE_INFOW;
STRUCT!{struct WSANAMESPACE_INFOEXA {
    NSProviderId: ::GUID,
    dwNameSpace: ::DWORD,
    fActive: ::BOOL,
    dwVersion: ::DWORD,
    lpszIdentifier: ::LPSTR,
    ProviderSpecific: ::BLOB,
}}
pub type PWSANAMESPACE_INFOEXA = *mut WSANAMESPACE_INFOEXA;
pub type LPWSANAMESPACE_INFOEXA = *mut WSANAMESPACE_INFOEXA;
STRUCT!{struct WSANAMESPACE_INFOEXW {
    NSProviderId: ::GUID,
    dwNameSpace: ::DWORD,
    fActive: ::BOOL,
    dwVersion: ::DWORD,
    lpszIdentifier: ::LPWSTR,
    ProviderSpecific: ::BLOB,
}}
pub type PWSANAMESPACE_INFOEXW = *mut WSANAMESPACE_INFOEXW;
pub type LPWSANAMESPACE_INFOEXW = *mut WSANAMESPACE_INFOEXW;
pub const POLLRDNORM: ::SHORT = 0x0100;
pub const POLLRDBAND: ::SHORT = 0x0200;
pub const POLLIN: ::SHORT = POLLRDNORM | POLLRDBAND;
pub const POLLPRI: ::SHORT = 0x0400;
pub const POLLWRNORM: ::SHORT = 0x0010;
pub const POLLOUT: ::SHORT = POLLWRNORM;
pub const POLLWRBAND: ::SHORT = 0x0020;
pub const POLLERR: ::SHORT = 0x0001;
pub const POLLHUP: ::SHORT = 0x0002;
pub const POLLNVAL: ::SHORT = 0x0004;
STRUCT!{struct WSAPOLLFD {
    fd: ::SOCKET,
    events: ::SHORT,
    revents: ::SHORT,
}}
pub type PWSAPOLLFD = *mut WSAPOLLFD;
pub type LPWSAPOLLFD = *mut WSAPOLLFD;
pub const FIONBIO: ::c_ulong = 0x8004667e;
