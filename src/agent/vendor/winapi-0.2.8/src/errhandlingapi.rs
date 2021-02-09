// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! ApiSet Contract for api-ms-win-core-errorhandling-l1
pub type PTOP_LEVEL_EXCEPTION_FILTER = Option<unsafe extern "system" fn(
    ExceptionInfo: *mut ::EXCEPTION_POINTERS,
) -> ::LONG>;
pub type LPTOP_LEVEL_EXCEPTION_FILTER = PTOP_LEVEL_EXCEPTION_FILTER;
