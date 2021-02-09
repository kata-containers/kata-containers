// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! ApiSet Contract for api-ms-win-core-timezone-l1
pub const TIME_ZONE_ID_INVALID: ::DWORD = 0xFFFFFFFF;
STRUCT!{struct TIME_ZONE_INFORMATION {
    Bias: ::LONG,
    StandardName: [::WCHAR; 32],
    StandardDate: ::SYSTEMTIME,
    StandardBias: ::LONG,
    DaylightName: [::WCHAR; 32],
    DaylightDate: ::SYSTEMTIME,
    DaylightBias: ::LONG,
}}
pub type PTIME_ZONE_INFORMATION = *mut TIME_ZONE_INFORMATION;
pub type LPTIME_ZONE_INFORMATION = *mut TIME_ZONE_INFORMATION;
STRUCT!{nodebug struct DYNAMIC_TIME_ZONE_INFORMATION {
    Bias: ::LONG,
    StandardName: [::WCHAR; 32],
    StandardDate: ::SYSTEMTIME,
    StandardBias: ::LONG,
    DaylightName: [::WCHAR; 32],
    DaylightDate: ::SYSTEMTIME,
    DaylightBias: ::LONG,
    TimeZoneKeyName: [::WCHAR; 128],
    DynamicDaylightTimeDisabled: ::BOOLEAN,
}}
pub type PDYNAMIC_TIME_ZONE_INFORMATION = *mut DYNAMIC_TIME_ZONE_INFORMATION;
