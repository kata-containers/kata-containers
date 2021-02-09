// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
// STUB
DECLARE_HANDLE!(HDROP, HDROP__);

pub const NIM_ADD: ::DWORD = 0x00000000;
pub const NIM_MODIFY: ::DWORD = 0x00000001;
pub const NIM_DELETE: ::DWORD = 0x00000002;
pub const NIM_SETFOCUS: ::DWORD = 0x00000003;
pub const NIM_SETVERSION: ::DWORD = 0x00000004;
pub const NIF_MESSAGE: ::UINT = 0x00000001;
pub const NIF_ICON: ::UINT = 0x00000002;
pub const NIF_TIP: ::UINT = 0x00000004;
pub const NIF_STATE: ::UINT = 0x00000008;
pub const NIF_INFO: ::UINT = 0x00000010;
pub const NIF_GUID: ::UINT = 0x00000020;
pub const NIF_REALTIME: ::UINT = 0x00000040;
pub const NIF_SHOWTIP: ::UINT = 0x00000080;
pub const NOTIFYICON_VERSION: ::UINT = 3;
pub const NOTIFYICON_VERSION_4: ::UINT = 4;

STRUCT!{nodebug struct NOTIFYICONDATAA {
    cbSize: ::DWORD,
    hWnd: ::HWND,
    uID: ::UINT,
    uFlags: ::UINT,
    uCallbackMessage: ::UINT,
    hIcon: ::HICON,
    szTip: [::CHAR; 128],
    dwState: ::DWORD,
    dwStateMask: ::DWORD,
    szInfo: [::CHAR; 256],
    uTimeout: ::UINT,
    szInfoTitle: [::CHAR; 64],
    dwInfoFlags: ::DWORD,
    guidItem: ::GUID,
    hBalloonIcon: ::HICON,
}}
UNION!(NOTIFYICONDATAA, uTimeout, uTimeout, uTimeout_mut, ::UINT);
UNION!(NOTIFYICONDATAA, uTimeout, uVersion, uVersion_mut, ::UINT);
pub type PNOTIFYICONDATAA = *mut NOTIFYICONDATAA;

STRUCT!{nodebug struct NOTIFYICONDATAW {
    cbSize: ::DWORD,
    hWnd: ::HWND,
    uID: ::UINT,
    uFlags: ::UINT,
    uCallbackMessage: ::UINT,
    hIcon: ::HICON,
    szTip: [::WCHAR; 128],
    dwState: ::DWORD,
    dwStateMask: ::DWORD,
    szInfo: [::WCHAR; 256],
    uTimeout: ::UINT,
    szInfoTitle: [::WCHAR; 64],
    dwInfoFlags: ::DWORD,
    guidItem: ::GUID,
    hBalloonIcon: ::HICON,
}}
UNION!(NOTIFYICONDATAW, uTimeout, uTimeout, uTimeout_mut, ::UINT);
UNION!(NOTIFYICONDATAW, uTimeout, uVersion, uVersion_mut, ::UINT); // used with NIM_SETVERSION, values 0, 3 and 4
pub type PNOTIFYICONDATAW = *mut NOTIFYICONDATAW;
