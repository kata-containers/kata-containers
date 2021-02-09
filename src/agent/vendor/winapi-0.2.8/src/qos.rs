// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! QoS definitions for NDIS components.
pub type SERVICETYPE = ::ULONG;
STRUCT!{struct FLOWSPEC {
    TokenRate: ::ULONG,
    TokenBucketSize: ::ULONG,
    PeakBandwidth: ::ULONG,
    Latency: ::ULONG,
    DelayVariation: ::ULONG,
    ServiceType: SERVICETYPE,
    MaxSduSize: ::ULONG,
    MinimumPolicedSize: ::ULONG,
}}
pub type PFLOWSPEC = *mut FLOWSPEC;
pub type LPFLOWSPEC = *mut FLOWSPEC;
