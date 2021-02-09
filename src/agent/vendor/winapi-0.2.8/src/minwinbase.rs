// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! This module defines the 32-Bit Windows Base APIs
STRUCT!{struct SECURITY_ATTRIBUTES {
    nLength: ::DWORD,
    lpSecurityDescriptor: ::LPVOID,
    bInheritHandle: ::BOOL,
}}
pub type PSECURITY_ATTRIBUTES = *mut SECURITY_ATTRIBUTES;
pub type LPSECURITY_ATTRIBUTES = *mut SECURITY_ATTRIBUTES;
STRUCT!{struct OVERLAPPED {
    Internal: ::ULONG_PTR,
    InternalHigh: ::ULONG_PTR,
    Offset: ::DWORD,
    OffsetHigh: ::DWORD,
    hEvent: ::HANDLE,
}}
UNION!(OVERLAPPED, Offset, Pointer, Pointer_mut, ::PVOID);
pub type LPOVERLAPPED = *mut OVERLAPPED;
STRUCT!{struct OVERLAPPED_ENTRY {
    lpCompletionKey: ::ULONG_PTR,
    lpOverlapped: LPOVERLAPPED,
    Internal: ::ULONG_PTR,
    dwNumberOfBytesTransferred: ::DWORD,
}}
pub type LPOVERLAPPED_ENTRY = *mut OVERLAPPED_ENTRY;
STRUCT!{struct SYSTEMTIME {
    wYear: ::WORD,
    wMonth: ::WORD,
    wDayOfWeek: ::WORD,
    wDay: ::WORD,
    wHour: ::WORD,
    wMinute: ::WORD,
    wSecond: ::WORD,
    wMilliseconds: ::WORD,
}}
pub type PSYSTEMTIME = *mut SYSTEMTIME;
pub type LPSYSTEMTIME = *mut SYSTEMTIME;
STRUCT!{nodebug struct WIN32_FIND_DATAA {
    dwFileAttributes: ::DWORD,
    ftCreationTime: ::FILETIME,
    ftLastAccessTime: ::FILETIME,
    ftLastWriteTime: ::FILETIME,
    nFileSizeHigh: ::DWORD,
    nFileSizeLow: ::DWORD,
    dwReserved0: ::DWORD,
    dwReserved1: ::DWORD,
    cFileName: [::CHAR; ::MAX_PATH],
    cAlternateFileName: [::CHAR; 14],
}}
pub type PWIN32_FIND_DATAA = *mut WIN32_FIND_DATAA;
pub type LPWIN32_FIND_DATAA = *mut WIN32_FIND_DATAA;
STRUCT!{nodebug struct WIN32_FIND_DATAW {
    dwFileAttributes: ::DWORD,
    ftCreationTime: ::FILETIME,
    ftLastAccessTime: ::FILETIME,
    ftLastWriteTime: ::FILETIME,
    nFileSizeHigh: ::DWORD,
    nFileSizeLow: ::DWORD,
    dwReserved0: ::DWORD,
    dwReserved1: ::DWORD,
    cFileName: [::WCHAR; ::MAX_PATH],
    cAlternateFileName: [::WCHAR; 14],
}}
pub type PWIN32_FIND_DATAW = *mut WIN32_FIND_DATAW;
pub type LPWIN32_FIND_DATAW = *mut WIN32_FIND_DATAW;
ENUM!{enum FINDEX_INFO_LEVELS {
    FindExInfoStandard,
    FindExInfoBasic,
    FindExInfoMaxInfoLevel,
}}
pub const FIND_FIRST_EX_CASE_SENSITIVE: ::DWORD = 0x00000001;
pub const FIND_FIRST_EX_LARGE_FETCH: ::DWORD = 0x00000002;
ENUM!{enum FINDEX_SEARCH_OPS {
    FindExSearchNameMatch,
    FindExSearchLimitToDirectories,
    FindExSearchLimitToDevices,
    FindExSearchMaxSearchOp,
}}
ENUM!{enum GET_FILEEX_INFO_LEVELS {
    GetFileExInfoStandard,
    GetFileExMaxInfoLevel,
}}
ENUM!{enum FILE_INFO_BY_HANDLE_CLASS {
    FileBasicInfo,
    FileStandardInfo,
    FileNameInfo,
    FileRenameInfo,
    FileDispositionInfo,
    FileAllocationInfo,
    FileEndOfFileInfo,
    FileStreamInfo,
    FileCompressionInfo,
    FileAttributeTagInfo,
    FileIdBothDirectoryInfo,
    FileIdBothDirectoryRestartInfo,
    FileIoPriorityHintInfo,
    FileRemoteProtocolInfo,
    FileFullDirectoryInfo,
    FileFullDirectoryRestartInfo,
    FileStorageInfo,
    FileAlignmentInfo,
    FileIdInfo,
    FileIdExtdDirectoryInfo,
    FileIdExtdDirectoryRestartInfo,
    MaximumFileInfoByHandleClass,
}}
pub type PFILE_INFO_BY_HANDLE_CLASS = *mut FILE_INFO_BY_HANDLE_CLASS;
pub type CRITICAL_SECTION = ::RTL_CRITICAL_SECTION;
pub type PCRITICAL_SECTION = ::PRTL_CRITICAL_SECTION;
pub type LPCRITICAL_SECTION = ::PRTL_CRITICAL_SECTION;
pub type CRITICAL_SECTION_DEBUG = ::RTL_CRITICAL_SECTION_DEBUG;
pub type PCRITICAL_SECTION_DEBUG = ::PRTL_CRITICAL_SECTION_DEBUG;
pub type LPCRITICAL_SECTION_DEBUG = ::PRTL_CRITICAL_SECTION_DEBUG;
pub type LPOVERLAPPED_COMPLETION_ROUTINE = Option<unsafe extern "system" fn(
    dwErrorCode: ::DWORD, dwNumberOfBytesTransfered: ::DWORD, lpOverlapped: LPOVERLAPPED,
)>;
pub const LOCKFILE_FAIL_IMMEDIATELY: ::DWORD = 0x00000001;
pub const LOCKFILE_EXCLUSIVE_LOCK: ::DWORD = 0x00000002;
STRUCT!{struct PROCESS_HEAP_ENTRY_Block {
    hMem: ::HANDLE,
    dwReserved: [::DWORD; 3],
}}
STRUCT!{struct PROCESS_HEAP_ENTRY_Region {
    dwCommittedSize: ::DWORD,
    dwUnCommittedSize: ::DWORD,
    lpFirstBlock: ::LPVOID,
    lpLastBlock: ::LPVOID,
}}
STRUCT!{struct PROCESS_HEAP_ENTRY {
    lpData: ::PVOID,
    cbData: ::DWORD,
    cbOverhead: ::BYTE,
    iRegionIndex: ::BYTE,
    wFlags: ::WORD,
    Region: PROCESS_HEAP_ENTRY_Region,
}}
UNION!(PROCESS_HEAP_ENTRY, Region, Block, Block_mut, PROCESS_HEAP_ENTRY_Block);
pub type LPPROCESS_HEAP_ENTRY = *mut PROCESS_HEAP_ENTRY;
pub type PPROCESS_HEAP_ENTRY = *mut PROCESS_HEAP_ENTRY;
pub const PROCESS_HEAP_REGION: ::WORD = 0x0001;
pub const PROCESS_HEAP_UNCOMMITTED_RANGE: ::WORD = 0x0002;
pub const PROCESS_HEAP_ENTRY_BUSY: ::WORD = 0x0004;
pub const PROCESS_HEAP_SEG_ALLOC: ::WORD = 0x0008;
pub const PROCESS_HEAP_ENTRY_MOVEABLE: ::WORD = 0x0010;
pub const PROCESS_HEAP_ENTRY_DDESHARE: ::WORD = 0x0020;
pub type PTHREAD_START_ROUTINE = Option<unsafe extern "system" fn(
    lpThreadParameter: ::LPVOID,
) -> ::DWORD>;
pub type LPTHREAD_START_ROUTINE = PTHREAD_START_ROUTINE;
pub type LPCONTEXT = ::PCONTEXT;
STRUCT!{struct REASON_CONTEXT_Detailed {
    LocalizedReasonModule: ::HMODULE,
    LocalizedReasonId: ::ULONG,
    ReasonStringCount: ::ULONG,
    ReasonStrings: *mut ::LPWSTR,
}}
STRUCT!{struct REASON_CONTEXT {
    Version: ::ULONG,
    Flags: ::DWORD,
    Reason: REASON_CONTEXT_Detailed,
}}
UNION!(REASON_CONTEXT, Reason, SimpleReasonString, SimpleReasonString_mut, ::LPWSTR);
pub type PREASON_CONTEXT = *mut REASON_CONTEXT;
pub const EXCEPTION_DEBUG_EVENT: ::DWORD = 1;
pub const CREATE_THREAD_DEBUG_EVENT: ::DWORD = 2;
pub const CREATE_PROCESS_DEBUG_EVENT: ::DWORD = 3;
pub const EXIT_THREAD_DEBUG_EVENT: ::DWORD = 4;
pub const EXIT_PROCESS_DEBUG_EVENT: ::DWORD = 5;
pub const LOAD_DLL_DEBUG_EVENT: ::DWORD = 6;
pub const UNLOAD_DLL_DEBUG_EVENT: ::DWORD = 7;
pub const OUTPUT_DEBUG_STRING_EVENT: ::DWORD = 8;
pub const RIP_EVENT: ::DWORD = 9;
STRUCT!{struct EXCEPTION_DEBUG_INFO {
    ExceptionRecord: ::EXCEPTION_RECORD,
    dwFirstChance: ::DWORD,
}}
pub type LPEXCEPTION_DEBUG_INFO = *mut EXCEPTION_DEBUG_INFO;
STRUCT!{nodebug struct CREATE_THREAD_DEBUG_INFO {
    hThread: ::HANDLE,
    lpThreadLocalBase: ::LPVOID,
    lpStartAddress: LPTHREAD_START_ROUTINE,
}}
pub type LPCREATE_THREAD_DEBUG_INFO = *mut CREATE_THREAD_DEBUG_INFO;
STRUCT!{nodebug struct CREATE_PROCESS_DEBUG_INFO {
    hFile: ::HANDLE,
    hProcess: ::HANDLE,
    hThread: ::HANDLE,
    lpBaseOfImage: ::LPVOID,
    dwDebugInfoFileOffset: ::DWORD,
    nDebugInfoSize: ::DWORD,
    lpThreadLocalBase: ::LPVOID,
    lpStartAddress: LPTHREAD_START_ROUTINE,
    lpImageName: ::LPVOID,
    fUnicode: ::WORD,
}}
pub type LPCREATE_PROCESS_DEBUG_INFO = *mut CREATE_PROCESS_DEBUG_INFO;
STRUCT!{struct EXIT_THREAD_DEBUG_INFO {
    dwExitCode: ::DWORD,
}}
pub type LPEXIT_THREAD_DEBUG_INFO = *mut EXIT_THREAD_DEBUG_INFO;
STRUCT!{struct EXIT_PROCESS_DEBUG_INFO {
    dwExitCode: ::DWORD,
}}
pub type LPEXIT_PROCESS_DEBUG_INFO = *mut EXIT_PROCESS_DEBUG_INFO;
STRUCT!{struct LOAD_DLL_DEBUG_INFO {
    hFile: ::HANDLE,
    lpBaseOfDll: ::LPVOID,
    dwDebugInfoFileOffset: ::DWORD,
    nDebugInfoSize: ::DWORD,
    lpImageName: ::LPVOID,
    fUnicode: ::WORD,
}}
pub type LPLOAD_DLL_DEBUG_INFO = *mut LOAD_DLL_DEBUG_INFO;
STRUCT!{struct UNLOAD_DLL_DEBUG_INFO {
    lpBaseOfDll: ::LPVOID,
}}
pub type LPUNLOAD_DLL_DEBUG_INFO = *mut UNLOAD_DLL_DEBUG_INFO;
STRUCT!{struct OUTPUT_DEBUG_STRING_INFO {
    lpDebugStringData: ::LPSTR,
    fUnicode: ::WORD,
    nDebugStringLength: ::WORD,
}}
pub type LPOUTPUT_DEBUG_STRING_INFO = *mut OUTPUT_DEBUG_STRING_INFO;
STRUCT!{struct RIP_INFO {
    dwError: ::DWORD,
    dwType: ::DWORD,
}}
pub type LPRIP_INFO = *mut RIP_INFO;
#[cfg(target_arch="x86_64")]
STRUCT!{nodebug struct DEBUG_EVENT {
    dwDebugEventCode: ::DWORD,
    dwProcessId: ::DWORD,
    dwThreadId: ::DWORD,
    u: [u8; 160],
}}
#[cfg(target_arch="x86")]
STRUCT!{nodebug struct DEBUG_EVENT {
    dwDebugEventCode: ::DWORD,
    dwProcessId: ::DWORD,
    dwThreadId: ::DWORD,
    u: [u8; 84],
}}
UNION!(DEBUG_EVENT, u, Exception, Exception_mut, EXCEPTION_DEBUG_INFO);
UNION!(DEBUG_EVENT, u, CreateThread, CreateThread_mut, CREATE_THREAD_DEBUG_INFO);
UNION!(DEBUG_EVENT, u, CreateProcessInfo, CreateProcessInfo_mut, CREATE_PROCESS_DEBUG_INFO);
UNION!(DEBUG_EVENT, u, ExitThread, ExitThread_mut, EXIT_THREAD_DEBUG_INFO);
UNION!(DEBUG_EVENT, u, ExitProcess, ExitProcess_mut, EXIT_PROCESS_DEBUG_INFO);
UNION!(DEBUG_EVENT, u, LoadDll, LoadDll_mut, LOAD_DLL_DEBUG_INFO);
UNION!(DEBUG_EVENT, u, UnloadDll, UnloadDll_mut, UNLOAD_DLL_DEBUG_INFO);
UNION!(DEBUG_EVENT, u, DebugString, DebugString_mut, OUTPUT_DEBUG_STRING_INFO);
UNION!(DEBUG_EVENT, u, RipInfo, RipInfo_mut, RIP_INFO);
pub type LPDEBUG_EVENT = *mut DEBUG_EVENT;
