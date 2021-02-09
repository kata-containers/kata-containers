// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! ApiSet Contract for api-ms-win-core-libraryloader-l1
pub type DLL_DIRECTORY_COOKIE = ::PVOID;
pub type PDLL_DIRECTORY_COOKIE = *mut ::PVOID;
pub type ENUMRESLANGPROCA = Option<unsafe extern "system" fn(
    hModule: ::HMODULE, lpType: ::LPCSTR, lpName: ::LPCSTR, wLanguage: ::WORD, lParam: ::LONG_PTR,
) -> ::BOOL>;
pub type ENUMRESLANGPROCW = Option<unsafe extern "system" fn(
    hModule: ::HMODULE, lpType: ::LPCWSTR, lpName: ::LPCWSTR, wLanguage: ::WORD, lParam: ::LONG_PTR,
) -> ::BOOL>;
pub type ENUMRESNAMEPROCA = Option<unsafe extern "system" fn(
    hModule: ::HMODULE, lpType: ::LPCSTR, lpName: ::LPSTR, lParam: ::LONG_PTR,
) -> ::BOOL>;
pub type ENUMRESNAMEPROCW = Option<unsafe extern "system" fn(
    hModule: ::HMODULE, lpType: ::LPCWSTR, lpName: ::LPWSTR, lParam: ::LONG_PTR,
) -> ::BOOL>;
pub type ENUMRESTYPEPROCA = Option<unsafe extern "system" fn(
    hModule: ::HMODULE, lpType: ::LPSTR, lParam: ::LONG_PTR,
) -> ::BOOL>;
pub type ENUMRESTYPEPROCW = Option<unsafe extern "system" fn(
    hModule: ::HMODULE, lpType: ::LPWSTR, lParam: ::LONG_PTR,
) -> ::BOOL>;
