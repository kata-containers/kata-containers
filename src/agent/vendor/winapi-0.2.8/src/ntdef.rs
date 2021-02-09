// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! Type definitions for the basic types.
//909
pub type NTSTATUS = ::LONG;
pub type PNTSTATUS = *mut NTSTATUS;
pub type PCNTSTATUS = *const NTSTATUS;
