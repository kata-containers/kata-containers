// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! ApiSet Contract for api-ms-win-core-memory-l1-1-0
pub const FILE_MAP_WRITE: ::DWORD = ::SECTION_MAP_WRITE;
pub const FILE_MAP_READ: ::DWORD = ::SECTION_MAP_READ;
pub const FILE_MAP_ALL_ACCESS: ::DWORD = ::SECTION_ALL_ACCESS;
pub const FILE_MAP_EXECUTE: ::DWORD = ::SECTION_MAP_EXECUTE_EXPLICIT;
pub const FILE_MAP_COPY: ::DWORD = 0x00000001;
pub const FILE_MAP_RESERVE: ::DWORD = 0x80000000;
ENUM!{enum MEMORY_RESOURCE_NOTIFICATION_TYPE {
    LowMemoryResourceNotification,
    HighMemoryResourceNotification,
}}
STRUCT!{struct WIN32_MEMORY_RANGE_ENTRY {
    VirtualAddress: ::PVOID,
    NumberOfBytes: ::SIZE_T,
}}
pub type PWIN32_MEMORY_RANGE_ENTRY = *mut WIN32_MEMORY_RANGE_ENTRY;
pub type PBAD_MEMORY_CALLBACK_ROUTINE = Option<unsafe extern "system" fn()>;
