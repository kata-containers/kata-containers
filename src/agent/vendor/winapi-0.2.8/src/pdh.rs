// Copyright © 2016, Klavs Madsen
// Licensed under the MIT License <LICENSE.md>
//! Common Performance Data Helper definitions
pub const PDH_FMT_RAW: ::DWORD = 0x00000010;
pub const PDH_FMT_ANSI: ::DWORD = 0x00000020;
pub const PDH_FMT_UNICODE: ::DWORD = 0x00000040;
pub const PDH_FMT_LONG: ::DWORD = 0x00000100;
pub const PDH_FMT_DOUBLE: ::DWORD = 0x00000200;
pub const PDH_FMT_LARGE: ::DWORD = 0x00000400;
pub const PDH_FMT_NOSCALE: ::DWORD = 0x00001000;
pub const PDH_FMT_1000: ::DWORD = 0x00002000;
pub const PDH_FMT_NODATA: ::DWORD = 0x00004000;
pub const PDH_FMT_NOCAP100: ::DWORD = 0x00008000;
pub const PERF_DETAIL_COSTLY: ::DWORD = 0x00010000;
pub const PERF_DETAIL_STANDARD: ::DWORD = 0x0000FFFF;

pub type PDH_STATUS = ::LONG;
pub type PDH_HQUERY = ::HANDLE;
pub type HQUERY = PDH_HQUERY;
pub type PDH_HCOUNTER = ::HANDLE;
pub type HCOUNTER = PDH_HCOUNTER;

STRUCT!{struct PDH_FMT_COUNTERVALUE {
    CStatus: ::DWORD,
    largeValue: ::LONGLONG,
}}
UNION!(PDH_FMT_COUNTERVALUE, largeValue, largeValue, largeValue_mut, ::LONGLONG);
UNION!(PDH_FMT_COUNTERVALUE, largeValue, longValue, longValue_mut, ::LONG);
UNION!(PDH_FMT_COUNTERVALUE, largeValue, doubleValue, doubleValue_mut, ::DOUBLE); 
UNION!(PDH_FMT_COUNTERVALUE, largeValue, AnsiStringValue, AnsiStringValue_mut, ::LPCSTR);
UNION!(PDH_FMT_COUNTERVALUE, largeValue, WideStringValue, WideStringValue_mut, ::LPCWSTR);
pub type PPDH_FMT_COUNTERVALUE = *mut PDH_FMT_COUNTERVALUE;

STRUCT!{struct PDH_COUNTER_PATH_ELEMENTS_A {
    szMachineName: ::LPSTR,
    szObjectName: ::LPSTR,
    szInstanceName: ::LPSTR,
    szParentInstance: ::LPSTR,
    dwInstanceIndex: ::DWORD,
    szCounterName: ::LPSTR,
}}
pub type PPDH_COUNTER_PATH_ELEMENTS_A = *mut PDH_COUNTER_PATH_ELEMENTS_A;

STRUCT!{struct PDH_COUNTER_PATH_ELEMENTS_W {
    szMachineName: ::LPWSTR,
    szObjectName: ::LPWSTR,
    szInstanceName: ::LPWSTR,
    szParentInstance: ::LPWSTR,
    dwInstanceIndex: ::DWORD,
    szCounterName: ::LPWSTR,
}}
pub type PPDH_COUNTER_PATH_ELEMENTS_W = *mut PDH_COUNTER_PATH_ELEMENTS_W;
