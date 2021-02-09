// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! ApiSet Contract for api-ms-win-core-threadpool-l1.
pub type PTP_WIN32_IO_CALLBACK = Option<unsafe extern "system" fn(
    Instance: ::PTP_CALLBACK_INSTANCE, Context: ::PVOID, Overlapped: ::PVOID, IoResult: ::ULONG,
    NumberOfBytesTransferred: ::ULONG_PTR, Io: ::PTP_IO,
)>;
