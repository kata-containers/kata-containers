// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! ApiSet Contract for api-ms-win-core-synch-l1
pub type SRWLOCK = ::RTL_SRWLOCK;
pub type PSRWLOCK = *mut ::RTL_SRWLOCK;
pub type SYNCHRONIZATION_BARRIER = ::RTL_BARRIER;
pub type PSYNCHRONIZATION_BARRIER = ::PRTL_BARRIER;
pub type LPSYNCHRONIZATION_BARRIER = ::PRTL_BARRIER;
pub type PINIT_ONCE_FN = Option<unsafe extern "system" fn(
    InitOnce: ::PINIT_ONCE, Parameter: ::PVOID, Context: *mut ::PVOID,
) -> ::BOOL>;
pub type PTIMERAPCROUTINE = Option<unsafe extern "system" fn(
    lpArgToCompletionRoutine: ::LPVOID, dwTimerLowValue: ::DWORD, dwTimerHighValue: ::DWORD,
)>;
