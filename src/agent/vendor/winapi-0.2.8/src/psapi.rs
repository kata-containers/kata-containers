// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! API Prototypes and Definitions for PSAPI.DLL
pub const LIST_MODULES_DEFAULT: ::DWORD = 0x0;
pub const LIST_MODULES_32BIT: ::DWORD = 0x01;
pub const LIST_MODULES_64BIT: ::DWORD = 0x02;
pub const LIST_MODULES_ALL: ::DWORD = LIST_MODULES_32BIT | LIST_MODULES_64BIT;
STRUCT!{struct MODULEINFO {
    lpBaseOfDll: ::LPVOID,
    SizeOfImage: ::DWORD,
    EntryPoint: ::LPVOID,
}}
pub type LPMODULEINFO = *mut MODULEINFO;
STRUCT!{struct PSAPI_WORKING_SET_BLOCK {
    Flags: ::ULONG_PTR,
    BitFields: ::ULONG_PTR,
}}
#[cfg(target_arch="x86")]
BITFIELD!(PSAPI_WORKING_SET_BLOCK BitFields: ::ULONG_PTR [
    Protection set_Protection[0..5],
    ShareCount set_ShareCount[5..8],
    Shared set_Shared[8..9],
    Reserved set_Reserved[9..12],
    VirtualPage set_VirtualPage[12..32],
]);
#[cfg(target_arch="x86_64")]
BITFIELD!(PSAPI_WORKING_SET_BLOCK BitFields: ::ULONG_PTR [
    Protection set_Protection[0..5],
    ShareCount set_ShareCount[5..8],
    Shared set_Shared[8..9],
    Reserved set_Reserved[9..12],
    VirtualPage set_VirtualPage[12..64],
]);
pub type PPSAPI_WORKING_SET_BLOCK = *mut PSAPI_WORKING_SET_BLOCK;
STRUCT!{struct PSAPI_WORKING_SET_INFORMATION {
    NumberOfEntries: ::ULONG_PTR,
    WorkingSetInfo: [PSAPI_WORKING_SET_BLOCK; 1],
}}
pub type PPSAPI_WORKING_SET_INFORMATION = *mut PSAPI_WORKING_SET_INFORMATION;
STRUCT!{struct PSAPI_WORKING_SET_EX_BLOCK_Invalid {
    BitFields: ::ULONG_PTR,
}}
#[cfg(target_arch="x86")]
BITFIELD!(PSAPI_WORKING_SET_EX_BLOCK_Invalid BitFields: ::ULONG_PTR [
    Valid set_Valid[0..1],
    Reserved0 set_Reserved0[1..15],
    Shared set_Shared[15..16],
    Reserved1 set_Reserved1[16..31],
    Bad set_Bad[31..32],
]);
#[cfg(target_arch="x86_64")]
BITFIELD!(PSAPI_WORKING_SET_EX_BLOCK_Invalid BitFields: ::ULONG_PTR [
    Valid set_Valid[0..1],
    Reserved0 set_Reserved0[1..15],
    Shared set_Shared[15..16],
    Reserved1 set_Reserved1[16..31],
    Bad set_Bad[31..32],
    ReservedUlong set_ReservedUlong[32..64],
]);
STRUCT!{struct PSAPI_WORKING_SET_EX_BLOCK {
    Flags: ::ULONG_PTR,
    BitFields: ::ULONG_PTR,
}}
#[cfg(target_arch="x86")]
BITFIELD!(PSAPI_WORKING_SET_EX_BLOCK BitFields: ::ULONG_PTR [
    Valid set_Valid[0..1],
    ShareCount set_ShareCount[1..4],
    Win32Protection set_Win32Protection[4..15],
    Shared set_Shared[15..16],
    Node set_Node[16..22],
    Locked set_Locked[22..23],
    LargePage set_LargePage[23..24],
    Reserved set_Reserved[24..31],
    Bad set_Bad[31..32],
]);
#[cfg(target_arch="x86_64")]
BITFIELD!(PSAPI_WORKING_SET_EX_BLOCK BitFields: ::ULONG_PTR [
    Valid set_Valid[0..1],
    ShareCount set_ShareCount[1..4],
    Win32Protection set_Win32Protection[4..15],
    Shared set_Shared[15..16],
    Node set_Node[16..22],
    Locked set_Locked[22..23],
    LargePage set_LargePage[23..24],
    Reserved set_Reserved[24..31],
    Bad set_Bad[31..32],
    ReservedUlong set_ReservedUlong[32..64],
]);
UNION!(
    PSAPI_WORKING_SET_EX_BLOCK, BitFields, Invalid, Invalid_mut, PSAPI_WORKING_SET_EX_BLOCK_Invalid
);
pub type PPSAPI_WORKING_SET_EX_BLOCK = *mut PSAPI_WORKING_SET_EX_BLOCK;
STRUCT!{struct PSAPI_WORKING_SET_EX_INFORMATION {
    VirtualAddress: ::PVOID,
    VirtualAttributes: PSAPI_WORKING_SET_EX_BLOCK,
}}
pub type PPSAPI_WORKING_SET_EX_INFORMATION = *mut PSAPI_WORKING_SET_EX_INFORMATION;
STRUCT!{struct PSAPI_WS_WATCH_INFORMATION {
    FaultingPc: ::LPVOID,
    FaultingVa: ::LPVOID,
}}
pub type PPSAPI_WS_WATCH_INFORMATION = *mut PSAPI_WS_WATCH_INFORMATION;
STRUCT!{struct PSAPI_WS_WATCH_INFORMATION_EX {
    BasicInfo: PSAPI_WS_WATCH_INFORMATION,
    FaultingThreadId: ::ULONG_PTR,
    Flags: ::ULONG_PTR,
}}
pub type PPSAPI_WS_WATCH_INFORMATION_EX = *mut PSAPI_WS_WATCH_INFORMATION_EX;
STRUCT!{struct PROCESS_MEMORY_COUNTERS {
    cb: ::DWORD,
    PageFaultCount: ::DWORD,
    PeakWorkingSetSize: ::SIZE_T,
    WorkingSetSize: ::SIZE_T,
    QuotaPeakPagedPoolUsage: ::SIZE_T,
    QuotaPagedPoolUsage: ::SIZE_T,
    QuotaPeakNonPagedPoolUsage: ::SIZE_T,
    QuotaNonPagedPoolUsage: ::SIZE_T,
    PagefileUsage: ::SIZE_T,
    PeakPagefileUsage: ::SIZE_T,
}}
pub type PPROCESS_MEMORY_COUNTERS = *mut PROCESS_MEMORY_COUNTERS;
STRUCT!{struct PROCESS_MEMORY_COUNTERS_EX {
    cb: ::DWORD,
    PageFaultCount: ::DWORD,
    PeakWorkingSetSize: ::SIZE_T,
    WorkingSetSize: ::SIZE_T,
    QuotaPeakPagedPoolUsage: ::SIZE_T,
    QuotaPagedPoolUsage: ::SIZE_T,
    QuotaPeakNonPagedPoolUsage: ::SIZE_T,
    QuotaNonPagedPoolUsage: ::SIZE_T,
    PagefileUsage: ::SIZE_T,
    PeakPagefileUsage: ::SIZE_T,
    PrivateUsage: ::SIZE_T,
}}
pub type PPROCESS_MEMORY_COUNTERS_EX = *mut PROCESS_MEMORY_COUNTERS_EX;
STRUCT!{struct PERFORMANCE_INFORMATION {
    cb: ::DWORD,
    CommitTotal: ::SIZE_T,
    CommitLimit: ::SIZE_T,
    CommitPeak: ::SIZE_T,
    PhysicalTotal: ::SIZE_T,
    PhysicalAvailable: ::SIZE_T,
    SystemCache: ::SIZE_T,
    KernelTotal: ::SIZE_T,
    KernelPaged: ::SIZE_T,
    KernelNonpaged: ::SIZE_T,
    PageSize: ::SIZE_T,
    HandleCount: ::DWORD,
    ProcessCount: ::DWORD,
    ThreadCount: ::DWORD,
}}
pub type PPERFORMANCE_INFORMATION = *mut PERFORMANCE_INFORMATION;
STRUCT!{struct ENUM_PAGE_FILE_INFORMATION {
    cb: ::DWORD,
    Reserved: ::DWORD,
    TotalSize: ::SIZE_T,
    TotalInUse: ::SIZE_T,
    PeakUsage: ::SIZE_T,
}}
pub type PENUM_PAGE_FILE_INFORMATION = *mut ENUM_PAGE_FILE_INFORMATION;
pub type PENUM_PAGE_FILE_CALLBACKA = Option<unsafe extern "system" fn(
    pContext: ::LPVOID, pPageFileInfo: PENUM_PAGE_FILE_INFORMATION, lpFilename: ::LPCSTR,
) -> ::BOOL>;
pub type PENUM_PAGE_FILE_CALLBACKW = Option<unsafe extern "system" fn(
    pContext: ::LPVOID, pPageFileInfo: PENUM_PAGE_FILE_INFORMATION, lpFilename: ::LPCWSTR,
) -> ::BOOL>;
