// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! ApiSet Contract for api-ms-win-core-heap-l1
STRUCT!{struct HEAP_SUMMARY {
    cb: ::DWORD,
    cbAllocated: ::SIZE_T,
    cbCommitted: ::SIZE_T,
    cbReserved: ::SIZE_T,
    cbMaxReserve: ::SIZE_T,
}}
pub type PHEAP_SUMMARY = *mut HEAP_SUMMARY;
pub type LPHEAP_SUMMARY = PHEAP_SUMMARY;
