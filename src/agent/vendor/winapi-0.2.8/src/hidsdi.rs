// Copyright © 2015, Peter Atashian and Alex Daniel Jones
// Licensed under the MIT License <LICENSE.md>
STRUCT!{struct HIDD_CONFIGURATION {
    cookie: ::PVOID,
    size: ::ULONG,
    RingBufferSize: ::ULONG,
}}
pub type PHIDD_CONFIGURATION = *mut HIDD_CONFIGURATION;
STRUCT!{struct HIDD_ATTRIBUTES {
    Size: ::ULONG,
    VendorID: ::USHORT,
    ProductID: ::USHORT,
    VersionNumber: ::USHORT,
}}
pub type PHIDD_ATTRIBUTES = *mut HIDD_ATTRIBUTES;
