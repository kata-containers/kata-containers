// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! Data Protection API Prototypes and Definitions
// This header file provides the definitions and symbols necessary for an
// Application or Smart Card Service Provider to access the Smartcard Subsystem.
pub type LPCBYTE = *const ::BYTE;
pub type SCARDCONTEXT = ::ULONG_PTR;
pub type PSCARDCONTEXT = *mut SCARDCONTEXT;
pub type LPSCARDCONTEXT = *mut SCARDCONTEXT;
pub type SCARDHANDLE = ::ULONG_PTR;
pub type PSCARDHANDLE = *mut SCARDHANDLE;
pub type LPSCARDHANDLE = *mut SCARDHANDLE;
pub const SCARD_AUTOALLOCATE: ::DWORD = -1i32 as ::DWORD;
pub const SCARD_SCOPE_USER: ::DWORD = 0;
pub const SCARD_SCOPE_TERMINAL: ::DWORD = 1;
pub const SCARD_SCOPE_SYSTEM: ::DWORD = 2;
pub const SCARD_PROVIDER_PRIMARY: ::DWORD = 1;
pub const SCARD_PROVIDER_CSP: ::DWORD = 2;
pub const SCARD_PROVIDER_KSP: ::DWORD = 3;
STRUCT!{nodebug struct SCARD_READERSTATEA {
    szReader: ::LPCSTR,
    pvUserData: ::LPVOID,
    dwCurrentState: ::DWORD,
    dwEventState: ::DWORD,
    cbAtr: ::DWORD,
    rgbAtr: [::BYTE; 36],
}}
pub type PSCARD_READERSTATEA = *mut SCARD_READERSTATEA;
pub type LPSCARD_READERSTATEA = *mut SCARD_READERSTATEA;
STRUCT!{nodebug struct SCARD_READERSTATEW {
    szReader: ::LPCWSTR,
    pvUserData: ::LPVOID,
    dwCurrentState: ::DWORD,
    dwEventState: ::DWORD,
    cbAtr: ::DWORD,
    rgbAtr: [::BYTE; 36],
}}
pub type PSCARD_READERSTATEW = *mut SCARD_READERSTATEW;
pub type LPSCARD_READERSTATEW = *mut SCARD_READERSTATEW;
pub type SCARD_READERSTATE_A = SCARD_READERSTATEA;
pub type SCARD_READERSTATE_W = SCARD_READERSTATEW;
pub type PSCARD_READERSTATE_A = PSCARD_READERSTATEA;
pub type PSCARD_READERSTATE_W = PSCARD_READERSTATEW;
pub type LPSCARD_READERSTATE_A = LPSCARD_READERSTATEA;
pub type LPSCARD_READERSTATE_W = LPSCARD_READERSTATEW;
pub const SCARD_STATE_UNAWARE: ::DWORD = 0x00000000;
pub const SCARD_STATE_IGNORE: ::DWORD = 0x00000001;
pub const SCARD_STATE_CHANGED: ::DWORD = 0x00000002;
pub const SCARD_STATE_UNKNOWN: ::DWORD = 0x00000004;
pub const SCARD_STATE_UNAVAILABLE: ::DWORD = 0x00000008;
pub const SCARD_STATE_EMPTY: ::DWORD = 0x00000010;
pub const SCARD_STATE_PRESENT: ::DWORD = 0x00000020;
pub const SCARD_STATE_ATRMATCH: ::DWORD = 0x00000040;
pub const SCARD_STATE_EXCLUSIVE: ::DWORD = 0x00000080;
pub const SCARD_STATE_INUSE: ::DWORD = 0x00000100;
pub const SCARD_STATE_MUTE: ::DWORD = 0x00000200;
pub const SCARD_STATE_UNPOWERED: ::DWORD = 0x00000400;
STRUCT!{nodebug struct SCARD_ATRMASK {
    cbAtr: ::DWORD,
    rgbAtr: [::BYTE; 36],
    rgbMask: [::BYTE; 36],
}}
pub type PSCARD_ATRMASK = *mut SCARD_ATRMASK;
pub type LPSCARD_ATRMASK = *mut SCARD_ATRMASK;
pub const SCARD_SHARE_EXCLUSIVE: ::DWORD = 1;
pub const SCARD_SHARE_SHARED: ::DWORD = 2;
pub const SCARD_SHARE_DIRECT: ::DWORD = 3;
pub const SCARD_LEAVE_CARD: ::DWORD = 0;
pub const SCARD_RESET_CARD: ::DWORD = 1;
pub const SCARD_UNPOWER_CARD: ::DWORD = 2;
pub const SCARD_EJECT_CARD: ::DWORD = 3;
pub const SC_DLG_MINIMAL_UI: ::DWORD = 0x01;
pub const SC_DLG_NO_UI: ::DWORD = 0x02;
pub const SC_DLG_FORCE_UI: ::DWORD = 0x04;
pub const SCERR_NOCARDNAME: ::DWORD = 0x4000;
pub const SCERR_NOGUIDS: ::DWORD = 0x8000;
pub type LPOCNCONNPROCA = Option<unsafe extern "system" fn(
    SCARDCONTEXT, ::LPSTR, ::LPSTR, ::PVOID,
) -> SCARDHANDLE>;
pub type LPOCNCONNPROCW = Option<unsafe extern "system" fn(
    SCARDCONTEXT, ::LPWSTR, ::LPWSTR, ::PVOID,
) -> SCARDHANDLE>;
pub type LPOCNCHKPROC = Option<unsafe extern "system" fn(
    SCARDCONTEXT, SCARDHANDLE, ::PVOID,
) -> ::BOOL>;
pub type LPOCNDSCPROC = Option<unsafe extern "system" fn(SCARDCONTEXT, SCARDHANDLE, ::PVOID)>;
STRUCT!{nodebug struct OPENCARD_SEARCH_CRITERIAA {
    dwStructSize: ::DWORD,
    lpstrGroupNames: ::LPSTR,
    nMaxGroupNames: ::DWORD,
    rgguidInterfaces: ::LPCGUID,
    cguidInterfaces: ::DWORD,
    lpstrCardNames: ::LPSTR,
    nMaxCardNames: ::DWORD,
    lpfnCheck: LPOCNCHKPROC,
    lpfnConnect: LPOCNCONNPROCA,
    lpfnDisconnect: LPOCNDSCPROC,
    pvUserData: ::LPVOID,
    dwShareMode: ::DWORD,
    dwPreferredProtocols: ::DWORD,
}}
pub type POPENCARD_SEARCH_CRITERIAA = *mut OPENCARD_SEARCH_CRITERIAA;
pub type LPOPENCARD_SEARCH_CRITERIAA = *mut OPENCARD_SEARCH_CRITERIAA;
STRUCT!{nodebug struct OPENCARD_SEARCH_CRITERIAW {
    dwStructSize: ::DWORD,
    lpstrGroupNames: ::LPWSTR,
    nMaxGroupNames: ::DWORD,
    rgguidInterfaces: ::LPCGUID,
    cguidInterfaces: ::DWORD,
    lpstrCardNames: ::LPWSTR,
    nMaxCardNames: ::DWORD,
    lpfnCheck: LPOCNCHKPROC,
    lpfnConnect: LPOCNCONNPROCW,
    lpfnDisconnect: LPOCNDSCPROC,
    pvUserData: ::LPVOID,
    dwShareMode: ::DWORD,
    dwPreferredProtocols: ::DWORD,
}}
pub type POPENCARD_SEARCH_CRITERIAW = *mut OPENCARD_SEARCH_CRITERIAW;
pub type LPOPENCARD_SEARCH_CRITERIAW = *mut OPENCARD_SEARCH_CRITERIAW;
STRUCT!{nodebug struct OPENCARDNAME_EXA {
    dwStructSize: ::DWORD,
    hSCardContext: SCARDCONTEXT,
    hwndOwner: ::HWND,
    dwFlags: ::DWORD,
    lpstrTitle: ::LPCSTR,
    lpstrSearchDesc: ::LPCSTR,
    hIcon: ::HICON,
    pOpenCardSearchCriteria: POPENCARD_SEARCH_CRITERIAA,
    lpfnConnect: LPOCNCONNPROCA,
    pvUserData: ::LPVOID,
    dwShareMode: ::DWORD,
    dwPreferredProtocols: ::DWORD,
    lpstrRdr: ::LPSTR,
    nMaxRdr: ::DWORD,
    lpstrCard: ::LPSTR,
    nMaxCard: ::DWORD,
    dwActiveProtocol: ::DWORD,
    hCardHandle: SCARDHANDLE,
}}
pub type POPENCARDNAME_EXA = *mut OPENCARDNAME_EXA;
pub type LPOPENCARDNAME_EXA = *mut OPENCARDNAME_EXA;
STRUCT!{nodebug struct OPENCARDNAME_EXW {
    dwStructSize: ::DWORD,
    hSCardContext: SCARDCONTEXT,
    hwndOwner: ::HWND,
    dwFlags: ::DWORD,
    lpstrTitle: ::LPCWSTR,
    lpstrSearchDesc: ::LPCWSTR,
    hIcon: ::HICON,
    pOpenCardSearchCriteria: POPENCARD_SEARCH_CRITERIAW,
    lpfnConnect: LPOCNCONNPROCW,
    pvUserData: ::LPVOID,
    dwShareMode: ::DWORD,
    dwPreferredProtocols: ::DWORD,
    lpstrRdr: ::LPWSTR,
    nMaxRdr: ::DWORD,
    lpstrCard: ::LPWSTR,
    nMaxCard: ::DWORD,
    dwActiveProtocol: ::DWORD,
    hCardHandle: SCARDHANDLE,
}}
pub type POPENCARDNAME_EXW = *mut OPENCARDNAME_EXW;
pub type LPOPENCARDNAME_EXW = *mut OPENCARDNAME_EXW;
pub type OPENCARDNAMEA_EX = OPENCARDNAME_EXA;
pub type OPENCARDNAMEW_EX = OPENCARDNAME_EXW;
pub type POPENCARDNAMEA_EX = POPENCARDNAME_EXA;
pub type POPENCARDNAMEW_EX = POPENCARDNAME_EXW;
pub type LPOPENCARDNAMEA_EX = LPOPENCARDNAME_EXA;
pub type LPOPENCARDNAMEW_EX = LPOPENCARDNAME_EXW;
pub const SCARD_READER_SEL_AUTH_PACKAGE: ::DWORD = -629i32 as ::DWORD;
ENUM!{enum READER_SEL_REQUEST_MATCH_TYPE {
    RSR_MATCH_TYPE_READER_AND_CONTAINER = 1,
    RSR_MATCH_TYPE_SERIAL_NUMBER,
    RSR_MATCH_TYPE_ALL_CARDS,
}}
STRUCT!{struct READER_SEL_REQUEST_ReaderAndContainerParameter {
    cbReaderNameOffset: ::DWORD,
    cchReaderNameLength: ::DWORD,
    cbContainerNameOffset: ::DWORD,
    cchContainerNameLength: ::DWORD,
    dwDesiredCardModuleVersion: ::DWORD,
    dwCspFlags: ::DWORD,
}}
STRUCT!{struct READER_SEL_REQUEST_SerialNumberParameter {
    cbSerialNumberOffset: ::DWORD,
    cbSerialNumberLength: ::DWORD,
    dwDesiredCardModuleVersion: ::DWORD,
}}
STRUCT!{struct READER_SEL_REQUEST {
    dwShareMode: ::DWORD,
    dwPreferredProtocols: ::DWORD,
    MatchType: READER_SEL_REQUEST_MATCH_TYPE,
    ReaderAndContainerParameter: READER_SEL_REQUEST_ReaderAndContainerParameter,
}}
UNION!(
    READER_SEL_REQUEST, ReaderAndContainerParameter, SerialNumberParameter,
    SerialNumberParameter_mut, READER_SEL_REQUEST_SerialNumberParameter
);
pub type PREADER_SEL_REQUEST = *mut READER_SEL_REQUEST;
STRUCT!{struct READER_SEL_RESPONSE {
    cbReaderNameOffset: ::DWORD,
    cchReaderNameLength: ::DWORD,
    cbCardNameOffset: ::DWORD,
    cchCardNameLength: ::DWORD,
}}
pub type PREADER_SEL_RESPONSE = *mut READER_SEL_RESPONSE;
STRUCT!{nodebug struct OPENCARDNAMEA {
    dwStructSize: ::DWORD,
    hwndOwner: ::HWND,
    hSCardContext: SCARDCONTEXT,
    lpstrGroupNames: ::LPSTR,
    nMaxGroupNames: ::DWORD,
    lpstrCardNames: ::LPSTR,
    nMaxCardNames: ::DWORD,
    rgguidInterfaces: ::LPCGUID,
    cguidInterfaces: ::DWORD,
    lpstrRdr: ::LPSTR,
    nMaxRdr: ::DWORD,
    lpstrCard: ::LPSTR,
    nMaxCard: ::DWORD,
    lpstrTitle: ::LPCSTR,
    dwFlags: ::DWORD,
    pvUserData: ::LPVOID,
    dwShareMode: ::DWORD,
    dwPreferredProtocols: ::DWORD,
    dwActiveProtocol: ::DWORD,
    lpfnConnect: LPOCNCONNPROCA,
    lpfnCheck: LPOCNCHKPROC,
    lpfnDisconnect: LPOCNDSCPROC,
    hCardHandle: SCARDHANDLE,
}}
pub type POPENCARDNAMEA = *mut OPENCARDNAMEA;
pub type LPOPENCARDNAMEA = *mut OPENCARDNAMEA;
STRUCT!{nodebug struct OPENCARDNAMEW {
    dwStructSize: ::DWORD,
    hwndOwner: ::HWND,
    hSCardContext: SCARDCONTEXT,
    lpstrGroupNames: ::LPWSTR,
    nMaxGroupNames: ::DWORD,
    lpstrCardNames: ::LPWSTR,
    nMaxCardNames: ::DWORD,
    rgguidInterfaces: ::LPCGUID,
    cguidInterfaces: ::DWORD,
    lpstrRdr: ::LPWSTR,
    nMaxRdr: ::DWORD,
    lpstrCard: ::LPWSTR,
    nMaxCard: ::DWORD,
    lpstrTitle: ::LPCWSTR,
    dwFlags: ::DWORD,
    pvUserData: ::LPVOID,
    dwShareMode: ::DWORD,
    dwPreferredProtocols: ::DWORD,
    dwActiveProtocol: ::DWORD,
    lpfnConnect: LPOCNCONNPROCW,
    lpfnCheck: LPOCNCHKPROC,
    lpfnDisconnect: LPOCNDSCPROC,
    hCardHandle: SCARDHANDLE,
}}
pub type POPENCARDNAMEW = *mut OPENCARDNAMEW;
pub type LPOPENCARDNAMEW = *mut OPENCARDNAMEW;
pub type OPENCARDNAME_A = OPENCARDNAMEA;
pub type OPENCARDNAME_W = OPENCARDNAMEW;
pub type POPENCARDNAME_A = POPENCARDNAMEA;
pub type POPENCARDNAME_W = POPENCARDNAMEW;
pub type LPOPENCARDNAME_A = LPOPENCARDNAMEA;
pub type LPOPENCARDNAME_W = LPOPENCARDNAMEW;
pub const SCARD_AUDIT_CHV_FAILURE: ::DWORD = 0x0;
pub const SCARD_AUDIT_CHV_SUCCESS: ::DWORD = 0x1;
