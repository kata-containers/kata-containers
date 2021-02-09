// Copyright © 2015, skdltmxn
// Licensed under the MIT License <LICENSE.md>
//! Winspool header file
STRUCT!{struct PRINTER_DEFAULTSA {
    pDataType: ::LPSTR,
    pDevMode: ::LPDEVMODEA,
    DesiredAccess: ::ACCESS_MASK,
}}
pub type PPRINTER_DEFAULTSA = *mut PRINTER_DEFAULTSA;
pub type LPPRINTER_DEFAULTSA = *mut PRINTER_DEFAULTSA;
STRUCT!{struct PRINTER_DEFAULTSW {
    pDataType: ::LPWSTR,
    pDevMode: ::LPDEVMODEW,
    DesiredAccess: ::ACCESS_MASK,
}}
pub type PPRINTER_DEFAULTSW = *mut PRINTER_DEFAULTSW;
pub type LPPRINTER_DEFAULTSW = *mut PRINTER_DEFAULTSW;
STRUCT!{struct PRINTER_OPTIONSA {
    cbSize: ::UINT,
    dwFlags: ::DWORD,
}}
pub type PPRINTER_OPTIONSA = *mut PRINTER_OPTIONSA;
pub type LPPRINTER_OPTIONSA = *mut PRINTER_OPTIONSA;
STRUCT!{struct PRINTER_OPTIONSW {
    cbSize: ::UINT,
    dwFlags: ::DWORD,
}}
pub type PPRINTER_OPTIONSW = *mut PRINTER_OPTIONSW;
pub type LPPRINTER_OPTIONSW = *mut PRINTER_OPTIONSW;
