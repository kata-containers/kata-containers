// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! Basic Windows Type Definitions
DECLARE_HANDLE!(HWND, HWND__);
DECLARE_HANDLE!(HHOOK, HHOOK__);
DECLARE_HANDLE!(HEVENT, HEVENT__);
pub type HGDIOBJ = *mut ::c_void;
DECLARE_HANDLE!(HACCEL, HACCEL__);
DECLARE_HANDLE!(HBITMAP, HBITMAP__);
DECLARE_HANDLE!(HBRUSH, HBRUSH__);
DECLARE_HANDLE!(HCOLORSPACE, HCOLORSPACE__);
DECLARE_HANDLE!(HDC, HDC__);
DECLARE_HANDLE!(HGLRC, HGLRC__);
DECLARE_HANDLE!(HDESK, HDESK__);
DECLARE_HANDLE!(HENHMETAFILE, HENHMETAFILE__);
DECLARE_HANDLE!(HFONT, HFONT__);
DECLARE_HANDLE!(HICON, HICON__);
DECLARE_HANDLE!(HMENU, HMENU__);
DECLARE_HANDLE!(HPALETTE, HPALETTE__);
DECLARE_HANDLE!(HPEN, HPEN__);
DECLARE_HANDLE!(HWINEVENTHOOK, HWINEVENTHOOK__);
DECLARE_HANDLE!(HMONITOR, HMONITOR__);
DECLARE_HANDLE!(HUMPD, HUMPD__);
pub type HCURSOR = HICON;
pub type COLORREF = ::DWORD;
pub type LPCOLORREF = *mut ::DWORD;
STRUCT!{struct RECT {
    left: ::LONG,
    top: ::LONG,
    right: ::LONG,
    bottom: ::LONG,
}}
pub type PRECT = *mut RECT;
pub type NPRECT = *mut RECT;
pub type LPRECT = *mut RECT;
pub type LPCRECT = *const RECT;
STRUCT!{struct RECTL {
    left: ::LONG,
    top: ::LONG,
    right: ::LONG,
    bottom: ::LONG,
}}
pub type PRECTL = *mut RECTL;
pub type LPRECTL = *mut RECTL;
pub type LPCRECTL = *const RECTL;
STRUCT!{struct POINT {
    x: ::LONG,
    y: ::LONG,
}}
pub type PPOINT = *mut POINT;
pub type NPPOINT = *mut POINT;
pub type LPPOINT = *mut POINT;
STRUCT!{struct POINTL {
    x: ::LONG,
    y: ::LONG,
}}
pub type PPOINTL = *mut POINTL;
