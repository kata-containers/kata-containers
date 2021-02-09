// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//114
pub type OLECHAR = ::WCHAR;
pub type LPOLESTR = *mut OLECHAR;
pub type LPCOLESTR = *const OLECHAR;
//147
pub type DOUBLE = ::c_double;
//281
pub type SCODE = ::LONG;
pub type PSCODE = *mut SCODE;
STRUCT!{struct BLOB {
    cbSize: ::ULONG,
    pBlobData: *mut ::BYTE,
}}
pub type LPBLOB = *mut BLOB;
STRUCT!{struct FLAGGED_WORD_BLOB {
    fFlags: ::ULONG,
    clSize: ::ULONG,
    asData: [::c_ushort; 1],
}}
STRUCT!{struct BYTE_SIZEDARR {
    clSize: ::ULONG,
    pData: *mut ::BYTE,
}}
STRUCT!{struct WORD_SIZEDARR {
    clSize: ::ULONG,
    pData: *mut ::c_ushort,
}}
STRUCT!{struct DWORD_SIZEDARR {
    clSize: ::ULONG,
    pData: *mut ::ULONG,
}}
STRUCT!{struct HYPER_SIZEDARR {
    clSize: ::ULONG,
    pData: *mut i64,
}}
