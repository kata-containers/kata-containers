// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! ApiSet Contract for api-ms-win-core-sysinfo-l1.
STRUCT!{struct SYSTEM_INFO {
    wProcessorArchitecture: ::WORD,
    wReserved: ::WORD,
    dwPageSize: ::DWORD,
    lpMinimumApplicationAddress: ::LPVOID,
    lpMaximumApplicationAddress: ::LPVOID,
    dwActiveProcessorMask: ::DWORD_PTR,
    dwNumberOfProcessors: ::DWORD,
    dwProcessorType: ::DWORD,
    dwAllocationGranularity: ::DWORD,
    wProcessorLevel: ::WORD,
    wProcessorRevision: ::WORD,
}}
UNION!(SYSTEM_INFO, wProcessorArchitecture, dwOemId, dwOemId_mut, ::DWORD);
pub type LPSYSTEM_INFO = *mut SYSTEM_INFO;
STRUCT!{struct MEMORYSTATUSEX {
    dwLength: ::DWORD,
    dwMemoryLoad: ::DWORD,
    ullTotalPhys: ::DWORDLONG,
    ullAvailPhys: ::DWORDLONG,
    ullTotalPageFile: ::DWORDLONG,
    ullAvailPageFile: ::DWORDLONG,
    ullTotalVirtual: ::DWORDLONG,
    ullAvailVirtual: ::DWORDLONG,
    ullAvailExtendedVirtual: ::DWORDLONG,
}}
pub type LPMEMORYSTATUSEX = *mut MEMORYSTATUSEX;
ENUM!{enum COMPUTER_NAME_FORMAT {
    ComputerNameNetBIOS,
    ComputerNameDnsHostname,
    ComputerNameDnsDomain,
    ComputerNameDnsFullyQualified,
    ComputerNamePhysicalNetBIOS,
    ComputerNamePhysicalDnsHostname,
    ComputerNamePhysicalDnsDomain,
    ComputerNamePhysicalDnsFullyQualified,
    ComputerNameMax,
}}
pub type INIT_ONCE = ::RTL_RUN_ONCE;
pub type PINIT_ONCE = ::PRTL_RUN_ONCE;
pub type LPINIT_ONCE = ::PRTL_RUN_ONCE;
pub type CONDITION_VARIABLE = ::RTL_CONDITION_VARIABLE;
pub type PCONDITION_VARIABLE = *mut CONDITION_VARIABLE;
