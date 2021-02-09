// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! Macro APIs, window message crackers, and control APIs
//1233
pub fn GET_X_LPARAM(lp: ::LPARAM) -> ::c_int {
    ::LOWORD(lp as ::DWORD) as ::c_short as ::c_int
}
pub fn GET_Y_LPARAM(lp: ::LPARAM) -> ::c_int {
    ::HIWORD(lp as ::DWORD) as ::c_short as ::c_int
}
#[test]
fn test_get_x_lparam() {
    assert_eq!(GET_X_LPARAM(0xDEAD1234u32 as ::LPARAM), 0x1234);
    assert_eq!(GET_X_LPARAM(0xBEEFffffu32 as ::LPARAM), -1);
    assert_eq!(GET_X_LPARAM(0xCAFEFB2Eu32 as ::LPARAM), -1234);
}
#[test]
fn test_get_y_lparam() {
    assert_eq!(GET_Y_LPARAM(0x1234DEADu32 as ::LPARAM), 0x1234);
    assert_eq!(GET_Y_LPARAM(0xffffBEEFu32 as ::LPARAM), -1);
    assert_eq!(GET_Y_LPARAM(0xFB2ECAFEu32 as ::LPARAM), -1234);
}
