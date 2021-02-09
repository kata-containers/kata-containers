// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
STRUCT!{struct PROCESS_INFORMATION {
    hProcess: ::HANDLE,
    hThread: ::HANDLE,
    dwProcessId: ::DWORD,
    dwThreadId: ::DWORD,
}}
pub type PPROCESS_INFORMATION = *mut PROCESS_INFORMATION;
pub type LPPROCESS_INFORMATION = *mut PROCESS_INFORMATION;
STRUCT!{struct STARTUPINFOA {
    cb: ::DWORD,
    lpReserved: ::LPSTR,
    lpDesktop: ::LPSTR,
    lpTitle: ::LPSTR,
    dwX: ::DWORD,
    dwY: ::DWORD,
    dwXSize: ::DWORD,
    dwYSize: ::DWORD,
    dwXCountChars: ::DWORD,
    dwYCountChars: ::DWORD,
    dwFillAttribute: ::DWORD,
    dwFlags: ::DWORD,
    wShowWindow: ::WORD,
    cbReserved2: ::WORD,
    lpReserved2: ::LPBYTE,
    hStdInput: ::HANDLE,
    hStdOutput: ::HANDLE,
    hStdError: ::HANDLE,
}}
pub type LPSTARTUPINFOA = *mut STARTUPINFOA;
STRUCT!{struct STARTUPINFOW {
    cb: ::DWORD,
    lpReserved: ::LPWSTR,
    lpDesktop: ::LPWSTR,
    lpTitle: ::LPWSTR,
    dwX: ::DWORD,
    dwY: ::DWORD,
    dwXSize: ::DWORD,
    dwYSize: ::DWORD,
    dwXCountChars: ::DWORD,
    dwYCountChars: ::DWORD,
    dwFillAttribute: ::DWORD,
    dwFlags: ::DWORD,
    wShowWindow: ::WORD,
    cbReserved2: ::WORD,
    lpReserved2: ::LPBYTE,
    hStdInput: ::HANDLE,
    hStdOutput: ::HANDLE,
    hStdError: ::HANDLE,
}}
pub type LPSTARTUPINFOW = *mut STARTUPINFOW;
STRUCT!{struct PROC_THREAD_ATTRIBUTE_LIST {
    dummy: *mut ::c_void,
}}
pub type PPROC_THREAD_ATTRIBUTE_LIST = *mut PROC_THREAD_ATTRIBUTE_LIST;
pub type LPPROC_THREAD_ATTRIBUTE_LIST = *mut PROC_THREAD_ATTRIBUTE_LIST;
ENUM!{enum THREAD_INFORMATION_CLASS {
    ThreadMemoryPriority,
    ThreadAbsoluteCpuPriority,
    ThreadInformationClassMax,
}}
