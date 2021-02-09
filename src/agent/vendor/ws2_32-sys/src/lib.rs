// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! FFI bindings to ws2_32.
#![cfg(windows)]
extern crate winapi;
use winapi::*;
extern "system" {
    pub fn FreeAddrInfoEx(pAddrInfoEx: PADDRINFOEXA);
    pub fn FreeAddrInfoExW(pAddrInfoEx: PADDRINFOEXW);
    pub fn FreeAddrInfoW(pAddrInfo: PADDRINFOW);
    pub fn GetAddrInfoExA(
        pName: PCSTR, pServiceName: PCSTR, dwNameSpace: DWORD, lpNspId: LPGUID,
        hints: *const ADDRINFOEXA, ppResult: *mut PADDRINFOEXA, timeout: *mut timeval,
        lpOverlapped: LPOVERLAPPED, lpCompletionRoutine: LPLOOKUPSERVICE_COMPLETION_ROUTINE,
        lpNameHandle: LPHANDLE,
    ) -> INT;
    pub fn GetAddrInfoExCancel(lpHandle: LPHANDLE) -> INT;
    pub fn GetAddrInfoExOverlappedResult(lpOverlapped: LPOVERLAPPED) -> INT;
    pub fn GetAddrInfoExW(
        pName: PCWSTR, pServiceName: PCWSTR, dwNameSpace: DWORD, lpNspId: LPGUID,
        hints: *const ADDRINFOEXW, ppResult: *mut PADDRINFOEXW, timeout: *mut timeval,
        lpOverlapped: LPOVERLAPPED, lpCompletionRoutine: LPLOOKUPSERVICE_COMPLETION_ROUTINE,
        lpNameHandle: LPHANDLE,
    ) -> INT;
    pub fn GetAddrInfoW(
        pNodeName: PCWSTR, pServiceName: PCWSTR, pHints: *const ADDRINFOW,
        ppResult: *mut PADDRINFOW,
    ) -> INT;
    pub fn GetHostNameW(name: PWSTR, namelen: c_int) -> c_int;
    pub fn GetNameInfoW(
        pSockaddr: *const SOCKADDR, SockaddrLength: socklen_t, pNodeBuffer: PWCHAR,
        NodeBufferSize: DWORD, pServiceBuffer: PWCHAR, ServiceBufferSize: DWORD, Flags: INT,
    ) -> INT;
    pub fn InetNtopW(Family: INT, pAddr: PVOID, pStringBuf: PWSTR, StringBufSize: size_t) -> PCWSTR;
    pub fn InetPtonW(Family: INT, pszAddrString: PCWSTR, pAddrBuf: PVOID) -> INT;
    pub fn SetAddrInfoExA(
        pName: PCSTR, pServiceName: PCSTR, pAddresses: *mut SOCKET_ADDRESS, dwAddressCount: DWORD,
        lpBlob: LPBLOB, dwFlags: DWORD, dwNameSpace: DWORD, lpNspId: LPGUID, timeout: *mut timeval,
        lpOverlapped: LPOVERLAPPED, lpCompletionRoutine: LPLOOKUPSERVICE_COMPLETION_ROUTINE,
        lpNameHandle: LPHANDLE,
    ) -> INT;
    pub fn SetAddrInfoExW(
        pName: PCWSTR, pServiceName: PCWSTR, pAddresses: *mut SOCKET_ADDRESS, dwAddressCount: DWORD,
        lpBlob: LPBLOB, dwFlags: DWORD, dwNameSpace: DWORD, lpNspId: LPGUID, timeout: *mut timeval,
        lpOverlapped: LPOVERLAPPED, lpCompletionRoutine: LPLOOKUPSERVICE_COMPLETION_ROUTINE,
        lpNameHandle: LPHANDLE,
    ) -> INT;
    // pub fn WEP();
    pub fn WPUCompleteOverlappedRequest(
        s: SOCKET, lpOverlapped: LPWSAOVERLAPPED, dwError: DWORD, cbTransferred: DWORD,
        lpErrno: LPINT,
    ) -> c_int;
    // pub fn WPUGetProviderPathEx();
    pub fn WSAAccept(
        s: SOCKET, addr: *mut SOCKADDR, addrlen: LPINT, lpfnCondition: LPCONDITIONPROC,
        dwCallbackData: DWORD_PTR,
    ) -> SOCKET;
    pub fn WSAAddressToStringA(
        lpsaAddress: LPSOCKADDR, dwAddressLength: DWORD, lpProtocolInfo: LPWSAPROTOCOL_INFOA,
        lpszAddressString: LPSTR, lpdwAddressStringLength: LPDWORD,
    ) -> INT;
    pub fn WSAAddressToStringW(
        lpsaAddress: LPSOCKADDR, dwAddressLength: DWORD, lpProtocolInfo: LPWSAPROTOCOL_INFOW,
        lpszAddressString: LPWSTR, lpdwAddressStringLength: LPDWORD,
    ) -> INT;
    pub fn WSAAdvertiseProvider(
        puuidProviderId: *const GUID, pNSPv2Routine: *const LPCNSPV2_ROUTINE,
    ) -> INT;
    pub fn WSAAsyncGetHostByAddr(
        hWnd: HWND, wMsg: u_int, addr: *const c_char, len: c_int, _type: c_int, buf: *mut c_char,
        buflen: c_int,
    ) -> HANDLE;
    pub fn WSAAsyncGetHostByName(
        hWnd: HWND, wMsg: u_int, name: *const c_char, buf: *mut c_char, buflen: c_int,
    ) -> HANDLE;
    pub fn WSAAsyncGetProtoByName(
        hWnd: HWND, wMsg: u_int, name: *const c_char, buf: *mut c_char, buflen: c_int,
    ) -> HANDLE;
    pub fn WSAAsyncGetProtoByNumber(
        hWnd: HWND, wMsg: u_int, number: c_int, buf: *mut c_char, buflen: c_int,
    ) -> HANDLE;
    pub fn WSAAsyncGetServByName(
        hWnd: HWND, wMsg: u_int, name: *const c_char, proto: *const c_char, buf: *mut c_char,
        buflen: c_int,
    ) -> HANDLE;
    pub fn WSAAsyncGetServByPort(
        hWnd: HWND, wMsg: u_int, port: c_int, proto: *const c_char, buf: *mut c_char, buflen: c_int,
    ) -> HANDLE;
    pub fn WSAAsyncSelect(s: SOCKET, hWnd: HWND, wMsg: u_int, lEvent: c_long) -> c_int;
    pub fn WSACancelAsyncRequest(hAsyncTaskHandle: HANDLE) -> c_int;
    pub fn WSACancelBlockingCall() -> c_int;
    pub fn WSACleanup() -> c_int;
    pub fn WSACloseEvent(hEvent: WSAEVENT) -> BOOL;
    pub fn WSAConnect(
        s: SOCKET, name: *const SOCKADDR, namelen: c_int, lpCallerData: LPWSABUF,
        lpCalleeData: LPWSABUF, lpSQOS: LPQOS, lpGQOS: LPQOS,
    ) -> c_int;
    pub fn WSAConnectByList(
        s: SOCKET, SocketAddress: PSOCKET_ADDRESS_LIST, LocalAddressLength: LPDWORD,
        LocalAddress: LPSOCKADDR, RemoteAddressLength: LPDWORD, RemoteAddress: LPSOCKADDR,
        timeout: *const timeval, Reserved: LPWSAOVERLAPPED,
    ) -> BOOL;
    pub fn WSAConnectByNameA(
        s: SOCKET, nodename: LPCSTR, servicename: LPCSTR, LocalAddressLength: LPDWORD,
        LocalAddress: LPSOCKADDR, RemoteAddressLength: LPDWORD, RemoteAddress: LPSOCKADDR,
        timeout: *const timeval, Reserved: LPWSAOVERLAPPED,
    ) -> BOOL;
    pub fn WSAConnectByNameW(
        s: SOCKET, nodename: LPWSTR, servicename: LPWSTR, LocalAddressLength: LPDWORD,
        LocalAddress: LPSOCKADDR, RemoteAddressLength: LPDWORD, RemoteAddress: LPSOCKADDR,
        timeout: *const timeval, Reserved: LPWSAOVERLAPPED,
    ) -> BOOL;
    pub fn WSACreateEvent() -> WSAEVENT;
    pub fn WSADuplicateSocketA(
        s: SOCKET, dwProcessId: DWORD, lpProtocolInfo: LPWSAPROTOCOL_INFOA,
    ) -> c_int;
    pub fn WSADuplicateSocketW(
        s: SOCKET, dwProcessId: DWORD, lpProtocolInfo: LPWSAPROTOCOL_INFOW,
    ) -> c_int;
    pub fn WSAEnumNameSpaceProvidersA(
        lpdwBufferLength: LPDWORD, lpnspBuffer: LPWSANAMESPACE_INFOA,
    ) -> INT;
    pub fn WSAEnumNameSpaceProvidersExA(
        lpdwBufferLength: LPDWORD, lpnspBuffer: LPWSANAMESPACE_INFOEXA,
    ) -> INT;
    pub fn WSAEnumNameSpaceProvidersExW(
        lpdwBufferLength: LPDWORD, lpnspBuffer: LPWSANAMESPACE_INFOEXW,
    ) -> INT;
    pub fn WSAEnumNameSpaceProvidersW(
        lpdwBufferLength: LPDWORD, lpnspBuffer: LPWSANAMESPACE_INFOW,
    ) -> INT;
    pub fn WSAEnumNetworkEvents(
        s: SOCKET, hEventObject: WSAEVENT, lpNetworkEvents: LPWSANETWORKEVENTS,
    ) -> c_int;
    pub fn WSAEnumProtocolsA(
        lpiProtocols: LPINT, lpProtocolBuffer: LPWSAPROTOCOL_INFOA, lpdwBufferLength: LPDWORD,
    ) -> c_int;
    pub fn WSAEnumProtocolsW(
        lpiProtocols: LPINT, lpProtocolBuffer: LPWSAPROTOCOL_INFOW, lpdwBufferLength: LPDWORD,
    ) -> c_int;
    pub fn WSAEventSelect(s: SOCKET, hEventObject: WSAEVENT, lNetworkEvents: c_long) -> c_int;
    pub fn WSAGetLastError() -> c_int;
    pub fn WSAGetOverlappedResult(
        s: SOCKET, lpOverlapped: LPWSAOVERLAPPED, lpcbTransfer: LPDWORD, fWait: BOOL,
        lpdwFlags: LPDWORD,
    ) -> BOOL;
    pub fn WSAGetQOSByName(s: SOCKET, lpQOSName: LPWSABUF, lpQOS: LPQOS) -> BOOL;
    pub fn WSAGetServiceClassInfoA(
        lpProviderId: LPGUID, lpServiceClassId: LPGUID, lpdwBufSize: LPDWORD,
        lpServiceClassInfo: LPWSASERVICECLASSINFOA,
    ) -> INT;
    pub fn WSAGetServiceClassInfoW(
        lpProviderId: LPGUID, lpServiceClassId: LPGUID, lpdwBufSize: LPDWORD,
        lpServiceClassInfo: LPWSASERVICECLASSINFOW,
    ) -> INT;
    pub fn WSAGetServiceClassNameByClassIdA(
        lpServiceClassId: LPGUID, lpszServiceClassName: LPSTR, lpdwBufferLength: LPDWORD,
    ) -> INT;
    pub fn WSAGetServiceClassNameByClassIdW(
        lpServiceClassId: LPGUID, lpszServiceClassName: LPWSTR, lpdwBufferLength: LPDWORD,
    ) -> INT;
    pub fn WSAHtonl(s: SOCKET, hostlong: u_long, lpnetlong: *mut u_long) -> c_int;
    pub fn WSAHtons(s: SOCKET, hostshort: u_short, lpnetshort: *mut u_short) -> c_int;
    pub fn WSAInstallServiceClassA(lpServiceClassInfo: LPWSASERVICECLASSINFOA) -> INT;
    pub fn WSAInstallServiceClassW(lpServiceClassInfo: LPWSASERVICECLASSINFOW) -> INT;
    pub fn WSAIoctl(
        s: SOCKET, dwIoControlCode: DWORD, lpvInBuffer: LPVOID, cbInBuffer: DWORD,
        lpvOutBuffer: LPVOID, cbOutBuffer: DWORD, lpcbBytesReturned: LPDWORD,
        lpOverlapped: LPWSAOVERLAPPED, lpCompletionRoutine: LPWSAOVERLAPPED_COMPLETION_ROUTINE,
    ) -> c_int;
    pub fn WSAIsBlocking() -> BOOL;
    pub fn WSAJoinLeaf(
        s: SOCKET, name: *const SOCKADDR, namelen: c_int, lpCallerData: LPWSABUF,
        lpCalleeData: LPWSABUF, lpSQOS: LPQOS, lpGQOS: LPQOS, dwFlags: DWORD,
    ) -> SOCKET;
    pub fn WSALookupServiceBeginA(
        lpqsRestrictions: LPWSAQUERYSETA, dwControlFlags: DWORD, lphLookup: LPHANDLE,
    ) -> INT;
    pub fn WSALookupServiceBeginW(
        lpqsRestrictions: LPWSAQUERYSETW, dwControlFlags: DWORD, lphLookup: LPHANDLE,
    ) -> INT;
    pub fn WSALookupServiceEnd(hLookup: HANDLE) -> INT;
    pub fn WSALookupServiceNextA(
        hLookup: HANDLE, dwControlFlags: DWORD, lpdwBufferLength: LPDWORD,
        lpqsResults: LPWSAQUERYSETA,
    ) -> INT;
    pub fn WSALookupServiceNextW(
        hLookup: HANDLE, dwControlFlags: DWORD, lpdwBufferLength: LPDWORD,
        lpqsResults: LPWSAQUERYSETW,
    ) -> INT;
    pub fn WSANSPIoctl(
        hLookup: HANDLE, dwControlFlags: DWORD, lpvInBuffer: LPVOID, cbInBuffer: DWORD,
        lpvOutBuffer: LPVOID, cbOutBuffer: DWORD, lpcbBytesReturned: LPDWORD,
        lpCompletion: LPWSACOMPLETION,
    ) -> INT;
    pub fn WSANtohl(s: SOCKET, netlong: u_long, lphostlong: *mut c_long) -> c_int;
    pub fn WSANtohs(s: SOCKET, netshort: u_short, lphostshort: *mut c_short) -> c_int;
    pub fn WSAPoll(fdArray: LPWSAPOLLFD, fds: ULONG, timeout: INT) -> c_int;
    pub fn WSAProviderCompleteAsyncCall(hAsyncCall: HANDLE, iRetCode: INT) -> INT;
    pub fn WSAProviderConfigChange(
        lpNotificationHandle: LPHANDLE, lpOverlapped: LPWSAOVERLAPPED,
        lpCompletionRoutine: LPWSAOVERLAPPED_COMPLETION_ROUTINE,
    ) -> INT;
    pub fn WSARecv(
        s: SOCKET, lpBuffers: LPWSABUF, dwBufferCount: DWORD, lpNumberOfBytesRecvd: LPDWORD,
        lpFlags: LPDWORD, lpOverlapped: LPWSAOVERLAPPED,
        lpCompletionRoutine: LPWSAOVERLAPPED_COMPLETION_ROUTINE,
    ) -> c_int;
    pub fn WSARecvDisconnect(s: SOCKET, lpInboundDisconnectData: LPWSABUF) -> c_int;
    pub fn WSARecvFrom(
        s: SOCKET, lpBuffers: LPWSABUF, dwBufferCount: DWORD, lpNumberOfBytesRecvd: LPDWORD,
        lpFlags: LPDWORD, lpFrom: *mut SOCKADDR, lpFromlen: LPINT, lpOverlapped: LPWSAOVERLAPPED,
        lpCompletionRoutine: LPWSAOVERLAPPED_COMPLETION_ROUTINE,
    ) -> c_int;
    pub fn WSARemoveServiceClass(lpServiceClassId: LPGUID) -> INT;
    pub fn WSAResetEvent(hEvent: WSAEVENT) -> BOOL;
    pub fn WSASend(
        s: SOCKET, lpBuffers: LPWSABUF, dwBufferCount: DWORD, lpNumberOfBytesSent: LPDWORD,
        dwFlags: DWORD, lpOverlapped: LPWSAOVERLAPPED,
        lpCompletionRoutine: LPWSAOVERLAPPED_COMPLETION_ROUTINE,
    ) -> c_int;
    pub fn WSASendDisconnect(s: SOCKET, lpOutboundDisconnectData: LPWSABUF) -> c_int;
    pub fn WSASendMsg(
        Handle: SOCKET, lpMsg: LPWSAMSG, dwFlags: DWORD, lpNumberOfBytesSent: LPDWORD,
        lpOverlapped: LPWSAOVERLAPPED, lpCompletionRoutine: LPWSAOVERLAPPED_COMPLETION_ROUTINE,
    ) -> c_int;
    pub fn WSASendTo(
        s: SOCKET, lpBuffers: LPWSABUF, dwBufferCount: DWORD, lpNumberOfBytesSent: LPDWORD,
        dwFlags: DWORD, lpTo: *const SOCKADDR, iToLen: c_int, lpOverlapped: LPWSAOVERLAPPED,
        lpCompletionRoutine: LPWSAOVERLAPPED_COMPLETION_ROUTINE,
    ) -> c_int;
    pub fn WSASetBlockingHook(lpBlockFunc: FARPROC) -> FARPROC;
    pub fn WSASetEvent(hEvent: WSAEVENT) -> BOOL;
    pub fn WSASetLastError(iError: c_int);
    pub fn WSASetServiceA(
        lpqsRegInfo: LPWSAQUERYSETA, essoperation: WSAESETSERVICEOP, dwControlFlags: DWORD,
    ) -> INT;
    pub fn WSASetServiceW(
        lpqsRegInfo: LPWSAQUERYSETW, essoperation: WSAESETSERVICEOP, dwControlFlags: DWORD,
    ) -> INT;
    pub fn WSASocketA(
        af: c_int, _type: c_int, protocol: c_int, lpProtocolInfo: LPWSAPROTOCOL_INFOA, g: GROUP,
        dwFlags: DWORD,
    ) -> SOCKET;
    pub fn WSASocketW(
        af: c_int, _type: c_int, protocol: c_int, lpProtocolInfo: LPWSAPROTOCOL_INFOW, g: GROUP,
        dwFlags: DWORD,
    ) -> SOCKET;
    pub fn WSAStartup(wVersionRequested: WORD, lpWSAData: LPWSADATA) -> c_int;
    pub fn WSAStringToAddressA(
        AddressString: LPSTR, AddressFamily: INT, lpProtocolInfo: LPWSAPROTOCOL_INFOA,
        lpAddress: LPSOCKADDR, lpAddressLength: LPINT,
    ) -> INT;
    pub fn WSAStringToAddressW(
        AddressString: LPWSTR, AddressFamily: INT, lpProtocolInfo: LPWSAPROTOCOL_INFOW,
        lpAddress: LPSOCKADDR, lpAddressLength: LPINT,
    ) -> INT;
    pub fn WSAUnadvertiseProvider(puuidProviderId: *const GUID) -> INT;
    pub fn WSAUnhookBlockingHook() -> c_int;
    pub fn WSAWaitForMultipleEvents(
        cEvents: DWORD, lphEvents: *const WSAEVENT, fWaitAll: BOOL, dwTimeout: DWORD,
        fAlertable: BOOL,
    ) -> DWORD;
    pub fn WSCDeinstallProvider(lpProviderId: LPGUID, lpErrno: LPINT) -> c_int;
    // pub fn WSCDeinstallProviderEx();
    pub fn WSCEnableNSProvider(lpProviderId: LPGUID, fEnable: BOOL) -> INT;
    pub fn WSCEnumProtocols(
        lpiProtocols: LPINT, lpProtocolBuffer: LPWSAPROTOCOL_INFOW, lpdwBufferLength: LPDWORD,
        lpErrno: LPINT,
    ) -> c_int;
    // pub fn WSCEnumProtocolsEx();
    pub fn WSCGetApplicationCategory(
        Path: LPCWSTR, PathLength: DWORD, Extra: LPCWSTR, ExtraLength: DWORD,
        pPermittedLspCategories: *mut DWORD, lpErrno: LPINT,
    ) -> c_int;
    // pub fn WSCGetApplicationCategoryEx();
    pub fn WSCGetProviderInfo(
        lpProviderId: LPGUID, InfoType: WSC_PROVIDER_INFO_TYPE, Info: PBYTE, InfoSize: *mut size_t,
        Flags: DWORD, lpErrno: LPINT,
    ) -> c_int;
    pub fn WSCGetProviderPath(
        lpProviderId: LPGUID, lpszProviderDllPath: *mut WCHAR, lpProviderDllPathLen: LPINT,
        lpErrno: LPINT,
    ) -> c_int;
    pub fn WSCInstallNameSpace(
        lpszIdentifier: LPWSTR, lpszPathName: LPWSTR, dwNameSpace: DWORD, dwVersion: DWORD,
        lpProviderId: LPGUID,
    ) -> INT;
    pub fn WSCInstallNameSpaceEx(
        lpszIdentifier: LPWSTR, lpszPathName: LPWSTR, dwNameSpace: DWORD, dwVersion: DWORD,
        lpProviderId: LPGUID, lpProviderSpecific: LPBLOB,
    ) -> INT;
    // pub fn WSCInstallNameSpaceEx2();
    pub fn WSCInstallProvider(
        lpProviderId: LPGUID, lpszProviderDllPath: *const WCHAR,
        lpProtocolInfoList: LPWSAPROTOCOL_INFOW, dwNumberOfEntries: DWORD, lpErrno: LPINT,
    ) -> c_int;
    // pub fn WSCInstallProviderEx();
    pub fn WSCSetApplicationCategory(
        Path: LPCWSTR, PathLength: DWORD, Extra: LPCWSTR, ExtraLength: DWORD,
        PermittedLspCategories: DWORD, pPrevPermLspCat: *mut DWORD, lpErrno: LPINT,
    ) -> c_int;
    // pub fn WSCSetApplicationCategoryEx();
    pub fn WSCSetProviderInfo(
        lpProviderId: LPGUID, InfoType: WSC_PROVIDER_INFO_TYPE, Info: PBYTE, InfoSize: size_t,
        Flags: DWORD, lpErrno: LPINT,
    ) -> c_int;
    pub fn WSCUnInstallNameSpace(lpProviderId: LPGUID) -> INT;
    // pub fn WSCUnInstallNameSpaceEx2();
    pub fn WSCUpdateProvider(
        lpProviderId: LPGUID, lpszProviderDllPath: *const WCHAR,
        lpProtocolInfoList: LPWSAPROTOCOL_INFOW, dwNumberOfEntries: DWORD, lpErrno: LPINT,
    ) -> c_int;
    // pub fn WSCUpdateProviderEx();
    pub fn WSCWriteNameSpaceOrder(lpProviderId: LPGUID, dwNumberOfEntries: DWORD) -> c_int;
    pub fn WSCWriteProviderOrder(lpwdCatalogEntryId: LPDWORD, dwNumberOfEntries: DWORD) -> c_int;
    // pub fn WSCWriteProviderOrderEx();
    // pub fn WahCloseApcHelper();
    // pub fn WahCloseHandleHelper();
    // pub fn WahCloseNotificationHandleHelper();
    // pub fn WahCloseSocketHandle();
    // pub fn WahCloseThread();
    // pub fn WahCompleteRequest();
    // pub fn WahCreateHandleContextTable();
    // pub fn WahCreateNotificationHandle();
    // pub fn WahCreateSocketHandle();
    // pub fn WahDestroyHandleContextTable();
    // pub fn WahDisableNonIFSHandleSupport();
    // pub fn WahEnableNonIFSHandleSupport();
    // pub fn WahEnumerateHandleContexts();
    // pub fn WahInsertHandleContext();
    // pub fn WahNotifyAllProcesses();
    // pub fn WahOpenApcHelper();
    // pub fn WahOpenCurrentThread();
    // pub fn WahOpenHandleHelper();
    // pub fn WahOpenNotificationHandleHelper();
    // pub fn WahQueueUserApc();
    // pub fn WahReferenceContextByHandle();
    // pub fn WahRemoveHandleContext();
    // pub fn WahWaitForNotification();
    // pub fn WahWriteLSPEvent();
    pub fn __WSAFDIsSet(fd: SOCKET, _: *mut fd_set) -> c_int;
    pub fn accept(s: SOCKET, addr: *mut SOCKADDR, addrlen: *mut c_int) -> SOCKET;
    pub fn bind(s: SOCKET, name: *const SOCKADDR, namelen: c_int) -> c_int;
    pub fn closesocket(s: SOCKET) -> c_int;
    pub fn connect(s: SOCKET, name: *const SOCKADDR, namelen: c_int) -> c_int;
    pub fn freeaddrinfo(pAddrInfo: PADDRINFOA);
    pub fn getaddrinfo(
        pNodeName: PCSTR, pServiceName: PCSTR, pHints: *const ADDRINFOA, ppResult: *mut PADDRINFOA,
    ) -> INT;
    pub fn gethostbyaddr(addr: *const c_char, len: c_int, _type: c_int) -> *mut hostent;
    pub fn gethostbyname(name: *const c_char) -> *mut hostent;
    pub fn gethostname(name: *mut c_char, namelen: c_int) -> c_int;
    pub fn getnameinfo(
        pSockaddr: *const SOCKADDR, SockaddrLength: socklen_t, pNodeBuffer: PCHAR,
        NodeBufferSize: DWORD, pServiceBuffer: PCHAR, ServiceBufferSize: DWORD, Flags: INT,
    ) -> INT;
    pub fn getpeername(s: SOCKET, name: *mut SOCKADDR, namelen: *mut c_int) -> c_int;
    pub fn getprotobyname(name: *const c_char) -> *mut protoent;
    pub fn getprotobynumber(number: c_int) -> *mut protoent;
    pub fn getservbyname(name: *const c_char, proto: *const c_char) -> *mut servent;
    pub fn getservbyport(port: c_int, proto: *const c_char) -> *mut servent;
    pub fn getsockname(s: SOCKET, name: *mut SOCKADDR, namelen: *mut c_int) -> c_int;
    pub fn getsockopt(
        s: SOCKET, level: c_int, optname: c_int, optval: *mut c_char, optlen: *mut c_int,
    ) -> c_int;
    pub fn htonl(hostlong: u_long) -> u_long;
    pub fn htons(hostshort: u_short) -> u_short;
    pub fn inet_addr(cp: *const c_char) -> c_ulong;
    pub fn inet_ntoa(_in: in_addr) -> *mut c_char;
    pub fn inet_ntop(Family: INT, pAddr: PVOID, pStringBuf: PSTR, StringBufSize: size_t) -> PCSTR;
    pub fn inet_pton(Family: INT, pszAddrString: PCSTR, pAddrBuf: PVOID) -> INT;
    pub fn ioctlsocket(s: SOCKET, cmd: c_long, argp: *mut u_long) -> c_int;
    pub fn listen(s: SOCKET, backlog: c_int) -> c_int;
    pub fn ntohl(netlong: u_long) -> u_long;
    pub fn ntohs(netshort: u_short) -> u_short;
    pub fn recv(s: SOCKET, buf: *mut c_char, len: c_int, flags: c_int) -> c_int;
    pub fn recvfrom(
        s: SOCKET, buf: *mut c_char, len: c_int, flags: c_int, from: *mut SOCKADDR,
        fromlen: *mut c_int,
    ) -> c_int;
    pub fn select(
        nfds: c_int, readfds: *mut fd_set, writefds: *mut fd_set, exceptfds: *mut fd_set,
        timeout: *const timeval,
    ) -> c_int;
    pub fn send(s: SOCKET, buf: *const c_char, len: c_int, flags: c_int) -> c_int;
    pub fn sendto(
        s: SOCKET, buf: *const c_char, len: c_int, flags: c_int, to: *const SOCKADDR, tolen: c_int,
    ) -> c_int;
    pub fn setsockopt(
        s: SOCKET, level: c_int, optname: c_int, optval: *const c_char, optlen: c_int,
    ) -> c_int;
    pub fn shutdown(s: SOCKET, how: c_int) -> c_int;
    pub fn socket(af: c_int, _type: c_int, protocol: c_int) -> SOCKET;
}
#[cfg(any(target_arch = "x86", target_arch = "arm"))]
extern "system" {
    pub fn WSCInstallProviderAndChains(
        lpProviderId: LPGUID, lpszProviderDllPath: LPWSTR, lpszLspName: LPWSTR,
        dwServiceFlags: DWORD, lpProtocolInfoList: LPWSAPROTOCOL_INFOW, dwNumberOfEntries: DWORD,
        lpdwCatalogEntryId: LPDWORD, lpErrno: LPINT,
    ) -> c_int;
}
#[cfg(target_arch = "x86_64")]
extern "system" {
    pub fn WSCDeinstallProvider32(lpProviderId: LPGUID, lpErrno: LPINT) -> c_int;
    pub fn WSCEnableNSProvider32(lpProviderId: LPGUID, fEnable: BOOL) -> INT;
    pub fn WSCEnumNameSpaceProviders32(
        lpdwBufferLength: LPDWORD, lpnspBuffer: LPWSANAMESPACE_INFOW,
    ) -> INT;
    pub fn WSCEnumNameSpaceProvidersEx32(
        lpdwBufferLength: LPDWORD, lpnspBuffer: LPWSANAMESPACE_INFOEXW,
    ) -> INT;
    pub fn WSCEnumProtocols32(
        lpiProtocols: LPINT, lpProtocolBuffer: LPWSAPROTOCOL_INFOW, lpdwBufferLength: LPDWORD,
        lpErrno: LPINT,
    ) -> c_int;
    pub fn WSCGetProviderInfo32(
        lpProviderId: LPGUID, InfoType: WSC_PROVIDER_INFO_TYPE, Info: PBYTE, InfoSize: *mut size_t,
        Flags: DWORD, lpErrno: LPINT,
    ) -> c_int;
    pub fn WSCGetProviderPath32(
        lpProviderId: LPGUID, lpszProviderDllPath: *mut WCHAR, lpProviderDllPathLen: LPINT,
        lpErrno: LPINT,
    ) -> c_int;
    pub fn WSCInstallNameSpace32(
        lpszIdentifier: LPWSTR, lpszPathName: LPWSTR, dwNameSpace: DWORD, dwVersion: DWORD,
        lpProviderId: LPGUID,
    ) -> INT;
    pub fn WSCInstallNameSpaceEx32(
        lpszIdentifier: LPWSTR, lpszPathName: LPWSTR, dwNameSpace: DWORD, dwVersion: DWORD,
        lpProviderId: LPGUID, lpProviderSpecific: LPBLOB,
    ) -> INT;
    pub fn WSCInstallProvider64_32(
        lpProviderId: LPGUID, lpszProviderDllPath: *const WCHAR,
        lpProtocolInfoList: LPWSAPROTOCOL_INFOW, dwNumberOfEntries: DWORD, lpErrno: LPINT,
    ) -> c_int;
    pub fn WSCInstallProviderAndChains64_32(
        lpProviderId: LPGUID, lpszProviderDllPath: LPWSTR, lpszProviderDllPath32: LPWSTR,
        lpszLspName: LPWSTR, dwServiceFlags: DWORD, lpProtocolInfoList: LPWSAPROTOCOL_INFOW,
        dwNumberOfEntries: DWORD, lpdwCatalogEntryId: LPDWORD, lpErrno: LPINT,
    ) -> c_int;
    pub fn WSCSetProviderInfo32(
        lpProviderId: LPGUID, InfoType: WSC_PROVIDER_INFO_TYPE, Info: PBYTE, InfoSize: size_t,
        Flags: DWORD, lpErrno: LPINT,
    ) -> c_int;
    pub fn WSCUnInstallNameSpace32(lpProviderId: LPGUID) -> INT;
    pub fn WSCUpdateProvider32(
        lpProviderId: LPGUID, lpszProviderDllPath: *const WCHAR,
        lpProtocolInfoList: LPWSAPROTOCOL_INFOW, dwNumberOfEntries: DWORD, lpErrno: LPINT,
    ) -> c_int;
    pub fn WSCWriteNameSpaceOrder32(lpProviderId: LPGUID, dwNumberOfEntries: DWORD) -> c_int;
    pub fn WSCWriteProviderOrder32(lpwdCatalogEntryId: LPDWORD, dwNumberOfEntries: DWORD) -> c_int;
}
extern {
    // pub static AddressFamilyInformation;
    // pub static eui48_broadcast;
    // pub static in4addr_alligmpv3routersonlink;
    // pub static in4addr_allnodesonlink;
    // pub static in4addr_allroutersonlink;
    // pub static in4addr_allteredohostsonlink;
    // pub static in4addr_any;
    // pub static in4addr_broadcast;
    // pub static in4addr_linklocalprefix;
    // pub static in4addr_loopback;
    // pub static in4addr_multicastprefix;
    // pub static in6addr_6to4prefix;
    // pub static in6addr_allmldv2routersonlink;
    // pub static in6addr_allnodesonlink;
    // pub static in6addr_allnodesonnode;
    // pub static in6addr_allroutersonlink;
    // pub static in6addr_any;
    // pub static in6addr_linklocalprefix;
    // pub static in6addr_loopback;
    // pub static in6addr_multicastprefix;
    // pub static in6addr_solicitednodemulticastprefix;
    // pub static in6addr_teredoinitiallinklocaladdress;
    // pub static in6addr_teredoprefix;
    // pub static in6addr_teredoprefix_old;
    // pub static in6addr_v4mappedprefix;
    // pub static scopeid_unspecified;
    // pub static sockaddr_size;
}
