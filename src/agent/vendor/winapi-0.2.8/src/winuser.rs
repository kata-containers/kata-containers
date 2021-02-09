// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! USER procedure declarations, constant definitions and macros

// Edit Control Styles
//
pub const ES_LEFT: ::DWORD = 0x0000;
pub const ES_CENTER: ::DWORD = 0x0001;
pub const ES_RIGHT: ::DWORD = 0x0002;
pub const ES_MULTILINE: ::DWORD = 0x0004;
pub const ES_UPPERCASE: ::DWORD = 0x0008;
pub const ES_LOWERCASE: ::DWORD = 0x0010;
pub const ES_PASSWORD: ::DWORD = 0x0020;
pub const ES_AUTOVSCROLL: ::DWORD = 0x0040;
pub const ES_AUTOHSCROLL: ::DWORD = 0x0080;
pub const ES_NOHIDESEL: ::DWORD = 0x0100;
pub const ES_OEMCONVERT: ::DWORD = 0x0400;
pub const ES_READONLY: ::DWORD = 0x0800;
pub const ES_WANTRETURN: ::DWORD = 0x1000;
pub const ES_NUMBER: ::DWORD = 0x2000;

// Edit Control Notification Codes
//
pub const EN_SETFOCUS: ::WORD = 0x0100;
pub const EN_KILLFOCUS: ::WORD = 0x0200;
pub const EN_CHANGE: ::WORD = 0x0300;
pub const EN_UPDATE: ::WORD = 0x0400;
pub const EN_ERRSPACE: ::WORD = 0x0500;
pub const EN_MAXTEXT: ::WORD = 0x0501;
pub const EN_HSCROLL: ::WORD = 0x0601;
pub const EN_VSCROLL: ::WORD = 0x0602;

pub const EN_ALIGN_LTR_EC: ::WORD = 0x0700;
pub const EN_ALIGN_RTL_EC: ::WORD = 0x0701;

// Edit control EM_SETMARGIN parameters
pub const EC_LEFTMARGIN: ::WORD = 0x0001;
pub const EC_RIGHTMARGIN: ::WORD = 0x0002;
pub const EC_USEFONTINFO: ::WORD = 0xffff;

// wParam of EM_GET/SETIMESTATUS
pub const EMSIS_COMPOSITIONSTRING: ::WORD = 0x0001;

// lParam for EMSIS_COMPOSITIONSTRING
pub const EIMES_GETCOMPSTRATONCE: ::WORD = 0x0001;
pub const EIMES_CANCELCOMPSTRINFOCUS: ::WORD = 0x0002;
pub const EIMES_COMPLETECOMPSTRKILLFOCUS: ::WORD = 0x0004;

// Edit Control Messages
//
pub const EM_GETSEL: ::WORD = 0x00B0;
pub const EM_SETSEL: ::WORD = 0x00B1;
pub const EM_GETRECT: ::WORD = 0x00B2;
pub const EM_SETRECT: ::WORD = 0x00B3;
pub const EM_SETRECTNP: ::WORD = 0x00B4;
pub const EM_SCROLL: ::WORD = 0x00B5;
pub const EM_LINESCROLL: ::WORD = 0x00B6;
pub const EM_SCROLLCARET: ::WORD = 0x00B7;
pub const EM_GETMODIFY: ::WORD = 0x00B8;
pub const EM_SETMODIFY: ::WORD = 0x00B9;
pub const EM_GETLINECOUNT: ::WORD = 0x00BA;
pub const EM_LINEINDEX: ::WORD = 0x00BB;
pub const EM_SETHANDLE: ::WORD = 0x00BC;
pub const EM_GETHANDLE: ::WORD = 0x00BD;
pub const EM_GETTHUMB: ::WORD = 0x00BE;
pub const EM_LINELENGTH: ::WORD = 0x00C1;
pub const EM_REPLACESEL: ::WORD = 0x00C2;
pub const EM_GETLINE: ::WORD = 0x00C4;
pub const EM_LIMITTEXT: ::WORD = 0x00C5;
pub const EM_CANUNDO: ::WORD = 0x00C6;
pub const EM_UNDO: ::WORD = 0x00C7;
pub const EM_FMTLINES: ::WORD = 0x00C8;
pub const EM_LINEFROMCHAR: ::WORD = 0x00C9;
pub const EM_SETTABSTOPS: ::WORD = 0x00CB;
pub const EM_SETPASSWORDCHAR: ::WORD = 0x00CC;
pub const EM_EMPTYUNDOBUFFER: ::WORD = 0x00CD;
pub const EM_GETFIRSTVISIBLELINE: ::WORD = 0x00CE;
pub const EM_SETREADONLY: ::WORD = 0x00CF;
pub const EM_SETWORDBREAKPROC: ::WORD = 0x00D0;
pub const EM_GETWORDBREAKPROC: ::WORD = 0x00D1;
pub const EM_GETPASSWORDCHAR: ::WORD = 0x00D2;

pub const EM_SETMARGINS: ::WORD = 0x00D3;
pub const EM_GETMARGINS: ::WORD = 0x00D4;
pub const EM_SETLIMITTEXT: ::WORD = EM_LIMITTEXT;
pub const EM_GETLIMITTEXT: ::WORD = 0x00D5;
pub const EM_POSFROMCHAR: ::WORD = 0x00D6;
pub const EM_CHARFROMPOS: ::WORD = 0x00D7;

pub const EM_SETIMESTATUS: ::WORD = 0x00D8;
pub const EM_GETIMESTATUS: ::WORD = 0x00D9;

// EDITWORDBREAKPROC code values
//
pub const WB_LEFT: ::WORD = 0;
pub const WB_RIGHT: ::WORD = 1;
pub const WB_ISDELIMITER: ::WORD = 2;

pub const BN_CLICKED: ::WORD = 0;
pub const BN_PAINT: ::WORD = 1;
pub const BN_HILITE: ::WORD = 2;
pub const BN_UNHILITE: ::WORD = 3;
pub const BN_DISABLE: ::WORD = 4;
pub const BN_DOUBLECLICKED: ::WORD = 5;
pub const BN_PUSHED: ::WORD = BN_HILITE;
pub const BN_UNPUSHED: ::WORD = BN_UNHILITE;
pub const BN_DBLCLK: ::WORD = BN_DOUBLECLICKED;
pub const BN_SETFOCUS: ::WORD = 6;
pub const BN_KILLFOCUS: ::WORD = 7;
pub const BS_PUSHBUTTON: ::DWORD = 0x00000000;
pub const BS_DEFPUSHBUTTON: ::DWORD = 0x00000001;
pub const BS_CHECKBOX: ::DWORD = 0x00000002;
pub const BS_AUTOCHECKBOX: ::DWORD = 0x00000003;
pub const BS_RADIOBUTTON: ::DWORD = 0x00000004;
pub const BS_3STATE: ::DWORD = 0x00000005;
pub const BS_AUTO3STATE: ::DWORD = 0x00000006;
pub const BS_GROUPBOX: ::DWORD = 0x00000007;
pub const BS_USERBUTTON: ::DWORD = 0x00000008;
pub const BS_AUTORADIOBUTTON: ::DWORD = 0x00000009;
pub const BS_PUSHBOX: ::DWORD = 0x0000000A;
pub const BS_OWNERDRAW: ::DWORD = 0x0000000B;
pub const BS_TYPEMASK: ::DWORD = 0x0000000F;
pub const BS_LEFTTEXT: ::DWORD = 0x00000020;
pub const BS_TEXT: ::DWORD = 0x00000000;
pub const BS_ICON: ::DWORD = 0x00000040;
pub const BS_BITMAP: ::DWORD = 0x00000080;
pub const BS_LEFT: ::DWORD = 0x00000100;
pub const BS_RIGHT: ::DWORD = 0x00000200;
pub const BS_CENTER: ::DWORD = 0x00000300;
pub const BS_TOP: ::DWORD = 0x00000400;
pub const BS_BOTTOM: ::DWORD = 0x00000800;
pub const BS_VCENTER: ::DWORD = 0x00000C00;
pub const BS_PUSHLIKE: ::DWORD = 0x00001000;
pub const BS_MULTILINE: ::DWORD = 0x00002000;
pub const BS_NOTIFY: ::DWORD = 0x00004000;
pub const BS_FLAT: ::DWORD = 0x00008000;
pub const BS_RIGHTBUTTON: ::DWORD = BS_LEFTTEXT;
pub const CCHILDREN_SCROLLBAR: usize = 5;
pub const CDS_UPDATEREGISTRY: ::DWORD = 0x00000001;
pub const CDS_TEST: ::DWORD = 0x00000002;
pub const CDS_FULLSCREEN: ::DWORD = 0x00000004;
pub const CDS_GLOBAL: ::DWORD = 0x00000008;
pub const CDS_SET_PRIMARY: ::DWORD = 0x00000010;
pub const CDS_VIDEOPARAMETERS: ::DWORD = 0x00000020;
pub const CDS_ENABLE_UNSAFE_MODES: ::DWORD = 0x00000100;
pub const CDS_DISABLE_UNSAFE_MODES: ::DWORD = 0x00000200;
pub const CDS_RESET: ::DWORD = 0x40000000;
pub const CDS_RESET_EX: ::DWORD = 0x20000000;
pub const CDS_NORESET: ::DWORD = 0x10000000;
pub const CF_TEXT: ::UINT = 1;
pub const CF_BITMAP: ::UINT = 2;
pub const CF_METAFILEPICT: ::UINT = 3;
pub const CF_SYLK: ::UINT = 4;
pub const CF_DIF: ::UINT = 5;
pub const CF_TIFF: ::UINT = 6;
pub const CF_OEMTEXT: ::UINT = 7;
pub const CF_DIB: ::UINT = 8;
pub const CF_PALETTE: ::UINT = 9;
pub const CF_PENDATA: ::UINT = 10;
pub const CF_RIFF: ::UINT = 11;
pub const CF_WAVE: ::UINT = 12;
pub const CF_UNICODETEXT: ::UINT = 13;
pub const CF_ENHMETAFILE: ::UINT = 14;
pub const CF_HDROP: ::UINT = 15;
pub const CF_LOCALE: ::UINT = 16;
pub const CF_DIBV5: ::UINT = 17;
pub const CF_OWNERDISPLAY: ::UINT = 0x0080;
pub const CF_DSPTEXT: ::UINT = 0x0081;
pub const CF_DSPBITMAP: ::UINT = 0x0082;
pub const CF_DSPENHMETAFILE: ::UINT = 0x008E;
pub const CF_DSPMETAFILEPICT: ::UINT = 0x0083;
pub const CF_PRIVATEFIRST: ::UINT = 0x0200;
pub const CF_PRIVATELAST: ::UINT = 0x02FF;
pub const CF_GDIOBJFIRST: ::UINT = 0x0300;
pub const CF_GDIOBJLAST: ::UINT = 0x03FF;
pub const CS_VREDRAW: ::DWORD = 0x0001;
pub const CS_HREDRAW: ::DWORD = 0x0002;
pub const CS_DBLCLKS: ::DWORD = 0x0008;
pub const CS_OWNDC: ::DWORD = 0x0020;
pub const CS_CLASSDC: ::DWORD = 0x0040;
pub const CS_PARENTDC: ::DWORD = 0x0080;
pub const CS_NOCLOSE: ::DWORD = 0x0200;
pub const CS_SAVEBITS: ::DWORD = 0x0800;
pub const CS_BYTEALIGNCLIENT: ::DWORD = 0x1000;
pub const CS_BYTEALIGNWINDOW: ::DWORD = 0x2000;
pub const CS_GLOBALCLASS: ::DWORD = 0x4000;
pub const CS_IME: ::DWORD = 0x00010000;
pub const CS_DROPSHADOW: ::DWORD = 0x00020000;
pub const DFC_CAPTION: ::UINT = 1;
pub const DFC_MENU: ::UINT = 2;
pub const DFC_SCROLL: ::UINT = 3;
pub const DFC_BUTTON: ::UINT = 4;
pub const DFCS_CAPTIONCLOSE: ::UINT = 0x0000;
pub const DFCS_CAPTIONMIN: ::UINT = 0x0001;
pub const DFCS_CAPTIONMAX: ::UINT = 0x0002;
pub const DFCS_CAPTIONRESTORE: ::UINT = 0x0003;
pub const DFCS_CAPTIONHELP: ::UINT = 0x0004;
pub const DFCS_MENUARROW: ::UINT = 0x0000;
pub const DFCS_MENUCHECK: ::UINT = 0x0001;
pub const DFCS_MENUBULLET: ::UINT = 0x0002;
pub const DFCS_MENUARROWRIGHT: ::UINT = 0x0004;
pub const DFCS_SCROLLUP: ::UINT = 0x0000;
pub const DFCS_SCROLLDOWN: ::UINT = 0x0001;
pub const DFCS_SCROLLLEFT: ::UINT = 0x0002;
pub const DFCS_SCROLLRIGHT: ::UINT = 0x0003;
pub const DFCS_SCROLLCOMBOBOX: ::UINT = 0x0005;
pub const DFCS_SCROLLSIZEGRIP: ::UINT = 0x0008;
pub const DFCS_SCROLLSIZEGRIPRIGHT: ::UINT = 0x0010;
pub const DFCS_BUTTONCHECK: ::UINT = 0x0000;
pub const DFCS_BUTTONRADIOIMAGE: ::UINT = 0x0001;
pub const DFCS_BUTTONRADIOMASK: ::UINT = 0x0002;
pub const DFCS_BUTTONRADIO: ::UINT = 0x0004;
pub const DFCS_BUTTON3STATE: ::UINT = 0x0008;
pub const DFCS_BUTTONPUSH: ::UINT = 0x0010;
pub const DFCS_INACTIVE: ::UINT = 0x0100;
pub const DFCS_PUSHED: ::UINT = 0x0200;
pub const DFCS_CHECKED: ::UINT = 0x0400;
// if WINVER >= 0x0500
pub const DFCS_TRANSPARENT: ::UINT = 0x0800;
pub const DFCS_HOT: ::UINT = 0x1000;
// end if WINVER >= 0x0500
pub const DFCS_ADJUSTRECT: ::UINT = 0x2000;
pub const DFCS_FLAT: ::UINT = 0x4000;
pub const DFCS_MONO: ::UINT = 0x8000;
pub const CW_USEDEFAULT: ::c_int = 0x80000000u32 as ::c_int;
pub const DISP_CHANGE_SUCCESSFUL: ::LONG = 0;
pub const DISP_CHANGE_RESTART: ::LONG = 1;
pub const DISP_CHANGE_FAILED: ::LONG = -1;
pub const DISP_CHANGE_BADMODE: ::LONG = -2;
pub const DISP_CHANGE_NOTUPDATED: ::LONG = -3;
pub const DISP_CHANGE_BADFLAGS: ::LONG = -4;
pub const DISP_CHANGE_BADPARAM: ::LONG = -5;
pub const DISP_CHANGE_BADDUALVIEW: ::LONG = -6;
pub const EDD_GET_DEVICE_INTERFACE_NAME: ::DWORD = 0x00000001;
pub const ENUM_CURRENT_SETTINGS: ::DWORD = 0xFFFFFFFF;
pub const ENUM_REGISTRY_SETTINGS: ::DWORD = 0xFFFFFFFE;
pub const GW_HWNDFIRST: ::UINT = 0;
pub const GW_HWNDLAST: ::UINT = 1;
pub const GW_HWNDNEXT: ::UINT = 2;
pub const GW_HWNDPREV: ::UINT = 3;
pub const GW_OWNER: ::UINT = 4;
pub const GW_CHILD: ::UINT = 5;
pub const GW_ENABLEDPOPUP: ::UINT = 6;
pub const GW_MAX: ::UINT = 6;
pub const HTERROR: ::c_int = -2;
pub const HTTRANSPARENT: ::c_int = -1;
pub const HTNOWHERE: ::c_int = 0;
pub const HTCLIENT: ::c_int = 1;
pub const HTCAPTION: ::c_int = 2;
pub const HTSYSMENU: ::c_int = 3;
pub const HTGROWBOX: ::c_int = 4;
pub const HTSIZE: ::c_int = HTGROWBOX;
pub const HTMENU: ::c_int = 5;
pub const HTHSCROLL: ::c_int = 6;
pub const HTVSCROLL: ::c_int = 7;
pub const HTMINBUTTON: ::c_int = 8;
pub const HTMAXBUTTON: ::c_int = 9;
pub const HTLEFT: ::c_int = 10;
pub const HTRIGHT: ::c_int = 11;
pub const HTTOP: ::c_int = 12;
pub const HTTOPLEFT: ::c_int = 13;
pub const HTTOPRIGHT: ::c_int = 14;
pub const HTBOTTOM: ::c_int = 15;
pub const HTBOTTOMLEFT: ::c_int = 16;
pub const HTBOTTOMRIGHT: ::c_int = 17;
pub const HTBORDER: ::c_int = 18;
pub const HTREDUCE: ::c_int = HTMINBUTTON;
pub const HTZOOM: ::c_int = HTMAXBUTTON;
pub const HTSIZEFIRST: ::c_int = HTLEFT;
pub const HTSIZELAST: ::c_int = HTBOTTOMRIGHT;
pub const HTOBJECT: ::c_int = 19;
pub const HTCLOSE: ::c_int = 20;
pub const HTHELP: ::c_int = 21;
pub const LSFW_LOCK: ::UINT = 1;
pub const LSFW_UNLOCK: ::UINT = 2;
pub const MDITILE_VERTICAL: ::UINT = 0x0000;
pub const MDITILE_HORIZONTAL: ::UINT = 0x0001;
pub const MDITILE_SKIPDISABLED: ::UINT = 0x0002;
pub const MDITILE_ZORDER: ::UINT = 0x0004;
pub const MB_OK: ::DWORD = 0x00000000;
pub const MB_OKCANCEL: ::DWORD = 0x00000001;
pub const MB_ABORTRETRYIGNORE: ::DWORD = 0x00000002;
pub const MB_YESNOCANCEL: ::DWORD = 0x00000003;
pub const MB_YESNO: ::DWORD = 0x00000004;
pub const MB_RETRYCANCEL: ::DWORD = 0x00000005;
pub const MB_CANCELTRYCONTINUE: ::DWORD = 0x00000006;
pub const MB_ICONHAND: ::DWORD = 0x00000010;
pub const MB_ICONQUESTION: ::DWORD = 0x00000020;
pub const MB_ICONEXCLAMATION: ::DWORD = 0x00000030;
pub const MB_ICONASTERISK: ::DWORD = 0x00000040;
pub const MB_USERICON: ::DWORD = 0x00000080;
pub const MB_ICONWARNING: ::DWORD = MB_ICONEXCLAMATION;
pub const MB_ICONERROR: ::DWORD = MB_ICONHAND;
pub const MB_ICONINFORMATION: ::DWORD = MB_ICONASTERISK;
pub const MB_ICONSTOP: ::DWORD = MB_ICONHAND;
pub const MB_DEFBUTTON1: ::DWORD = 0x00000000;
pub const MB_DEFBUTTON2: ::DWORD = 0x00000100;
pub const MB_DEFBUTTON3: ::DWORD = 0x00000200;
pub const MB_DEFBUTTON4: ::DWORD = 0x00000300;
pub const MB_APPLMODAL: ::DWORD = 0x00000000;
pub const MB_SYSTEMMODAL: ::DWORD = 0x00001000;
pub const MB_TASKMODAL: ::DWORD = 0x00002000;
pub const MB_HELP: ::DWORD = 0x00004000;
pub const MB_NOFOCUS: ::DWORD = 0x00008000;
pub const MB_SETFOREGROUND: ::DWORD = 0x00010000;
pub const MB_DEFAULT_DESKTOP_ONLY: ::DWORD = 0x00020000;
pub const MB_TOPMOST: ::DWORD = 0x00040000;
pub const MB_RIGHT: ::DWORD = 0x00080000;
pub const MB_RTLREADING: ::DWORD = 0x00100000;
pub const MB_SERVICE_NOTIFICATION: ::DWORD = 0x00200000;
pub const MB_SERVICE_NOTIFICATION_NT3X: ::DWORD = 0x00040000;
pub const MB_TYPEMASK: ::DWORD = 0x0000000F;
pub const MB_ICONMASK: ::DWORD = 0x000000F0;
pub const MB_DEFMASK: ::DWORD = 0x00000F00;
pub const MB_MODEMASK: ::DWORD = 0x00003000;
pub const MB_MISCMASK: ::DWORD = 0x0000C000;
pub const MF_BITMAP: ::UINT = 0x00000004;
pub const MF_CHECKED: ::UINT = 0x00000008;
pub const MF_DISABLED: ::UINT = 0x00000002;
pub const MF_ENABLED: ::UINT = 0x00000000;
pub const MF_GRAYED: ::UINT = 0x00000001;
pub const MF_MENUBARBREAK: ::UINT = 0x00000020;
pub const MF_MENUBREAK: ::UINT = 0x00000040;
pub const MF_OWNERDRAW: ::UINT = 0x00000100;
pub const MF_POPUP: ::UINT = 0x00000010;
pub const MF_SEPARATOR: ::UINT = 0x00000800;
pub const MF_STRING: ::UINT = 0x00000000;
pub const MF_UNCHECKED: ::UINT = 0x00000000;
pub const SB_HORZ: ::c_int = 0;
pub const SB_VERT: ::c_int = 1;
pub const SB_CTL: ::c_int = 2;
pub const SB_BOTH: ::c_int = 3;
pub const SW_HIDE: ::c_int = 0;
pub const SW_SHOWNORMAL: ::c_int = 1;
pub const SW_NORMAL: ::c_int = 1;
pub const SW_SHOWMINIMIZED: ::c_int = 2;
pub const SW_SHOWMAXIMIZED: ::c_int = 3;
pub const SW_MAXIMIZE: ::c_int = 3;
pub const SW_SHOWNOACTIVATE: ::c_int = 4;
pub const SW_SHOW: ::c_int = 5;
pub const SW_MINIMIZE: ::c_int = 6;
pub const SW_SHOWMINNOACTIVE: ::c_int = 7;
pub const SW_SHOWNA: ::c_int = 8;
pub const SW_RESTORE: ::c_int = 9;
pub const SW_SHOWDEFAULT: ::c_int = 10;
pub const SW_FORCEMINIMIZE: ::c_int = 11;
pub const SW_MAX: ::c_int = 11;
pub const SWP_NOSIZE: ::UINT = 0x0001;
pub const SWP_NOMOVE: ::UINT = 0x0002;
pub const SWP_NOZORDER: ::UINT = 0x0004;
pub const SWP_NOREDRAW: ::UINT = 0x0008;
pub const SWP_NOACTIVATE: ::UINT = 0x0010;
pub const SWP_FRAMECHANGED: ::UINT = 0x0020;
pub const SWP_SHOWWINDOW: ::UINT = 0x0040;
pub const SWP_HIDEWINDOW: ::UINT = 0x0080;
pub const SWP_NOCOPYBITS: ::UINT = 0x0100;
pub const SWP_NOOWNERZORDER: ::UINT = 0x0200;
pub const SWP_NOSENDCHANGING: ::UINT = 0x0400;
pub const SWP_DRAWFRAME: ::UINT = SWP_FRAMECHANGED;
pub const SWP_NOREPOSITION: ::UINT = SWP_NOOWNERZORDER;
pub const SWP_DEFERERASE: ::UINT = 0x2000;
pub const SWP_ASYNCWINDOWPOS: ::UINT = 0x4000;
pub const VK_LBUTTON: ::c_int = 0x01;
pub const VK_RBUTTON: ::c_int = 0x02;
pub const VK_CANCEL: ::c_int = 0x03;
pub const VK_MBUTTON: ::c_int = 0x04;
pub const VK_XBUTTON1: ::c_int = 0x05;
pub const VK_XBUTTON2: ::c_int = 0x06;
pub const VK_BACK: ::c_int = 0x08;
pub const VK_TAB: ::c_int = 0x09;
pub const VK_CLEAR: ::c_int = 0x0C;
pub const VK_RETURN: ::c_int = 0x0D;
pub const VK_SHIFT: ::c_int = 0x10;
pub const VK_CONTROL: ::c_int = 0x11;
pub const VK_MENU: ::c_int = 0x12;
pub const VK_PAUSE: ::c_int = 0x13;
pub const VK_CAPITAL: ::c_int = 0x14;
pub const VK_KANA: ::c_int = 0x15;
pub const VK_HANGUEL: ::c_int = 0x15;
pub const VK_HANGUL: ::c_int = 0x15;
pub const VK_JUNJA: ::c_int = 0x17;
pub const VK_FINAL: ::c_int = 0x18;
pub const VK_HANJA: ::c_int = 0x19;
pub const VK_KANJI: ::c_int = 0x19;
pub const VK_ESCAPE: ::c_int = 0x1B;
pub const VK_CONVERT: ::c_int = 0x1C;
pub const VK_NONCONVERT: ::c_int = 0x1D;
pub const VK_ACCEPT: ::c_int = 0x1E;
pub const VK_MODECHANGE: ::c_int = 0x1F;
pub const VK_SPACE: ::c_int = 0x20;
pub const VK_PRIOR: ::c_int = 0x21;
pub const VK_NEXT: ::c_int = 0x22;
pub const VK_END: ::c_int = 0x23;
pub const VK_HOME: ::c_int = 0x24;
pub const VK_LEFT: ::c_int = 0x25;
pub const VK_UP: ::c_int = 0x26;
pub const VK_RIGHT: ::c_int = 0x27;
pub const VK_DOWN: ::c_int = 0x28;
pub const VK_SELECT: ::c_int = 0x29;
pub const VK_PRINT: ::c_int = 0x2A;
pub const VK_EXECUTE: ::c_int = 0x2B;
pub const VK_SNAPSHOT: ::c_int = 0x2C;
pub const VK_INSERT: ::c_int = 0x2D;
pub const VK_DELETE: ::c_int = 0x2E;
pub const VK_HELP: ::c_int = 0x2F;
pub const VK_LWIN: ::c_int = 0x5B;
pub const VK_RWIN: ::c_int = 0x5C;
pub const VK_APPS: ::c_int = 0x5D;
pub const VK_SLEEP: ::c_int = 0x5F;
pub const VK_NUMPAD0: ::c_int = 0x60;
pub const VK_NUMPAD1: ::c_int = 0x61;
pub const VK_NUMPAD2: ::c_int = 0x62;
pub const VK_NUMPAD3: ::c_int = 0x63;
pub const VK_NUMPAD4: ::c_int = 0x64;
pub const VK_NUMPAD5: ::c_int = 0x65;
pub const VK_NUMPAD6: ::c_int = 0x66;
pub const VK_NUMPAD7: ::c_int = 0x67;
pub const VK_NUMPAD8: ::c_int = 0x68;
pub const VK_NUMPAD9: ::c_int = 0x69;
pub const VK_MULTIPLY: ::c_int = 0x6A;
pub const VK_ADD: ::c_int = 0x6B;
pub const VK_SEPARATOR: ::c_int = 0x6C;
pub const VK_SUBTRACT: ::c_int = 0x6D;
pub const VK_DECIMAL: ::c_int = 0x6E;
pub const VK_DIVIDE: ::c_int = 0x6F;
pub const VK_F1: ::c_int = 0x70;
pub const VK_F2: ::c_int = 0x71;
pub const VK_F3: ::c_int = 0x72;
pub const VK_F4: ::c_int = 0x73;
pub const VK_F5: ::c_int = 0x74;
pub const VK_F6: ::c_int = 0x75;
pub const VK_F7: ::c_int = 0x76;
pub const VK_F8: ::c_int = 0x77;
pub const VK_F9: ::c_int = 0x78;
pub const VK_F10: ::c_int = 0x79;
pub const VK_F11: ::c_int = 0x7A;
pub const VK_F12: ::c_int = 0x7B;
pub const VK_F13: ::c_int = 0x7C;
pub const VK_F14: ::c_int = 0x7D;
pub const VK_F15: ::c_int = 0x7E;
pub const VK_F16: ::c_int = 0x7F;
pub const VK_F17: ::c_int = 0x80;
pub const VK_F18: ::c_int = 0x81;
pub const VK_F19: ::c_int = 0x82;
pub const VK_F20: ::c_int = 0x83;
pub const VK_F21: ::c_int = 0x84;
pub const VK_F22: ::c_int = 0x85;
pub const VK_F23: ::c_int = 0x86;
pub const VK_F24: ::c_int = 0x87;
pub const VK_NUMLOCK: ::c_int = 0x90;
pub const VK_SCROLL: ::c_int = 0x91;
pub const VK_OEM_NEC_EQUAL: ::c_int = 0x92;
pub const VK_OEM_FJ_JISHO: ::c_int = 0x92;
pub const VK_OEM_FJ_MASSHOU: ::c_int = 0x93;
pub const VK_OEM_FJ_TOUROKU: ::c_int = 0x94;
pub const VK_OEM_FJ_LOYA: ::c_int = 0x95;
pub const VK_OEM_FJ_ROYA: ::c_int = 0x96;
pub const VK_LSHIFT: ::c_int = 0xA0;
pub const VK_RSHIFT: ::c_int = 0xA1;
pub const VK_LCONTROL: ::c_int = 0xA2;
pub const VK_RCONTROL: ::c_int = 0xA3;
pub const VK_LMENU: ::c_int = 0xA4;
pub const VK_RMENU: ::c_int = 0xA5;
pub const VK_BROWSER_BACK: ::c_int = 0xA6;
pub const VK_BROWSER_FORWARD: ::c_int = 0xA7;
pub const VK_BROWSER_REFRESH: ::c_int = 0xA8;
pub const VK_BROWSER_STOP: ::c_int = 0xA9;
pub const VK_BROWSER_SEARCH: ::c_int = 0xAA;
pub const VK_BROWSER_FAVORITES: ::c_int = 0xAB;
pub const VK_BROWSER_HOME: ::c_int = 0xAC;
pub const VK_VOLUME_MUTE: ::c_int = 0xAD;
pub const VK_VOLUME_DOWN: ::c_int = 0xAE;
pub const VK_VOLUME_UP: ::c_int = 0xAF;
pub const VK_MEDIA_NEXT_TRACK: ::c_int = 0xB0;
pub const VK_MEDIA_PREV_TRACK: ::c_int = 0xB1;
pub const VK_MEDIA_STOP: ::c_int = 0xB2;
pub const VK_MEDIA_PLAY_PAUSE: ::c_int = 0xB3;
pub const VK_LAUNCH_MAIL: ::c_int = 0xB4;
pub const VK_LAUNCH_MEDIA_SELECT: ::c_int = 0xB5;
pub const VK_LAUNCH_APP1: ::c_int = 0xB6;
pub const VK_LAUNCH_APP2: ::c_int = 0xB7;
pub const VK_OEM_1: ::c_int = 0xBA;
pub const VK_OEM_PLUS: ::c_int = 0xBB;
pub const VK_OEM_COMMA: ::c_int = 0xBC;
pub const VK_OEM_MINUS: ::c_int = 0xBD;
pub const VK_OEM_PERIOD: ::c_int = 0xBE;
pub const VK_OEM_2: ::c_int = 0xBF;
pub const VK_OEM_3: ::c_int = 0xC0;
pub const VK_OEM_4: ::c_int = 0xDB;
pub const VK_OEM_5: ::c_int = 0xDC;
pub const VK_OEM_6: ::c_int = 0xDD;
pub const VK_OEM_7: ::c_int = 0xDE;
pub const VK_OEM_8: ::c_int = 0xDF;
pub const VK_OEM_AX: ::c_int = 0xE1;
pub const VK_OEM_102: ::c_int = 0xE2;
pub const VK_ICO_HELP: ::c_int = 0xE3;
pub const VK_ICO_00: ::c_int = 0xE4;
pub const VK_PROCESSKEY: ::c_int = 0xE5;
pub const VK_ICO_CLEAR: ::c_int = 0xE6;
pub const VK_PACKET: ::c_int = 0xE7;
pub const VK_OEM_RESET: ::c_int = 0xE9;
pub const VK_OEM_JUMP: ::c_int = 0xEA;
pub const VK_OEM_PA1: ::c_int = 0xEB;
pub const VK_OEM_PA2: ::c_int = 0xEC;
pub const VK_OEM_PA3: ::c_int = 0xED;
pub const VK_OEM_WSCTRL: ::c_int = 0xEE;
pub const VK_OEM_CUSEL: ::c_int = 0xEF;
pub const VK_OEM_ATTN: ::c_int = 0xF0;
pub const VK_OEM_FINISH: ::c_int = 0xF1;
pub const VK_OEM_COPY: ::c_int = 0xF2;
pub const VK_OEM_AUTO: ::c_int = 0xF3;
pub const VK_OEM_ENLW: ::c_int = 0xF4;
pub const VK_OEM_BACKTAB: ::c_int = 0xF5;
pub const VK_ATTN: ::c_int = 0xF6;
pub const VK_CRSEL: ::c_int = 0xF7;
pub const VK_EXSEL: ::c_int = 0xF8;
pub const VK_EREOF: ::c_int = 0xF9;
pub const VK_PLAY: ::c_int = 0xFA;
pub const VK_ZOOM: ::c_int = 0xFB;
pub const VK_NONAME: ::c_int = 0xFC;
pub const VK_PA1: ::c_int = 0xFD;
pub const VK_OEM_CLEAR: ::c_int = 0xFE;
// if _WIN32_WINNT >= 0x0500
pub const APPCOMMAND_BROWSER_BACKWARD: ::c_short = 1;
pub const APPCOMMAND_BROWSER_FORWARD: ::c_short = 2;
pub const APPCOMMAND_BROWSER_REFRESH: ::c_short = 3;
pub const APPCOMMAND_BROWSER_STOP: ::c_short = 4;
pub const APPCOMMAND_BROWSER_SEARCH: ::c_short = 5;
pub const APPCOMMAND_BROWSER_FAVORITES: ::c_short = 6;
pub const APPCOMMAND_BROWSER_HOME: ::c_short = 7;
pub const APPCOMMAND_VOLUME_MUTE: ::c_short = 8;
pub const APPCOMMAND_VOLUME_DOWN: ::c_short = 9;
pub const APPCOMMAND_VOLUME_UP: ::c_short = 10;
pub const APPCOMMAND_MEDIA_NEXTTRACK: ::c_short = 11;
pub const APPCOMMAND_MEDIA_PREVIOUSTRACK: ::c_short = 12;
pub const APPCOMMAND_MEDIA_STOP: ::c_short = 13;
pub const APPCOMMAND_MEDIA_PLAY_PAUSE: ::c_short = 14;
pub const APPCOMMAND_LAUNCH_MAIL: ::c_short = 15;
pub const APPCOMMAND_LAUNCH_MEDIA_SELECT: ::c_short = 16;
pub const APPCOMMAND_LAUNCH_APP1: ::c_short = 17;
pub const APPCOMMAND_LAUNCH_APP2: ::c_short = 18;
pub const APPCOMMAND_BASS_DOWN: ::c_short = 19;
pub const APPCOMMAND_BASS_BOOST: ::c_short = 20;
pub const APPCOMMAND_BASS_UP: ::c_short = 21;
pub const APPCOMMAND_TREBLE_DOWN: ::c_short = 22;
pub const APPCOMMAND_TREBLE_UP: ::c_short = 23;
// if _WIN32_WINNT >= 0x0501
pub const APPCOMMAND_MICROPHONE_VOLUME_MUTE: ::c_short = 24;
pub const APPCOMMAND_MICROPHONE_VOLUME_DOWN: ::c_short = 25;
pub const APPCOMMAND_MICROPHONE_VOLUME_UP: ::c_short = 26;
pub const APPCOMMAND_HELP: ::c_short = 27;
pub const APPCOMMAND_FIND: ::c_short = 28;
pub const APPCOMMAND_NEW: ::c_short = 29;
pub const APPCOMMAND_OPEN: ::c_short = 30;
pub const APPCOMMAND_CLOSE: ::c_short = 31;
pub const APPCOMMAND_SAVE: ::c_short = 32;
pub const APPCOMMAND_PRINT: ::c_short = 33;
pub const APPCOMMAND_UNDO: ::c_short = 34;
pub const APPCOMMAND_REDO: ::c_short = 35;
pub const APPCOMMAND_COPY: ::c_short = 36;
pub const APPCOMMAND_CUT: ::c_short = 37;
pub const APPCOMMAND_PASTE: ::c_short = 38;
pub const APPCOMMAND_REPLY_TO_MAIL: ::c_short = 39;
pub const APPCOMMAND_FORWARD_MAIL: ::c_short = 40;
pub const APPCOMMAND_SEND_MAIL: ::c_short = 41;
pub const APPCOMMAND_SPELL_CHECK: ::c_short = 42;
pub const APPCOMMAND_DICTATE_OR_COMMAND_CONTROL_TOGGLE: ::c_short = 43;
pub const APPCOMMAND_MIC_ON_OFF_TOGGLE: ::c_short = 44;
pub const APPCOMMAND_CORRECTION_LIST: ::c_short = 45;
pub const APPCOMMAND_MEDIA_PLAY: ::c_short = 46;
pub const APPCOMMAND_MEDIA_PAUSE: ::c_short = 47;
pub const APPCOMMAND_MEDIA_RECORD: ::c_short = 48;
pub const APPCOMMAND_MEDIA_FAST_FORWARD: ::c_short = 49;
pub const APPCOMMAND_MEDIA_REWIND: ::c_short = 50;
pub const APPCOMMAND_MEDIA_CHANNEL_UP: ::c_short = 51;
pub const APPCOMMAND_MEDIA_CHANNEL_DOWN: ::c_short = 52;
// end if _WIN32_WINNT >= 0x0501
// if _WIN32_WINNT >= 0x0600
pub const APPCOMMAND_DELETE: ::c_short = 53;
pub const APPCOMMAND_DWM_FLIP3D: ::c_short = 54;
// end if _WIN32_WINNT >= 0x0600
pub const WM_NULL: ::UINT = 0x0000;
pub const WM_CREATE: ::UINT = 0x0001;
pub const WM_DESTROY: ::UINT = 0x0002;
pub const WM_MOVE: ::UINT = 0x0003;
pub const WM_SIZE: ::UINT = 0x0005;
pub const WM_ACTIVATE: ::UINT = 0x0006;
pub const WM_SETFOCUS: ::UINT = 0x0007;
pub const WM_KILLFOCUS: ::UINT = 0x0008;
pub const WM_ENABLE: ::UINT = 0x000A;
pub const WM_SETREDRAW: ::UINT = 0x000B;
pub const WM_SETTEXT: ::UINT = 0x000C;
pub const WM_GETTEXT: ::UINT = 0x000D;
pub const WM_GETTEXTLENGTH: ::UINT = 0x000E;
pub const WM_PAINT: ::UINT = 0x000F;
pub const WM_CLOSE: ::UINT = 0x0010;
pub const WM_QUERYENDSESSION: ::UINT = 0x0011;
pub const WM_QUERYOPEN: ::UINT = 0x0013;
pub const WM_ENDSESSION: ::UINT = 0x0016;
pub const WM_QUIT: ::UINT = 0x0012;
pub const WM_ERASEBKGND: ::UINT = 0x0014;
pub const WM_SYSCOLORCHANGE: ::UINT = 0x0015;
pub const WM_SHOWWINDOW: ::UINT = 0x0018;
pub const WM_WININICHANGE: ::UINT = 0x001A;
pub const WM_SETTINGCHANGE: ::UINT = WM_WININICHANGE;
pub const WM_DEVMODECHANGE: ::UINT = 0x001B;
pub const WM_ACTIVATEAPP: ::UINT = 0x001C;
pub const WM_FONTCHANGE: ::UINT = 0x001D;
pub const WM_TIMECHANGE: ::UINT = 0x001E;
pub const WM_CANCELMODE: ::UINT = 0x001F;
pub const WM_SETCURSOR: ::UINT = 0x0020;
pub const WM_MOUSEACTIVATE: ::UINT = 0x0021;
pub const WM_CHILDACTIVATE: ::UINT = 0x0022;
pub const WM_QUEUESYNC: ::UINT = 0x0023;
pub const WM_GETMINMAXINFO: ::UINT = 0x0024;
pub const WM_PAINTICON: ::UINT = 0x0026;
pub const WM_ICONERASEBKGND: ::UINT = 0x0027;
pub const WM_NEXTDLGCTL: ::UINT = 0x0028;
pub const WM_SPOOLERSTATUS: ::UINT = 0x002A;
pub const WM_DRAWITEM: ::UINT = 0x002B;
pub const WM_MEASUREITEM: ::UINT = 0x002C;
pub const WM_DELETEITEM: ::UINT = 0x002D;
pub const WM_VKEYTOITEM: ::UINT = 0x002E;
pub const WM_CHARTOITEM: ::UINT = 0x002F;
pub const WM_SETFONT: ::UINT = 0x0030;
pub const WM_GETFONT: ::UINT = 0x0031;
pub const WM_SETHOTKEY: ::UINT = 0x0032;
pub const WM_GETHOTKEY: ::UINT = 0x0033;
pub const WM_QUERYDRAGICON: ::UINT = 0x0037;
pub const WM_COMPAREITEM: ::UINT = 0x0039;
pub const WM_GETOBJECT: ::UINT = 0x003D;
pub const WM_COMPACTING: ::UINT = 0x0041;
pub const WM_COMMNOTIFY: ::UINT = 0x0044;
pub const WM_WINDOWPOSCHANGING: ::UINT = 0x0046;
pub const WM_WINDOWPOSCHANGED: ::UINT = 0x0047;
pub const WM_POWER: ::UINT = 0x0048;
pub const WM_COPYDATA: ::UINT = 0x004A;
pub const WM_CANCELJOURNAL: ::UINT = 0x004B;
pub const WM_NOTIFY: ::UINT = 0x004E;
pub const WM_INPUTLANGCHANGEREQUEST: ::UINT = 0x0050;
pub const WM_INPUTLANGCHANGE: ::UINT = 0x0051;
pub const WM_TCARD: ::UINT = 0x0052;
pub const WM_HELP: ::UINT = 0x0053;
pub const WM_USERCHANGED: ::UINT = 0x0054;
pub const WM_NOTIFYFORMAT: ::UINT = 0x0055;
pub const WM_CONTEXTMENU: ::UINT = 0x007B;
pub const WM_STYLECHANGING: ::UINT = 0x007C;
pub const WM_STYLECHANGED: ::UINT = 0x007D;
pub const WM_DISPLAYCHANGE: ::UINT = 0x007E;
pub const WM_GETICON: ::UINT = 0x007F;
pub const WM_SETICON: ::UINT = 0x0080;
pub const WM_NCCREATE: ::UINT = 0x0081;
pub const WM_NCDESTROY: ::UINT = 0x0082;
pub const WM_NCCALCSIZE: ::UINT = 0x0083;
pub const WM_NCHITTEST: ::UINT = 0x0084;
pub const WM_NCPAINT: ::UINT = 0x0085;
pub const WM_NCACTIVATE: ::UINT = 0x0086;
pub const WM_GETDLGCODE: ::UINT = 0x0087;
pub const WM_SYNCPAINT: ::UINT = 0x0088;
pub const WM_NCMOUSEMOVE: ::UINT = 0x00A0;
pub const WM_NCLBUTTONDOWN: ::UINT = 0x00A1;
pub const WM_NCLBUTTONUP: ::UINT = 0x00A2;
pub const WM_NCLBUTTONDBLCLK: ::UINT = 0x00A3;
pub const WM_NCRBUTTONDOWN: ::UINT = 0x00A4;
pub const WM_NCRBUTTONUP: ::UINT = 0x00A5;
pub const WM_NCRBUTTONDBLCLK: ::UINT = 0x00A6;
pub const WM_NCMBUTTONDOWN: ::UINT = 0x00A7;
pub const WM_NCMBUTTONUP: ::UINT = 0x00A8;
pub const WM_NCMBUTTONDBLCLK: ::UINT = 0x00A9;
pub const WM_NCXBUTTONDOWN: ::UINT = 0x00AB;
pub const WM_NCXBUTTONUP: ::UINT = 0x00AC;
pub const WM_NCXBUTTONDBLCLK: ::UINT = 0x00AD;
pub const WM_INPUT_DEVICE_CHANGE: ::UINT = 0x00FE;
pub const WM_INPUT: ::UINT = 0x00FF;
pub const WM_KEYFIRST: ::UINT = 0x0100;
pub const WM_KEYDOWN: ::UINT = 0x0100;
pub const WM_KEYUP: ::UINT = 0x0101;
pub const WM_CHAR: ::UINT = 0x0102;
pub const WM_DEADCHAR: ::UINT = 0x0103;
pub const WM_SYSKEYDOWN: ::UINT = 0x0104;
pub const WM_SYSKEYUP: ::UINT = 0x0105;
pub const WM_SYSCHAR: ::UINT = 0x0106;
pub const WM_SYSDEADCHAR: ::UINT = 0x0107;
pub const WM_UNICHAR: ::UINT = 0x0109;
pub const WM_KEYLAST: ::UINT = 0x0109;
pub const WM_IME_STARTCOMPOSITION: ::UINT = 0x010D;
pub const WM_IME_ENDCOMPOSITION: ::UINT = 0x010E;
pub const WM_IME_COMPOSITION: ::UINT = 0x010F;
pub const WM_IME_KEYLAST: ::UINT = 0x010F;
pub const WM_INITDIALOG: ::UINT = 0x0110;
pub const WM_COMMAND: ::UINT = 0x0111;
pub const WM_SYSCOMMAND: ::UINT = 0x0112;
pub const WM_TIMER: ::UINT = 0x0113;
pub const WM_HSCROLL: ::UINT = 0x0114;
pub const WM_VSCROLL: ::UINT = 0x0115;
pub const WM_INITMENU: ::UINT = 0x0116;
pub const WM_INITMENUPOPUP: ::UINT = 0x0117;
pub const WM_GESTURE: ::UINT = 0x0119;
pub const WM_GESTURENOTIFY: ::UINT = 0x011A;
pub const WM_MENUSELECT: ::UINT = 0x011F;
pub const WM_MENUCHAR: ::UINT = 0x0120;
pub const WM_ENTERIDLE: ::UINT = 0x0121;
pub const WM_MENURBUTTONUP: ::UINT = 0x0122;
pub const WM_MENUDRAG: ::UINT = 0x0123;
pub const WM_MENUGETOBJECT: ::UINT = 0x0124;
pub const WM_UNINITMENUPOPUP: ::UINT = 0x0125;
pub const WM_MENUCOMMAND: ::UINT = 0x0126;
pub const WM_CHANGEUISTATE: ::UINT = 0x0127;
pub const WM_UPDATEUISTATE: ::UINT = 0x0128;
pub const WM_QUERYUISTATE: ::UINT = 0x0129;
pub const WM_CTLCOLORMSGBOX: ::UINT = 0x0132;
pub const WM_CTLCOLOREDIT: ::UINT = 0x0133;
pub const WM_CTLCOLORLISTBOX: ::UINT = 0x0134;
pub const WM_CTLCOLORBTN: ::UINT = 0x0135;
pub const WM_CTLCOLORDLG: ::UINT = 0x0136;
pub const WM_CTLCOLORSCROLLBAR: ::UINT = 0x0137;
pub const WM_CTLCOLORSTATIC: ::UINT = 0x0138;
pub const WM_MOUSEFIRST: ::UINT = 0x0200;
pub const WM_MOUSEMOVE: ::UINT = 0x0200;
pub const WM_LBUTTONDOWN: ::UINT = 0x0201;
pub const WM_LBUTTONUP: ::UINT = 0x0202;
pub const WM_LBUTTONDBLCLK: ::UINT = 0x0203;
pub const WM_RBUTTONDOWN: ::UINT = 0x0204;
pub const WM_RBUTTONUP: ::UINT = 0x0205;
pub const WM_RBUTTONDBLCLK: ::UINT = 0x0206;
pub const WM_MBUTTONDOWN: ::UINT = 0x0207;
pub const WM_MBUTTONUP: ::UINT = 0x0208;
pub const WM_MBUTTONDBLCLK: ::UINT = 0x0209;
pub const WM_MOUSEWHEEL: ::UINT = 0x020A;
pub const WM_XBUTTONDOWN: ::UINT = 0x020B;
pub const WM_XBUTTONUP: ::UINT = 0x020C;
pub const WM_XBUTTONDBLCLK: ::UINT = 0x020D;
pub const WM_MOUSEHWHEEL: ::UINT = 0x020E;
pub const WM_MOUSELAST: ::UINT = 0x020E;
pub const WM_PARENTNOTIFY: ::UINT = 0x0210;
pub const WM_ENTERMENULOOP: ::UINT = 0x0211;
pub const WM_EXITMENULOOP: ::UINT = 0x0212;
pub const WM_NEXTMENU: ::UINT = 0x0213;
pub const WM_SIZING: ::UINT = 0x0214;
pub const WM_CAPTURECHANGED: ::UINT = 0x0215;
pub const WM_MOVING: ::UINT = 0x0216;
pub const WM_POWERBROADCAST: ::UINT = 0x0218;
pub const WM_DEVICECHANGE: ::UINT = 0x0219;
pub const WM_MDICREATE: ::UINT = 0x0220;
pub const WM_MDIDESTROY: ::UINT = 0x0221;
pub const WM_MDIACTIVATE: ::UINT = 0x0222;
pub const WM_MDIRESTORE: ::UINT = 0x0223;
pub const WM_MDINEXT: ::UINT = 0x0224;
pub const WM_MDIMAXIMIZE: ::UINT = 0x0225;
pub const WM_MDITILE: ::UINT = 0x0226;
pub const WM_MDICASCADE: ::UINT = 0x0227;
pub const WM_MDIICONARRANGE: ::UINT = 0x0228;
pub const WM_MDIGETACTIVE: ::UINT = 0x0229;
pub const WM_MDISETMENU: ::UINT = 0x0230;
pub const WM_ENTERSIZEMOVE: ::UINT = 0x0231;
pub const WM_EXITSIZEMOVE: ::UINT = 0x0232;
pub const WM_DROPFILES: ::UINT = 0x0233;
pub const WM_MDIREFRESHMENU: ::UINT = 0x0234;
pub const WM_POINTERDEVICECHANGE: ::UINT = 0x238;
pub const WM_POINTERDEVICEINRANGE: ::UINT = 0x239;
pub const WM_POINTERDEVICEOUTOFRANGE: ::UINT = 0x23A;
pub const WM_TOUCH: ::UINT = 0x0240;
pub const WM_NCPOINTERUPDATE: ::UINT = 0x0241;
pub const WM_NCPOINTERDOWN: ::UINT = 0x0242;
pub const WM_NCPOINTERUP: ::UINT = 0x0243;
pub const WM_POINTERUPDATE: ::UINT = 0x0245;
pub const WM_POINTERDOWN: ::UINT = 0x0246;
pub const WM_POINTERUP: ::UINT = 0x0247;
pub const WM_POINTERENTER: ::UINT = 0x0249;
pub const WM_POINTERLEAVE: ::UINT = 0x024A;
pub const WM_POINTERACTIVATE: ::UINT = 0x024B;
pub const WM_POINTERCAPTURECHANGED: ::UINT = 0x024C;
pub const WM_TOUCHHITTESTING: ::UINT = 0x024D;
pub const WM_POINTERWHEEL: ::UINT = 0x024E;
pub const WM_POINTERHWHEEL: ::UINT = 0x024F;
pub const WM_IME_SETCONTEXT: ::UINT = 0x0281;
pub const WM_IME_NOTIFY: ::UINT = 0x0282;
pub const WM_IME_CONTROL: ::UINT = 0x0283;
pub const WM_IME_COMPOSITIONFULL: ::UINT = 0x0284;
pub const WM_IME_SELECT: ::UINT = 0x0285;
pub const WM_IME_CHAR: ::UINT = 0x0286;
pub const WM_IME_REQUEST: ::UINT = 0x0288;
pub const WM_IME_KEYDOWN: ::UINT = 0x0290;
pub const WM_IME_KEYUP: ::UINT = 0x0291;
pub const WM_MOUSEHOVER: ::UINT = 0x02A1;
pub const WM_MOUSELEAVE: ::UINT = 0x02A3;
pub const WM_NCMOUSEHOVER: ::UINT = 0x02A0;
pub const WM_NCMOUSELEAVE: ::UINT = 0x02A2;
pub const WM_WTSSESSION_CHANGE: ::UINT = 0x02B1;
pub const WM_TABLET_FIRST: ::UINT = 0x02c0;
pub const WM_TABLET_LAST: ::UINT = 0x02df;
pub const WM_DPICHANGED: ::UINT = 0x02E0;
pub const WM_CUT: ::UINT = 0x0300;
pub const WM_COPY: ::UINT = 0x0301;
pub const WM_PASTE: ::UINT = 0x0302;
pub const WM_CLEAR: ::UINT = 0x0303;
pub const WM_UNDO: ::UINT = 0x0304;
pub const WM_RENDERFORMAT: ::UINT = 0x0305;
pub const WM_RENDERALLFORMATS: ::UINT = 0x0306;
pub const WM_DESTROYCLIPBOARD: ::UINT = 0x0307;
pub const WM_DRAWCLIPBOARD: ::UINT = 0x0308;
pub const WM_PAINTCLIPBOARD: ::UINT = 0x0309;
pub const WM_VSCROLLCLIPBOARD: ::UINT = 0x030A;
pub const WM_SIZECLIPBOARD: ::UINT = 0x030B;
pub const WM_ASKCBFORMATNAME: ::UINT = 0x030C;
pub const WM_CHANGECBCHAIN: ::UINT = 0x030D;
pub const WM_HSCROLLCLIPBOARD: ::UINT = 0x030E;
pub const WM_QUERYNEWPALETTE: ::UINT = 0x030F;
pub const WM_PALETTEISCHANGING: ::UINT = 0x0310;
pub const WM_PALETTECHANGED: ::UINT = 0x0311;
pub const WM_HOTKEY: ::UINT = 0x0312;
pub const WM_PRINT: ::UINT = 0x0317;
pub const WM_PRINTCLIENT: ::UINT = 0x0318;
pub const WM_APPCOMMAND: ::UINT = 0x0319;
pub const WM_THEMECHANGED: ::UINT = 0x031A;
pub const WM_CLIPBOARDUPDATE: ::UINT = 0x031D;
pub const WM_DWMCOMPOSITIONCHANGED: ::UINT = 0x031E;
pub const WM_DWMNCRENDERINGCHANGED: ::UINT = 0x031F;
pub const WM_DWMCOLORIZATIONCOLORCHANGED: ::UINT = 0x0320;
pub const WM_DWMWINDOWMAXIMIZEDCHANGE: ::UINT = 0x0321;
pub const WM_DWMSENDICONICTHUMBNAIL: ::UINT = 0x0323;
pub const WM_DWMSENDICONICLIVEPREVIEWBITMAP: ::UINT = 0x0326;
pub const WM_GETTITLEBARINFOEX: ::UINT = 0x033F;
pub const WM_HANDHELDFIRST: ::UINT = 0x0358;
pub const WM_HANDHELDLAST: ::UINT = 0x035F;
pub const WM_AFXFIRST: ::UINT = 0x0360;
pub const WM_AFXLAST: ::UINT = 0x037F;
pub const WM_PENWINFIRST: ::UINT = 0x0380;
pub const WM_PENWINLAST: ::UINT = 0x038F;
pub const WM_APP: ::UINT = 0x8000;
pub const WM_USER: ::UINT = 0x0400;
pub const WMSZ_LEFT: ::UINT = 1;
pub const WMSZ_RIGHT: ::UINT = 2;
pub const WMSZ_TOP: ::UINT = 3;
pub const WMSZ_TOPLEFT: ::UINT = 4;
pub const WMSZ_TOPRIGHT: ::UINT = 5;
pub const WMSZ_BOTTOM: ::UINT = 6;
pub const WMSZ_BOTTOMLEFT: ::UINT = 7;
pub const WMSZ_BOTTOMRIGHT: ::UINT = 8;
pub const SMTO_NORMAL: ::UINT = 0x0000;
pub const SMTO_BLOCK: ::UINT = 0x0001;
pub const SMTO_ABORTIFHUNG: ::UINT = 0x0002;
pub const SMTO_NOTIMEOUTIFNOTHUNG: ::UINT = 0x0008;
pub const SMTO_ERRORONEXIT: ::UINT = 0x0020;
pub const MA_ACTIVATE: ::UINT = 1;
pub const MA_ACTIVATEANDEAT: ::UINT = 2;
pub const MA_NOACTIVATE: ::UINT = 3;
pub const MA_NOACTIVATEANDEAT: ::UINT = 4;
pub const ICON_SMALL: ::UINT = 0;
pub const ICON_BIG: ::UINT = 1;
pub const ICON_SMALL2: ::UINT = 2;
pub const SIZE_RESTORED: ::UINT = 0;
pub const SIZE_MINIMIZED: ::UINT = 1;
pub const SIZE_MAXIMIZED: ::UINT = 2;
pub const SIZE_MAXSHOW: ::UINT = 3;
pub const SIZE_MAXHIDE: ::UINT = 4;
pub const SIZENORMAL: ::UINT = SIZE_RESTORED;
pub const SIZEICONIC: ::UINT = SIZE_MINIMIZED;
pub const SIZEFULLSCREEN: ::UINT = SIZE_MAXIMIZED;
pub const SIZEZOOMSHOW: ::UINT = SIZE_MAXSHOW;
pub const SIZEZOOMHIDE: ::UINT = SIZE_MAXHIDE;
STRUCT!{struct NCCALCSIZE_PARAMS {
    rgrc: [::RECT; 3],
    lppos: PWINDOWPOS,
}}
pub type PNCCALCSIZE_PARAMS = *mut NCCALCSIZE_PARAMS;
pub type NPNCCALCSIZE_PARAMS = *mut NCCALCSIZE_PARAMS;
pub type LPNCCALCSIZE_PARAMS = *mut NCCALCSIZE_PARAMS;
pub const WVR_ALIGNTOP: ::UINT = 0x0010;
pub const WVR_ALIGNLEFT: ::UINT = 0x0020;
pub const WVR_ALIGNBOTTOM: ::UINT = 0x0040;
pub const WVR_ALIGNRIGHT: ::UINT = 0x0080;
pub const WVR_HREDRAW: ::UINT = 0x0100;
pub const WVR_VREDRAW: ::UINT = 0x0200;
pub const WVR_REDRAW: ::UINT = WVR_HREDRAW | WVR_VREDRAW;
pub const WVR_VALIDRECTS: ::UINT = 0x0400;
pub const HOVER_DEFAULT: ::UINT = 0xFFFFFFFF;
pub const WS_OVERLAPPED: ::DWORD = 0x00000000;
pub const WS_POPUP: ::DWORD = 0x80000000;
pub const WS_CHILD: ::DWORD = 0x40000000;
pub const WS_MINIMIZE: ::DWORD = 0x20000000;
pub const WS_VISIBLE: ::DWORD = 0x10000000;
pub const WS_DISABLED: ::DWORD = 0x08000000;
pub const WS_CLIPSIBLINGS: ::DWORD = 0x04000000;
pub const WS_CLIPCHILDREN: ::DWORD = 0x02000000;
pub const WS_MAXIMIZE: ::DWORD = 0x01000000;
pub const WS_CAPTION: ::DWORD = 0x00C00000;
pub const WS_BORDER: ::DWORD = 0x00800000;
pub const WS_DLGFRAME: ::DWORD = 0x00400000;
pub const WS_VSCROLL: ::DWORD = 0x00200000;
pub const WS_HSCROLL: ::DWORD = 0x00100000;
pub const WS_SYSMENU: ::DWORD = 0x00080000;
pub const WS_THICKFRAME: ::DWORD = 0x00040000;
pub const WS_GROUP: ::DWORD = 0x00020000;
pub const WS_TABSTOP: ::DWORD = 0x00010000;
pub const WS_MINIMIZEBOX: ::DWORD = 0x00020000;
pub const WS_MAXIMIZEBOX: ::DWORD = 0x00010000;
pub const WS_TILED: ::DWORD = WS_OVERLAPPED;
pub const WS_ICONIC: ::DWORD = WS_MINIMIZE;
pub const WS_SIZEBOX: ::DWORD = WS_THICKFRAME;
pub const WS_TILEDWINDOW: ::DWORD = WS_OVERLAPPEDWINDOW;
pub const WS_OVERLAPPEDWINDOW: ::DWORD = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
pub const WS_POPUPWINDOW: ::DWORD = WS_POPUP | WS_BORDER | WS_SYSMENU;
pub const WS_CHILDWINDOW: ::DWORD = WS_CHILD;
pub const WS_EX_DLGMODALFRAME: ::DWORD = 0x00000001;
pub const WS_EX_NOPARENTNOTIFY: ::DWORD = 0x00000004;
pub const WS_EX_TOPMOST: ::DWORD = 0x00000008;
pub const WS_EX_ACCEPTFILES: ::DWORD = 0x00000010;
pub const WS_EX_TRANSPARENT: ::DWORD = 0x00000020;
pub const WS_EX_MDICHILD: ::DWORD = 0x00000040;
pub const WS_EX_TOOLWINDOW: ::DWORD = 0x00000080;
pub const WS_EX_WINDOWEDGE: ::DWORD = 0x00000100;
pub const WS_EX_CLIENTEDGE: ::DWORD = 0x00000200;
pub const WS_EX_CONTEXTHELP: ::DWORD = 0x00000400;
pub const WS_EX_RIGHT: ::DWORD = 0x00001000;
pub const WS_EX_LEFT: ::DWORD = 0x00000000;
pub const WS_EX_RTLREADING: ::DWORD = 0x00002000;
pub const WS_EX_LTRREADING: ::DWORD = 0x00000000;
pub const WS_EX_LEFTSCROLLBAR: ::DWORD = 0x00004000;
pub const WS_EX_RIGHTSCROLLBAR: ::DWORD = 0x00000000;
pub const WS_EX_CONTROLPARENT: ::DWORD = 0x00010000;
pub const WS_EX_STATICEDGE: ::DWORD = 0x00020000;
pub const WS_EX_APPWINDOW: ::DWORD = 0x00040000;
pub const WS_EX_OVERLAPPEDWINDOW: ::DWORD = WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE;
pub const WS_EX_PALETTEWINDOW: ::DWORD = WS_EX_WINDOWEDGE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST;
pub const WS_EX_LAYERED: ::DWORD = 0x00080000;
pub const WS_EX_NOINHERITLAYOUT: ::DWORD = 0x00100000;
pub const WS_EX_NOREDIRECTIONBITMAP: ::DWORD = 0x00200000;
pub const WS_EX_LAYOUTRTL: ::DWORD = 0x00400000;
pub const WS_EX_COMPOSITED: ::DWORD = 0x02000000;
pub const WS_EX_NOACTIVATE: ::DWORD = 0x08000000;
pub type NAMEENUMPROCA = Option<unsafe extern "system" fn(::LPSTR, ::LPARAM) -> ::BOOL>;
pub type NAMEENUMPROCW = Option<unsafe extern "system" fn(::LPWSTR, ::LPARAM) -> ::BOOL>;
pub type DESKTOPENUMPROCA = NAMEENUMPROCA;
pub type DESKTOPENUMPROCW = NAMEENUMPROCW;
pub type WINSTAENUMPROCA = NAMEENUMPROCA;
pub type WINSTAENUMPROCW = NAMEENUMPROCW;
pub type WNDENUMPROC = Option<unsafe extern "system" fn(::HWND, ::LPARAM) -> ::BOOL>;
pub type WNDPROC = Option<unsafe extern "system" fn(
    ::HWND, ::UINT, ::WPARAM, ::LPARAM,
) -> ::LRESULT>;
pub type DLGPROC = Option<unsafe extern "system" fn(
    ::HWND, ::UINT, ::WPARAM, ::LPARAM,
) -> ::INT_PTR>;
pub type HOOKPROC = Option<unsafe extern "system" fn(
    code: ::c_int, wParam: ::WPARAM, lParam: ::LPARAM,
) -> ::LRESULT>;
pub type TimerProc = Option<unsafe extern "system" fn(
    hwnd: ::HWND, uMsg: ::UINT, idEvent: ::UINT_PTR, dwTime: ::DWORD,
)>;
pub type DRAWSTATEPROC = Option<unsafe extern "system" fn(
    ::HDC, ::LPARAM, ::WPARAM, ::c_int, ::c_int,
) -> ::BOOL>;
pub type PROPENUMPROCA = Option<unsafe extern "system" fn(::HWND, ::LPCSTR, ::HANDLE) -> ::BOOL>;
pub type PROPENUMPROCW = Option<unsafe extern "system" fn(::HWND, ::LPCWSTR, ::HANDLE) -> ::BOOL>;
pub type GRAYSTRINGPROC = Option<unsafe extern "system" fn(::HDC, ::LPARAM, ::c_int) -> ::BOOL>;
pub type MSGBOXCALLBACK = Option<unsafe extern "system" fn(LPHELPINFO)>;
pub type WINEVENTPROC = Option<unsafe extern "system" fn(
    ::HWINEVENTHOOK, ::DWORD, ::HWND, ::LONG, ::LONG, ::DWORD, ::DWORD,
)>;
pub type HDEVNOTIFY = ::PVOID;
pub type MENUTEMPLATEA = ::VOID;
pub type MENUTEMPLATEW = ::VOID;
STRUCT!{struct MSG {
    hwnd: ::HWND,
    message: ::UINT,
    wParam: ::WPARAM,
    lParam: ::LPARAM,
    time: ::DWORD,
    pt: ::POINT,
}}
pub type PMSG = *mut MSG;
pub type NPMSG = *mut MSG;
pub type LPMSG = *mut MSG;
STRUCT!{struct PAINTSTRUCT {
    hdc: ::HDC,
    fErase: ::BOOL,
    rcPaint: ::RECT,
    fRestore: ::BOOL,
    fIncUpdate: ::BOOL,
    rgbReserved: [::BYTE; 32],
}}
pub type PPAINTSTRUCT = *mut PAINTSTRUCT;
pub type NPPAINTSTRUCT = *mut PAINTSTRUCT;
pub type LPPAINTSTRUCT = *mut PAINTSTRUCT;
STRUCT!{struct WINDOWPLACEMENT {
    length: ::UINT,
    flags: ::UINT,
    showCmd: ::UINT,
    ptMinPosition: ::POINT,
    ptMaxPosition: ::POINT,
    rcNormalPosition: ::RECT,
}}
pub type PWINDOWPLACEMENT = *mut WINDOWPLACEMENT;
pub type LPWINDOWPLACEMENT = *mut WINDOWPLACEMENT;
STRUCT!{nodebug struct WNDCLASSEXW {
    cbSize: ::UINT,
    style: ::UINT,
    lpfnWndProc: WNDPROC,
    cbClsExtra: ::c_int,
    cbWndExtra: ::c_int,
    hInstance: ::HINSTANCE,
    hIcon: ::HICON,
    hCursor: ::HCURSOR,
    hbrBackground: ::HBRUSH,
    lpszMenuName: ::LPCWSTR,
    lpszClassName: ::LPCWSTR,
    hIconSm: ::HICON,
}}
pub type PWNDCLASSEXW = *mut WNDCLASSEXW;
pub type NPWNDCLASSEXW = *mut WNDCLASSEXW;
pub type LPWNDCLASSEXW = *mut WNDCLASSEXW;
STRUCT!{nodebug struct WNDCLASSW {
    style: ::UINT,
    lpfnWndProc: WNDPROC,
    cbClsExtra: ::c_int,
    cbWndExtra: ::c_int,
    hInstance: ::HINSTANCE,
    hIcon: ::HICON,
    hCursor: ::HCURSOR,
    hbrBackground: ::HBRUSH,
    lpszMenuName: ::LPCWSTR,
    lpszClassName: ::LPCWSTR,
}}
pub type PWNDCLASSW = *mut WNDCLASSW;
pub type NPWNDCLASSW = *mut WNDCLASSW;
pub type LPWNDCLASSW = *mut WNDCLASSW;
STRUCT!{struct MINMAXINFO {
    ptReserved: ::POINT,
    ptMaxSize: ::POINT,
    ptMaxPosition: ::POINT,
    ptMinTrackSize: ::POINT,
    ptMaxTrackSize: ::POINT,
}}
STRUCT!{struct SCROLLBARINFO {
    cbSize: ::DWORD,
    rcScrollBar: ::RECT,
    dxyLineButton: ::c_int,
    xyThumbTop: ::c_int,
    xyThumbBottom: ::c_int,
    reserved: ::c_int,
    rgstate: [::DWORD; CCHILDREN_SCROLLBAR + 1],
}}
pub type PSCROLLBARINFO = *mut SCROLLBARINFO;
pub type LPSCROLLBARINFO = *mut SCROLLBARINFO;
STRUCT!{struct SCROLLINFO {
    cbSize: ::UINT,
    fMask: ::UINT,
    nMin: ::c_int,
    nMax: ::c_int,
    nPage: ::UINT,
    nPos: ::c_int,
    nTrackPos: ::c_int,
}}
pub type LPSCROLLINFO = *mut SCROLLINFO;
pub type LPCSCROLLINFO = *const SCROLLINFO;
STRUCT!{struct SIZE {
    cx: ::LONG,
    cy: ::LONG,
}}
pub type PSIZE = *mut SIZE;
pub type LPSIZE = *mut SIZE;
pub type SIZEL = SIZE;
pub type PSIZEL = *mut SIZEL;
pub type LPSIZEL = *mut SIZEL;
//1913
pub const UNICODE_NOCHAR: ::WPARAM = 0xffff;
pub type HDWP = *mut ::HANDLE;
//2193
pub const WHEEL_DELTA: ::DWORD = 120;
//2206
pub const XBUTTON1: ::DWORD = 0x0001;
pub const XBUTTON2: ::DWORD = 0x0002;
//2392
pub const MK_LBUTTON: ::WPARAM = 0x0001;
pub const MK_RBUTTON: ::WPARAM = 0x0002;
pub const MK_SHIFT: ::WPARAM = 0x0004;
pub const MK_CONTROL: ::WPARAM = 0x0008;
pub const MK_MBUTTON: ::WPARAM = 0x0010;
pub const MK_XBUTTON1: ::WPARAM = 0x0020;
pub const MK_XBUTTON2: ::WPARAM = 0x0040;
//2408
pub const TME_HOVER: ::DWORD = 0x0000_0001;
pub const TME_LEAVE: ::DWORD = 0x0000_0002;
pub const TME_NONCLIENT: ::DWORD = 0x0000_0010;
pub const TME_QUERY: ::DWORD = 0x4000_0000;
pub const TME_CANCEL: ::DWORD = 0x8000_0000;
pub const HWND_BROADCAST: ::HWND = 0xFFFF as ::HWND;
pub const HWND_MESSAGE: ::HWND = -3isize as ::HWND;
STRUCT!{struct TRACKMOUSEEVENT {
    cbSize: ::DWORD,
    dwFlags: ::DWORD,
    hwndTrack: ::HWND,
    dwHoverTime: ::DWORD,
}}
pub type LPTRACKMOUSEEVENT = *mut TRACKMOUSEEVENT;
//2575
STRUCT!{nodebug struct WINDOWPOS {
    hwnd: ::HWND,
    hwndInsertAfter: ::HWND,
    x: ::c_int,
    y: ::c_int,
    cx: ::c_int,
    cy: ::c_int,
    flags: ::UINT,
}}
pub type LPWINDOWPOS = *mut WINDOWPOS;
pub type PWINDOWPOS = *mut WINDOWPOS;
//3082
STRUCT!{struct CREATESTRUCTA {
    lpCreateParams: ::LPVOID,
    hInstance: ::HINSTANCE,
    hMenu: ::HMENU,
    hwndParent: ::HWND,
    cy: ::c_int,
    cx: ::c_int,
    y: ::c_int,
    x: ::c_int,
    style: ::LONG,
    lpszName: ::LPCSTR,
    lpszClass: ::LPCSTR,
    dwExStyle: ::DWORD,
}}
pub type LPCREATESTRUCTA = *mut CREATESTRUCTA;
STRUCT!{struct CREATESTRUCTW {
    lpCreateParams: ::LPVOID,
    hInstance: ::HINSTANCE,
    hMenu: ::HMENU,
    hwndParent: ::HWND,
    cy: ::c_int,
    cx: ::c_int,
    y: ::c_int,
    x: ::c_int,
    style: ::LONG,
    lpszName: ::LPCWSTR,
    lpszClass: ::LPCWSTR,
    dwExStyle: ::DWORD,
}}
pub type LPCREATESTRUCTW = *mut CREATESTRUCTW;
//3145
STRUCT!{struct NMHDR {
    hwndFrom: ::HWND,
    idFrom: ::UINT_PTR,
    code: ::UINT,  // NM_ code
}}
pub type LPNMHDR = *mut NMHDR;
//3400
pub const PM_NOREMOVE: ::UINT = 0x0000;
pub const PM_REMOVE: ::UINT = 0x0001;
pub const PM_NOYIELD: ::UINT = 0x0002;
pub const PM_QS_INPUT: ::UINT = QS_INPUT << 16;
pub const PM_QS_POSTMESSAGE: ::UINT = (QS_POSTMESSAGE | QS_HOTKEY | QS_TIMER) << 16;
pub const PM_QS_PAINT: ::UINT = QS_PAINT << 16;
pub const PM_QS_SENDMESSAGE: ::UINT = QS_SENDMESSAGE << 16;
//
pub const LWA_COLORKEY: ::DWORD = 0x00000001;
pub const LWA_ALPHA: ::DWORD = 0x00000002;
//3469
pub const EWX_LOGOFF: ::UINT = 0x00000000;
pub const EWX_SHUTDOWN: ::UINT = 0x00000001;
pub const EWX_REBOOT: ::UINT = 0x00000002;
pub const EWX_FORCE: ::UINT = 0x00000004;
pub const EWX_POWEROFF: ::UINT = 0x00000008;
pub const EWX_FORCEIFHUNG: ::UINT = 0x00000010;
pub const EWX_QUICKRESOLVE: ::UINT = 0x00000020;
pub const EWX_RESTARTAPPS: ::UINT = 0x00000040;
pub const EWX_HYBRID_SHUTDOWN: ::UINT = 0x00400000;
pub const EWX_BOOTOPTIONS: ::UINT = 0x01000000;
//4054 (Win 7 SDK)
STRUCT!{struct FLASHWINFO {
    cbSize: ::UINT,
    hwnd: ::HWND,
    dwFlags: ::DWORD,
    uCount: ::UINT,
    dwTimeout: ::DWORD,
}}
pub type PFLASHWINFO = *mut FLASHWINFO;
pub const FLASHW_STOP: ::DWORD = 0;
pub const FLASHW_CAPTION: ::DWORD = 0x00000001;
pub const FLASHW_TRAY: ::DWORD = 0x00000002;
pub const FLASHW_ALL: ::DWORD = FLASHW_CAPTION | FLASHW_TRAY;
pub const FLASHW_TIMER: ::DWORD = 0x00000004;
pub const FLASHW_TIMERNOFG: ::DWORD = 0x0000000C;
// 4674
pub const HWND_TOP: ::HWND = 0 as ::HWND;
pub const HWND_BOTTOM: ::HWND = 1 as ::HWND;
pub const HWND_TOPMOST: ::HWND = -1isize as ::HWND;
pub const HWND_NOTOPMOST: ::HWND = -2isize as ::HWND;
//5499
pub const MAPVK_VK_TO_VSC: ::UINT = 0;
pub const MAPVK_VSC_TO_VK: ::UINT = 1;
pub const MAPVK_VK_TO_CHAR: ::UINT = 2;
pub const MAPVK_VSC_TO_VK_EX: ::UINT = 3;
pub const MAPVK_VK_TO_VSC_EX: ::UINT = 4;
//5741
pub const KEYEVENTF_EXTENDEDKEY: ::DWORD = 0x0001;
pub const KEYEVENTF_KEYUP: ::DWORD = 0x0002;
pub const KEYEVENTF_UNICODE: ::DWORD = 0x0004;
pub const KEYEVENTF_SCANCODE: ::DWORD = 0x0008;
pub const MOUSEEVENTF_MOVE: ::DWORD = 0x0001;
pub const MOUSEEVENTF_LEFTDOWN: ::DWORD = 0x0002;
pub const MOUSEEVENTF_LEFTUP: ::DWORD = 0x0004;
pub const MOUSEEVENTF_RIGHTDOWN: ::DWORD = 0x0008;
pub const MOUSEEVENTF_RIGHTUP: ::DWORD = 0x0010;
pub const MOUSEEVENTF_MIDDLEDOWN: ::DWORD = 0x0020;
pub const MOUSEEVENTF_MIDDLEUP: ::DWORD = 0x0040;
pub const MOUSEEVENTF_XDOWN: ::DWORD = 0x0080;
pub const MOUSEEVENTF_XUP: ::DWORD = 0x0100;
pub const MOUSEEVENTF_WHEEL: ::DWORD = 0x0800;
pub const MOUSEEVENTF_HWHEEL: ::DWORD = 0x01000;
pub const MOUSEEVENTF_MOVE_NOCOALESCE: ::DWORD = 0x2000;
pub const MOUSEEVENTF_VIRTUALDESK: ::DWORD = 0x4000;
pub const MOUSEEVENTF_ABSOLUTE: ::DWORD = 0x8000;
STRUCT!{struct MOUSEINPUT {
    dx: ::LONG,
    dy: ::LONG,
    mouseData: ::DWORD,
    dwFlags: ::DWORD,
    time: ::DWORD,
    dwExtraInfo: ::ULONG_PTR,
}}
pub type PMOUSEINPUT = *mut MOUSEINPUT;
pub type LPMOUSEINPUT = *mut MOUSEINPUT;
STRUCT!{struct KEYBDINPUT {
    wVk: ::WORD,
    wScan: ::WORD,
    dwFlags: ::DWORD,
    time: ::DWORD,
    dwExtraInfo: ::ULONG_PTR,
}}
pub type PKEYBDINPUT = *mut KEYBDINPUT;
pub type LPKEYBDINPUT = *mut KEYBDINPUT;
STRUCT!{struct HARDWAREINPUT {
    uMsg: ::DWORD,
    wParamL: ::WORD,
    wParamH: ::WORD,
}}
pub type PHARDWAREINPUT = *mut HARDWAREINPUT;
pub type LPHARDWAREINPUT= *mut HARDWAREINPUT;
pub const INPUT_MOUSE: ::DWORD = 0;
pub const INPUT_KEYBOARD: ::DWORD = 1;
pub const INPUT_HARDWARE: ::DWORD = 2;
#[cfg(target_arch = "x86")]
STRUCT!{struct INPUT {
    type_: ::DWORD,
    u: [u32; 6],
}}
#[cfg(target_arch = "x86_64")]
STRUCT!{struct INPUT {
    type_: ::DWORD,
    u: [u64; 4],
}}
UNION!{INPUT, u, mi, mi_mut, MOUSEINPUT}
UNION!{INPUT, u, ki, ki_mut, KEYBDINPUT}
UNION!{INPUT, u, hi, hi_mut, HARDWAREINPUT}
pub type PINPUT = *mut INPUT;
pub type LPINPUT = *mut INPUT;
// if WINVER >= 0x0601
DECLARE_HANDLE!(HTOUCHINPUT, HTOUCHINPUT__);
STRUCT!{struct TOUCHINPUT {
    x: ::LONG,
    y: ::LONG,
    hSource: ::HANDLE,
    dwID: ::DWORD,
    dwFlags: ::DWORD,
    dwMask: ::DWORD,
    dwTime: ::DWORD,
    dwExtraInfo: ::ULONG_PTR,
    cxContact: ::DWORD,
    cyContact: ::DWORD,
}}
pub type PTOUCHINPUT = *mut TOUCHINPUT;
pub type PCTOUCHINPUT = *const TOUCHINPUT;
//Touch input flag values (TOUCHINPUT.dwFlags)
pub const TOUCHEVENTF_MOVE: ::DWORD = 0x0001;
pub const TOUCHEVENTF_DOWN: ::DWORD = 0x0002;
pub const TOUCHEVENTF_UP: ::DWORD = 0x0004;
pub const TOUCHEVENTF_INRANGE: ::DWORD = 0x0008;
pub const TOUCHEVENTF_PRIMARY: ::DWORD = 0x0010;
pub const TOUCHEVENTF_NOCOALESCE: ::DWORD = 0x0020;
pub const TOUCHEVENTF_PEN: ::DWORD = 0x0040;
pub const TOUCHEVENTF_PALM: ::DWORD = 0x0080;
//Touch input mask values (TOUCHINPUT.dwMask)
pub const TOUCHINPUTMASKF_TIMEFROMSYSTEM: ::DWORD = 0x0001;
pub const TOUCHINPUTMASKF_EXTRAINFO: ::DWORD = 0x0002;
pub const TOUCHINPUTMASKF_CONTACTAREA: ::DWORD = 0x0004;
//RegisterTouchWindow flag values
pub const TWF_FINETOUCH: ::ULONG = 0x00000001;
pub const TWF_WANTPALM: ::ULONG = 0x00000002;
// end if WINVER >= 0x0601
//Indices for GetWindowLong etc.
pub const GWL_EXSTYLE: ::c_int = -20;
pub const GWL_STYLE: ::c_int = -16;
pub const GWL_WNDPROC: ::c_int = -4;
pub const GWLP_WNDPROC: ::c_int = -4;
pub const GWL_HINSTANCE: ::c_int = -6;
pub const GWLP_HINSTANCE: ::c_int = -6;
pub const GWL_HWNDPARENT: ::c_int = -8;
pub const GWLP_HWNDPARENT: ::c_int = -8;
pub const GWL_ID: ::c_int = -12;
pub const GWLP_ID: ::c_int = -12;
pub const GWL_USERDATA: ::c_int = -21;
pub const GWLP_USERDATA: ::c_int = -21;
//5976
ENUM!{enum POINTER_INPUT_TYPE {
    PT_POINTER = 0x00000001,
    PT_TOUCH = 0x00000002,
    PT_PEN = 0x00000003,
    PT_MOUSE = 0x00000004,
    PT_TOUCHPAD = 0x00000005,
}}
//6566
// flags for MsgWaitForMultipleObjectsEx
pub const MWMO_WAITALL: ::DWORD = 0x0001;
pub const MWMO_ALERTABLE: ::DWORD = 0x0002;
pub const MWMO_INPUTAVAILABLE: ::DWORD = 0x0004;
//6573
pub const QS_KEY: ::UINT = 0x0001;
pub const QS_MOUSEMOVE: ::UINT = 0x0002;
pub const QS_MOUSEBUTTON: ::UINT = 0x0004;
pub const QS_POSTMESSAGE: ::UINT = 0x0008;
pub const QS_TIMER: ::UINT = 0x0010;
pub const QS_PAINT: ::UINT = 0x0020;
pub const QS_SENDMESSAGE: ::UINT = 0x0040;
pub const QS_HOTKEY: ::UINT = 0x0080;
pub const QS_ALLPOSTMESSAGE: ::UINT = 0x0100;
pub const QS_RAWINPUT: ::UINT = 0x0400;
pub const QS_TOUCH: ::UINT = 0x0800;
pub const QS_POINTER: ::UINT = 0x1000;
pub const QS_MOUSE: ::UINT = QS_MOUSEMOVE | QS_MOUSEBUTTON;
pub const QS_INPUT: ::UINT = QS_MOUSE | QS_KEY | QS_RAWINPUT | QS_TOUCH | QS_POINTER;
pub const QS_ALLEVENTS: ::UINT = QS_INPUT | QS_POSTMESSAGE | QS_TIMER | QS_PAINT | QS_HOTKEY;
pub const QS_ALLINPUT: ::UINT = QS_INPUT | QS_POSTMESSAGE | QS_TIMER
    | QS_PAINT | QS_HOTKEY | QS_SENDMESSAGE;
//6789
pub const SM_CXSCREEN: ::c_int = 0;
pub const SM_CYSCREEN: ::c_int = 1;
pub const SM_CXVSCROLL: ::c_int = 2;
pub const SM_CYHSCROLL: ::c_int = 3;
pub const SM_CYCAPTION: ::c_int = 4;
pub const SM_CXBORDER: ::c_int = 5;
pub const SM_CYBORDER: ::c_int = 6;
pub const SM_CXDLGFRAME: ::c_int = 7;
pub const SM_CYDLGFRAME: ::c_int = 8;
pub const SM_CYVTHUMB: ::c_int = 9;
pub const SM_CXHTHUMB: ::c_int = 10;
pub const SM_CXICON: ::c_int = 11;
pub const SM_CYICON: ::c_int = 12;
pub const SM_CXCURSOR: ::c_int = 13;
pub const SM_CYCURSOR: ::c_int = 14;
pub const SM_CYMENU: ::c_int = 15;
pub const SM_CXFULLSCREEN: ::c_int = 16;
pub const SM_CYFULLSCREEN: ::c_int = 17;
pub const SM_CYKANJIWINDOW: ::c_int = 18;
pub const SM_MOUSEPRESENT: ::c_int = 19;
pub const SM_CYVSCROLL: ::c_int = 20;
pub const SM_CXHSCROLL: ::c_int = 21;
pub const SM_DEBUG: ::c_int = 22;
pub const SM_SWAPBUTTON: ::c_int = 23;
pub const SM_RESERVED1: ::c_int = 24;
pub const SM_RESERVED2: ::c_int = 25;
pub const SM_RESERVED3: ::c_int = 26;
pub const SM_RESERVED4: ::c_int = 27;
pub const SM_CXMIN: ::c_int = 28;
pub const SM_CYMIN: ::c_int = 29;
pub const SM_CXSIZE: ::c_int = 30;
pub const SM_CYSIZE: ::c_int = 31;
pub const SM_CXFRAME: ::c_int = 32;
pub const SM_CYFRAME: ::c_int = 33;
pub const SM_CXMINTRACK: ::c_int = 34;
pub const SM_CYMINTRACK: ::c_int = 35;
pub const SM_CXDOUBLECLK: ::c_int = 36;
pub const SM_CYDOUBLECLK: ::c_int = 37;
pub const SM_CXICONSPACING: ::c_int = 38;
pub const SM_CYICONSPACING: ::c_int = 39;
pub const SM_MENUDROPALIGNMENT: ::c_int = 40;
pub const SM_PENWINDOWS: ::c_int = 41;
pub const SM_DBCSENABLED: ::c_int = 42;
pub const SM_CMOUSEBUTTONS: ::c_int = 43;
pub const SM_CXFIXEDFRAME: ::c_int = SM_CXDLGFRAME;
pub const SM_CYFIXEDFRAME: ::c_int = SM_CYDLGFRAME;
pub const SM_CXSIZEFRAME: ::c_int = SM_CXFRAME;
pub const SM_CYSIZEFRAME: ::c_int = SM_CYFRAME;
pub const SM_SECURE: ::c_int = 44;
pub const SM_CXEDGE: ::c_int = 45;
pub const SM_CYEDGE: ::c_int = 46;
pub const SM_CXMINSPACING: ::c_int = 47;
pub const SM_CYMINSPACING: ::c_int = 48;
pub const SM_CXSMICON: ::c_int = 49;
pub const SM_CYSMICON: ::c_int = 50;
pub const SM_CYSMCAPTION: ::c_int = 51;
pub const SM_CXSMSIZE: ::c_int = 52;
pub const SM_CYSMSIZE: ::c_int = 53;
pub const SM_CXMENUSIZE: ::c_int = 54;
pub const SM_CYMENUSIZE: ::c_int = 55;
pub const SM_ARRANGE: ::c_int = 56;
pub const SM_CXMINIMIZED: ::c_int = 57;
pub const SM_CYMINIMIZED: ::c_int = 58;
pub const SM_CXMAXTRACK: ::c_int = 59;
pub const SM_CYMAXTRACK: ::c_int = 60;
pub const SM_CXMAXIMIZED: ::c_int = 61;
pub const SM_CYMAXIMIZED: ::c_int = 62;
pub const SM_NETWORK: ::c_int = 63;
pub const SM_CLEANBOOT: ::c_int = 67;
pub const SM_CXDRAG: ::c_int = 68;
pub const SM_CYDRAG: ::c_int = 69;
pub const SM_SHOWSOUNDS: ::c_int = 70;
pub const SM_CXMENUCHECK: ::c_int = 71;
pub const SM_CYMENUCHECK: ::c_int = 72;
pub const SM_SLOWMACHINE: ::c_int = 73;
pub const SM_MIDEASTENABLED: ::c_int = 74;
pub const SM_MOUSEWHEELPRESENT: ::c_int = 75;
pub const SM_XVIRTUALSCREEN: ::c_int = 76;
pub const SM_YVIRTUALSCREEN: ::c_int = 77;
pub const SM_CXVIRTUALSCREEN: ::c_int = 78;
pub const SM_CYVIRTUALSCREEN: ::c_int = 79;
pub const SM_CMONITORS: ::c_int = 80;
pub const SM_SAMEDISPLAYFORMAT: ::c_int = 81;
pub const SM_IMMENABLED: ::c_int = 82;
pub const SM_CXFOCUSBORDER: ::c_int = 83;
pub const SM_CYFOCUSBORDER: ::c_int = 84;
pub const SM_TABLETPC: ::c_int = 86;
pub const SM_MEDIACENTER: ::c_int = 87;
pub const SM_STARTER: ::c_int = 88;
pub const SM_SERVERR2: ::c_int = 89;
pub const SM_MOUSEHORIZONTALWHEELPRESENT: ::c_int = 91;
pub const SM_CXPADDEDBORDER: ::c_int = 92;
pub const SM_DIGITIZER: ::c_int = 94;
pub const SM_MAXIMUMTOUCHES: ::c_int = 95;
pub const SM_CMETRICS: ::c_int = 97;
pub const SM_REMOTESESSION: ::c_int = 0x1000;
pub const SM_SHUTTINGDOWN: ::c_int = 0x2000;
pub const SM_REMOTECONTROL: ::c_int = 0x2001;
pub const SM_CARETBLINKINGENABLED: ::c_int = 0x2002;
pub const SM_CONVERTIBLESLATEMODE: ::c_int = 0x2003;
pub const SM_SYSTEMDOCKED: ::c_int = 0x2004;
//8855 (Win 7 SDK)
STRUCT!{struct ICONINFO {
    fIcon: ::BOOL,
    xHotspot: ::DWORD,
    yHotspot: ::DWORD,
    hbmMask: ::HBITMAP,
    hbmColor: ::HBITMAP,
}}
pub type PICONINFO = *mut ICONINFO;
//9066
// Color indexes for use in GetSysColor and SetSysColor
// 0-18 (after incrementing) are also valid in RegisterClass's WNDCLASS
pub const COLOR_SCROLLBAR: ::c_int = 0;
pub const COLOR_BACKGROUND: ::c_int = 1;
pub const COLOR_ACTIVECAPTION: ::c_int = 2;
pub const COLOR_INACTIVECAPTION: ::c_int = 3;
pub const COLOR_MENU: ::c_int = 4;
pub const COLOR_WINDOW: ::c_int = 5;
pub const COLOR_WINDOWFRAME: ::c_int = 6;
pub const COLOR_MENUTEXT: ::c_int = 7;
pub const COLOR_WINDOWTEXT: ::c_int = 8;
pub const COLOR_CAPTIONTEXT: ::c_int = 9;
pub const COLOR_ACTIVEBORDER: ::c_int = 10;
pub const COLOR_INACTIVEBORDER: ::c_int = 11;
pub const COLOR_APPWORKSPACE: ::c_int = 12;
pub const COLOR_HIGHLIGHT: ::c_int = 13;
pub const COLOR_HIGHLIGHTTEXT: ::c_int = 14;
pub const COLOR_BTNFACE: ::c_int = 15;
pub const COLOR_BTNSHADOW: ::c_int = 16;
pub const COLOR_GRAYTEXT: ::c_int = 17;
pub const COLOR_BTNTEXT: ::c_int = 18;
pub const COLOR_INACTIVECAPTIONTEXT: ::c_int = 19;
pub const COLOR_BTNHIGHLIGHT: ::c_int = 20;
// Introduced in Windows 95 (winver 0x0400):
pub const COLOR_3DDKSHADOW: ::c_int = 21;
pub const COLOR_3DLIGHT: ::c_int = 22;
pub const COLOR_INFOTEXT: ::c_int = 23;
pub const COLOR_INFOBK: ::c_int = 24;
pub const COLOR_DESKTOP: ::c_int = COLOR_BACKGROUND;
pub const COLOR_3DFACE: ::c_int = COLOR_BTNFACE;
pub const COLOR_3DSHADOW: ::c_int = COLOR_BTNSHADOW;
pub const COLOR_3DHIGHLIGHT: ::c_int = COLOR_BTNHIGHLIGHT;
pub const COLOR_3DHILIGHT: ::c_int = COLOR_BTNHIGHLIGHT;
pub const COLOR_BTNHILIGHT: ::c_int = COLOR_BTNHIGHLIGHT;
// Introduced in Windows 2000 (winver 0x0500)
pub const COLOR_HOTLIGHT: ::c_int = 26;
pub const COLOR_GRADIENTACTIVECAPTION: ::c_int = 27;
pub const COLOR_GRADIENTINACTIVECAPTION: ::c_int = 28;
// Introduced in Windows XP (winver 0x0501)
pub const COLOR_MENUHILIGHT: ::c_int = 29;
pub const COLOR_MENUBAR: ::c_int = 30;
//10069
pub const IDC_ARROW: ::LPCWSTR = 32512 as ::LPCWSTR;
pub const IDC_IBEAM: ::LPCWSTR = 32513 as ::LPCWSTR;
pub const IDC_WAIT: ::LPCWSTR = 32514 as ::LPCWSTR;
pub const IDC_CROSS: ::LPCWSTR = 32515 as ::LPCWSTR;
pub const IDC_UPARROW: ::LPCWSTR = 32516 as ::LPCWSTR;
pub const IDC_SIZE: ::LPCWSTR = 32640 as ::LPCWSTR;
pub const IDC_ICON: ::LPCWSTR = 32641 as ::LPCWSTR;
pub const IDC_SIZENWSE: ::LPCWSTR = 32642 as ::LPCWSTR;
pub const IDC_SIZENESW: ::LPCWSTR = 32643 as ::LPCWSTR;
pub const IDC_SIZEWE: ::LPCWSTR = 32644 as ::LPCWSTR;
pub const IDC_SIZENS: ::LPCWSTR = 32645 as ::LPCWSTR;
pub const IDC_SIZEALL: ::LPCWSTR = 32646 as ::LPCWSTR;
pub const IDC_NO: ::LPCWSTR = 32648 as ::LPCWSTR;
pub const IDC_HAND: ::LPCWSTR = 32649 as ::LPCWSTR;
pub const IDC_APPSTARTING: ::LPCWSTR = 32650 as ::LPCWSTR;
pub const IDC_HELP: ::LPCWSTR = 32651 as ::LPCWSTR;
//10492
pub const IDI_APPLICATION: ::LPCWSTR = 32512 as ::LPCWSTR;
pub const IDI_HAND: ::LPCWSTR = 32513 as ::LPCWSTR;
pub const IDI_QUESTION: ::LPCWSTR = 32514 as ::LPCWSTR;
pub const IDI_EXCLAMATION: ::LPCWSTR = 32515 as ::LPCWSTR;
pub const IDI_ASTERISK: ::LPCWSTR = 32516 as ::LPCWSTR;
pub const IDI_WINLOGO: ::LPCWSTR = 32517 as ::LPCWSTR;
pub const IDI_SHIELD: ::LPCWSTR = 32518 as ::LPCWSTR;
pub const IDI_WARNING: ::LPCWSTR = IDI_EXCLAMATION;
pub const IDI_ERROR: ::LPCWSTR = IDI_HAND;
pub const IDI_INFORMATION: ::LPCWSTR = IDI_ASTERISK;
pub const SPI_GETBEEP: ::UINT = 0x0001;
pub const SPI_SETBEEP: ::UINT = 0x0002;
pub const SPI_GETMOUSE: ::UINT = 0x0003;
pub const SPI_SETMOUSE: ::UINT = 0x0004;
pub const SPI_GETBORDER: ::UINT = 0x0005;
pub const SPI_SETBORDER: ::UINT = 0x0006;
pub const SPI_GETKEYBOARDSPEED: ::UINT = 0x000A;
pub const SPI_SETKEYBOARDSPEED: ::UINT = 0x000B;
pub const SPI_LANGDRIVER: ::UINT = 0x000C;
pub const SPI_ICONHORIZONTALSPACING: ::UINT = 0x000D;
pub const SPI_GETSCREENSAVETIMEOUT: ::UINT = 0x000E;
pub const SPI_SETSCREENSAVETIMEOUT: ::UINT = 0x000F;
pub const SPI_GETSCREENSAVEACTIVE: ::UINT = 0x0010;
pub const SPI_SETSCREENSAVEACTIVE: ::UINT = 0x0011;
pub const SPI_GETGRIDGRANULARITY: ::UINT = 0x0012;
pub const SPI_SETGRIDGRANULARITY: ::UINT = 0x0013;
pub const SPI_SETDESKWALLPAPER: ::UINT = 0x0014;
pub const SPI_SETDESKPATTERN: ::UINT = 0x0015;
pub const SPI_GETKEYBOARDDELAY: ::UINT = 0x0016;
pub const SPI_SETKEYBOARDDELAY: ::UINT = 0x0017;
pub const SPI_ICONVERTICALSPACING: ::UINT = 0x0018;
pub const SPI_GETICONTITLEWRAP: ::UINT = 0x0019;
pub const SPI_SETICONTITLEWRAP: ::UINT = 0x001A;
pub const SPI_GETMENUDROPALIGNMENT: ::UINT = 0x001B;
pub const SPI_SETMENUDROPALIGNMENT: ::UINT = 0x001C;
pub const SPI_SETDOUBLECLKWIDTH: ::UINT = 0x001D;
pub const SPI_SETDOUBLECLKHEIGHT: ::UINT = 0x001E;
pub const SPI_GETICONTITLELOGFONT: ::UINT = 0x001F;
pub const SPI_SETDOUBLECLICKTIME: ::UINT = 0x0020;
pub const SPI_SETMOUSEBUTTONSWAP: ::UINT = 0x0021;
pub const SPI_SETICONTITLELOGFONT: ::UINT = 0x0022;
pub const SPI_GETFASTTASKSWITCH: ::UINT = 0x0023;
pub const SPI_SETFASTTASKSWITCH: ::UINT = 0x0024;
pub const SPI_SETDRAGFULLWINDOWS: ::UINT = 0x0025;
pub const SPI_GETDRAGFULLWINDOWS: ::UINT = 0x0026;
pub const SPI_GETNONCLIENTMETRICS: ::UINT = 0x0029;
pub const SPI_SETNONCLIENTMETRICS: ::UINT = 0x002A;
pub const SPI_GETMINIMIZEDMETRICS: ::UINT = 0x002B;
pub const SPI_SETMINIMIZEDMETRICS: ::UINT = 0x002C;
pub const SPI_GETICONMETRICS: ::UINT = 0x002D;
pub const SPI_SETICONMETRICS: ::UINT = 0x002E;
pub const SPI_SETWORKAREA: ::UINT = 0x002F;
pub const SPI_GETWORKAREA: ::UINT = 0x0030;
pub const SPI_SETPENWINDOWS: ::UINT = 0x0031;
pub const SPI_GETHIGHCONTRAST: ::UINT = 0x0042;
pub const SPI_SETHIGHCONTRAST: ::UINT = 0x0043;
pub const SPI_GETKEYBOARDPREF: ::UINT = 0x0044;
pub const SPI_SETKEYBOARDPREF: ::UINT = 0x0045;
pub const SPI_GETSCREENREADER: ::UINT = 0x0046;
pub const SPI_SETSCREENREADER: ::UINT = 0x0047;
pub const SPI_GETANIMATION: ::UINT = 0x0048;
pub const SPI_SETANIMATION: ::UINT = 0x0049;
pub const SPI_GETFONTSMOOTHING: ::UINT = 0x004A;
pub const SPI_SETFONTSMOOTHING: ::UINT = 0x004B;
pub const SPI_SETDRAGWIDTH: ::UINT = 0x004C;
pub const SPI_SETDRAGHEIGHT: ::UINT = 0x004D;
pub const SPI_SETHANDHELD: ::UINT = 0x004E;
pub const SPI_GETLOWPOWERTIMEOUT: ::UINT = 0x004F;
pub const SPI_GETPOWEROFFTIMEOUT: ::UINT = 0x0050;
pub const SPI_SETLOWPOWERTIMEOUT: ::UINT = 0x0051;
pub const SPI_SETPOWEROFFTIMEOUT: ::UINT = 0x0052;
pub const SPI_GETLOWPOWERACTIVE: ::UINT = 0x0053;
pub const SPI_GETPOWEROFFACTIVE: ::UINT = 0x0054;
pub const SPI_SETLOWPOWERACTIVE: ::UINT = 0x0055;
pub const SPI_SETPOWEROFFACTIVE: ::UINT = 0x0056;
pub const SPI_SETCURSORS: ::UINT = 0x0057;
pub const SPI_SETICONS: ::UINT = 0x0058;
pub const SPI_GETDEFAULTINPUTLANG: ::UINT = 0x0059;
pub const SPI_SETDEFAULTINPUTLANG: ::UINT = 0x005A;
pub const SPI_SETLANGTOGGLE: ::UINT = 0x005B;
pub const SPI_GETWINDOWSEXTENSION: ::UINT = 0x005C;
pub const SPI_SETMOUSETRAILS: ::UINT = 0x005D;
pub const SPI_GETMOUSETRAILS: ::UINT = 0x005E;
pub const SPI_SETSCREENSAVERRUNNING: ::UINT = 0x0061;
pub const SPI_SCREENSAVERRUNNING: ::UINT = SPI_SETSCREENSAVERRUNNING;
pub const SPI_GETFILTERKEYS: ::UINT = 0x0032;
pub const SPI_SETFILTERKEYS: ::UINT = 0x0033;
pub const SPI_GETTOGGLEKEYS: ::UINT = 0x0034;
pub const SPI_SETTOGGLEKEYS: ::UINT = 0x0035;
pub const SPI_GETMOUSEKEYS: ::UINT = 0x0036;
pub const SPI_SETMOUSEKEYS: ::UINT = 0x0037;
pub const SPI_GETSHOWSOUNDS: ::UINT = 0x0038;
pub const SPI_SETSHOWSOUNDS: ::UINT = 0x0039;
pub const SPI_GETSTICKYKEYS: ::UINT = 0x003A;
pub const SPI_SETSTICKYKEYS: ::UINT = 0x003B;
pub const SPI_GETACCESSTIMEOUT: ::UINT = 0x003C;
pub const SPI_SETACCESSTIMEOUT: ::UINT = 0x003D;
pub const SPI_GETSERIALKEYS: ::UINT = 0x003E;
pub const SPI_SETSERIALKEYS: ::UINT = 0x003F;
pub const SPI_GETSOUNDSENTRY: ::UINT = 0x0040;
pub const SPI_SETSOUNDSENTRY: ::UINT = 0x0041;
pub const SPI_GETSNAPTODEFBUTTON: ::UINT = 0x005F;
pub const SPI_SETSNAPTODEFBUTTON: ::UINT = 0x0060;
pub const SPI_GETMOUSEHOVERWIDTH: ::UINT = 0x0062;
pub const SPI_SETMOUSEHOVERWIDTH: ::UINT = 0x0063;
pub const SPI_GETMOUSEHOVERHEIGHT: ::UINT = 0x0064;
pub const SPI_SETMOUSEHOVERHEIGHT: ::UINT = 0x0065;
pub const SPI_GETMOUSEHOVERTIME: ::UINT = 0x0066;
pub const SPI_SETMOUSEHOVERTIME: ::UINT = 0x0067;
pub const SPI_GETWHEELSCROLLLINES: ::UINT = 0x0068;
pub const SPI_SETWHEELSCROLLLINES: ::UINT = 0x0069;
pub const SPI_GETMENUSHOWDELAY: ::UINT = 0x006A;
pub const SPI_SETMENUSHOWDELAY: ::UINT = 0x006B;
pub const SPI_GETWHEELSCROLLCHARS: ::UINT = 0x006C;
pub const SPI_SETWHEELSCROLLCHARS: ::UINT = 0x006D;
pub const SPI_GETSHOWIMEUI: ::UINT = 0x006E;
pub const SPI_SETSHOWIMEUI: ::UINT = 0x006F;
pub const SPI_GETMOUSESPEED: ::UINT = 0x0070;
pub const SPI_SETMOUSESPEED: ::UINT = 0x0071;
pub const SPI_GETSCREENSAVERRUNNING: ::UINT = 0x0072;
pub const SPI_GETDESKWALLPAPER: ::UINT = 0x0073;
pub const SPI_GETAUDIODESCRIPTION: ::UINT = 0x0074;
pub const SPI_SETAUDIODESCRIPTION: ::UINT = 0x0075;
pub const SPI_GETSCREENSAVESECURE: ::UINT = 0x0076;
pub const SPI_SETSCREENSAVESECURE: ::UINT = 0x0077;
pub const SPI_GETHUNGAPPTIMEOUT: ::UINT = 0x0078;
pub const SPI_SETHUNGAPPTIMEOUT: ::UINT = 0x0079;
pub const SPI_GETWAITTOKILLTIMEOUT: ::UINT = 0x007A;
pub const SPI_SETWAITTOKILLTIMEOUT: ::UINT = 0x007B;
pub const SPI_GETWAITTOKILLSERVICETIMEOUT: ::UINT = 0x007C;
pub const SPI_SETWAITTOKILLSERVICETIMEOUT: ::UINT = 0x007D;
pub const SPI_GETMOUSEDOCKTHRESHOLD: ::UINT = 0x007E;
pub const SPI_SETMOUSEDOCKTHRESHOLD: ::UINT = 0x007F;
pub const SPI_GETPENDOCKTHRESHOLD: ::UINT = 0x0080;
pub const SPI_SETPENDOCKTHRESHOLD: ::UINT = 0x0081;
pub const SPI_GETWINARRANGING: ::UINT = 0x0082;
pub const SPI_SETWINARRANGING: ::UINT = 0x0083;
pub const SPI_GETMOUSEDRAGOUTTHRESHOLD: ::UINT = 0x0084;
pub const SPI_SETMOUSEDRAGOUTTHRESHOLD: ::UINT = 0x0085;
pub const SPI_GETPENDRAGOUTTHRESHOLD: ::UINT = 0x0086;
pub const SPI_SETPENDRAGOUTTHRESHOLD: ::UINT = 0x0087;
pub const SPI_GETMOUSESIDEMOVETHRESHOLD: ::UINT = 0x0088;
pub const SPI_SETMOUSESIDEMOVETHRESHOLD: ::UINT = 0x0089;
pub const SPI_GETPENSIDEMOVETHRESHOLD: ::UINT = 0x008A;
pub const SPI_SETPENSIDEMOVETHRESHOLD: ::UINT = 0x008B;
pub const SPI_GETDRAGFROMMAXIMIZE: ::UINT = 0x008C;
pub const SPI_SETDRAGFROMMAXIMIZE: ::UINT = 0x008D;
pub const SPI_GETSNAPSIZING: ::UINT = 0x008E;
pub const SPI_SETSNAPSIZING: ::UINT = 0x008F;
pub const SPI_GETDOCKMOVING: ::UINT = 0x0090;
pub const SPI_SETDOCKMOVING: ::UINT = 0x0091;
pub const SPI_GETACTIVEWINDOWTRACKING: ::UINT = 0x1000;
pub const SPI_SETACTIVEWINDOWTRACKING: ::UINT = 0x1001;
pub const SPI_GETMENUANIMATION: ::UINT = 0x1002;
pub const SPI_SETMENUANIMATION: ::UINT = 0x1003;
pub const SPI_GETCOMBOBOXANIMATION: ::UINT = 0x1004;
pub const SPI_SETCOMBOBOXANIMATION: ::UINT = 0x1005;
pub const SPI_GETLISTBOXSMOOTHSCROLLING: ::UINT = 0x1006;
pub const SPI_SETLISTBOXSMOOTHSCROLLING: ::UINT = 0x1007;
pub const SPI_GETGRADIENTCAPTIONS: ::UINT = 0x1008;
pub const SPI_SETGRADIENTCAPTIONS: ::UINT = 0x1009;
pub const SPI_GETKEYBOARDCUES: ::UINT = 0x100A;
pub const SPI_SETKEYBOARDCUES: ::UINT = 0x100B;
pub const SPI_GETMENUUNDERLINES: ::UINT = SPI_GETKEYBOARDCUES;
pub const SPI_SETMENUUNDERLINES: ::UINT = SPI_SETKEYBOARDCUES;
pub const SPI_GETACTIVEWNDTRKZORDER: ::UINT = 0x100C;
pub const SPI_SETACTIVEWNDTRKZORDER: ::UINT = 0x100D;
pub const SPI_GETHOTTRACKING: ::UINT = 0x100E;
pub const SPI_SETHOTTRACKING: ::UINT = 0x100F;
pub const SPI_GETMENUFADE: ::UINT = 0x1012;
pub const SPI_SETMENUFADE: ::UINT = 0x1013;
pub const SPI_GETSELECTIONFADE: ::UINT = 0x1014;
pub const SPI_SETSELECTIONFADE: ::UINT = 0x1015;
pub const SPI_GETTOOLTIPANIMATION: ::UINT = 0x1016;
pub const SPI_SETTOOLTIPANIMATION: ::UINT = 0x1017;
pub const SPI_GETTOOLTIPFADE: ::UINT = 0x1018;
pub const SPI_SETTOOLTIPFADE: ::UINT = 0x1019;
pub const SPI_GETCURSORSHADOW: ::UINT = 0x101A;
pub const SPI_SETCURSORSHADOW: ::UINT = 0x101B;
pub const SPI_GETMOUSESONAR: ::UINT = 0x101C;
pub const SPI_SETMOUSESONAR: ::UINT = 0x101D;
pub const SPI_GETMOUSECLICKLOCK: ::UINT = 0x101E;
pub const SPI_SETMOUSECLICKLOCK: ::UINT = 0x101F;
pub const SPI_GETMOUSEVANISH: ::UINT = 0x1020;
pub const SPI_SETMOUSEVANISH: ::UINT = 0x1021;
pub const SPI_GETFLATMENU: ::UINT = 0x1022;
pub const SPI_SETFLATMENU: ::UINT = 0x1023;
pub const SPI_GETDROPSHADOW: ::UINT = 0x1024;
pub const SPI_SETDROPSHADOW: ::UINT = 0x1025;
pub const SPI_GETBLOCKSENDINPUTRESETS: ::UINT = 0x1026;
pub const SPI_SETBLOCKSENDINPUTRESETS: ::UINT = 0x1027;
pub const SPI_GETUIEFFECTS: ::UINT = 0x103E;
pub const SPI_SETUIEFFECTS: ::UINT = 0x103F;
pub const SPI_GETDISABLEOVERLAPPEDCONTENT: ::UINT = 0x1040;
pub const SPI_SETDISABLEOVERLAPPEDCONTENT: ::UINT = 0x1041;
pub const SPI_GETCLIENTAREAANIMATION: ::UINT = 0x1042;
pub const SPI_SETCLIENTAREAANIMATION: ::UINT = 0x1043;
pub const SPI_GETCLEARTYPE: ::UINT = 0x1048;
pub const SPI_SETCLEARTYPE: ::UINT = 0x1049;
pub const SPI_GETSPEECHRECOGNITION: ::UINT = 0x104A;
pub const SPI_SETSPEECHRECOGNITION: ::UINT = 0x104B;
pub const SPI_GETFOREGROUNDLOCKTIMEOUT: ::UINT = 0x2000;
pub const SPI_SETFOREGROUNDLOCKTIMEOUT: ::UINT = 0x2001;
pub const SPI_GETACTIVEWNDTRKTIMEOUT: ::UINT = 0x2002;
pub const SPI_SETACTIVEWNDTRKTIMEOUT: ::UINT = 0x2003;
pub const SPI_GETFOREGROUNDFLASHCOUNT: ::UINT = 0x2004;
pub const SPI_SETFOREGROUNDFLASHCOUNT: ::UINT = 0x2005;
pub const SPI_GETCARETWIDTH: ::UINT = 0x2006;
pub const SPI_SETCARETWIDTH: ::UINT = 0x2007;
pub const SPI_GETMOUSECLICKLOCKTIME: ::UINT = 0x2008;
pub const SPI_SETMOUSECLICKLOCKTIME: ::UINT = 0x2009;
pub const SPI_GETFONTSMOOTHINGTYPE: ::UINT = 0x200A;
pub const SPI_SETFONTSMOOTHINGTYPE: ::UINT = 0x200B;
pub const FE_FONTSMOOTHINGSTANDARD: ::UINT = 0x0001;
pub const FE_FONTSMOOTHINGCLEARTYPE: ::UINT = 0x0002;
pub const SPI_GETFONTSMOOTHINGCONTRAST: ::UINT = 0x200C;
pub const SPI_SETFONTSMOOTHINGCONTRAST: ::UINT = 0x200D;
pub const SPI_GETFOCUSBORDERWIDTH: ::UINT = 0x200E;
pub const SPI_SETFOCUSBORDERWIDTH: ::UINT = 0x200F;
pub const SPI_GETFOCUSBORDERHEIGHT: ::UINT = 0x2010;
pub const SPI_SETFOCUSBORDERHEIGHT: ::UINT = 0x2011;
pub const SPI_GETFONTSMOOTHINGORIENTATION: ::UINT = 0x2012;
pub const SPI_SETFONTSMOOTHINGORIENTATION: ::UINT = 0x2013;
pub const FE_FONTSMOOTHINGORIENTATIONBGR: ::UINT = 0x0000;
pub const FE_FONTSMOOTHINGORIENTATIONRGB: ::UINT = 0x0001;
pub const SPI_GETMINIMUMHITRADIUS: ::UINT = 0x2014;
pub const SPI_SETMINIMUMHITRADIUS: ::UINT = 0x2015;
pub const SPI_GETMESSAGEDURATION: ::UINT = 0x2016;
pub const SPI_SETMESSAGEDURATION: ::UINT = 0x2017;
//11264
pub const CB_GETEDITSEL: ::UINT = 0x0140;
pub const CB_LIMITTEXT: ::UINT = 0x0141;
pub const CB_SETEDITSEL: ::UINT = 0x0142;
pub const CB_ADDSTRING: ::UINT = 0x0143;
pub const CB_DELETESTRING: ::UINT = 0x0144;
pub const CB_DIR: ::UINT = 0x0145;
pub const CB_GETCOUNT: ::UINT = 0x0146;
pub const CB_GETCURSEL: ::UINT = 0x0147;
pub const CB_GETLBTEXT: ::UINT = 0x0148;
pub const CB_GETLBTEXTLEN: ::UINT = 0x0149;
pub const CB_INSERTSTRING: ::UINT = 0x014A;
pub const CB_RESETCONTENT: ::UINT = 0x014B;
pub const CB_FINDSTRING: ::UINT = 0x014C;
pub const CB_SELECTSTRING: ::UINT = 0x014D;
pub const CB_SETCURSEL: ::UINT = 0x014E;
pub const CB_SHOWDROPDOWN: ::UINT = 0x014F;
pub const CB_GETITEMDATA: ::UINT = 0x0150;
pub const CB_SETITEMDATA: ::UINT = 0x0151;
pub const CB_GETDROPPEDCONTROLRECT: ::UINT = 0x0152;
pub const CB_SETITEMHEIGHT: ::UINT = 0x0153;
pub const CB_GETITEMHEIGHT: ::UINT = 0x0154;
pub const CB_SETEXTENDEDUI: ::UINT = 0x0155;
pub const CB_GETEXTENDEDUI: ::UINT = 0x0156;
pub const CB_GETDROPPEDSTATE: ::UINT = 0x0157;
pub const CB_FINDSTRINGEXACT: ::UINT = 0x0158;
pub const CB_SETLOCALE: ::UINT = 0x0159;
pub const CB_GETLOCALE: ::UINT = 0x015A;
pub const CB_GETTOPINDEX: ::UINT = 0x015b;
pub const CB_SETTOPINDEX: ::UINT = 0x015c;
pub const CB_GETHORIZONTALEXTENT: ::UINT = 0x015d;
pub const CB_SETHORIZONTALEXTENT: ::UINT = 0x015e;
pub const CB_GETDROPPEDWIDTH: ::UINT = 0x015f;
pub const CB_SETDROPPEDWIDTH: ::UINT = 0x0160;
pub const CB_INITSTORAGE: ::UINT = 0x0161;
//12141
STRUCT!{nodebug struct NONCLIENTMETRICSA {
    cbSize: ::UINT,
    iBorderWidth: ::c_int,
    iScrollWidth: ::c_int,
    iScrollHeight: ::c_int,
    iCaptionWidth: ::c_int,
    iCaptionHeight: ::c_int,
    lfCaptionFont: ::LOGFONTA,
    iSmCaptionWidth: ::c_int,
    iSmCaptionHeight: ::c_int,
    lfSmCaptionFont: ::LOGFONTA,
    iMenuWidth: ::c_int,
    iMenuHeight: ::c_int,
    lfMenuFont: ::LOGFONTA,
    lfStatusFont: ::LOGFONTA,
    lfMessageFont: ::LOGFONTA,
    iPaddedBorderWidth: ::c_int,
}}
pub type LPNONCLIENTMETRICSA = *mut NONCLIENTMETRICSA;
STRUCT!{nodebug struct NONCLIENTMETRICSW {
    cbSize: ::UINT,
    iBorderWidth: ::c_int,
    iScrollWidth: ::c_int,
    iScrollHeight: ::c_int,
    iCaptionWidth: ::c_int,
    iCaptionHeight: ::c_int,
    lfCaptionFont: ::LOGFONTW,
    iSmCaptionWidth: ::c_int,
    iSmCaptionHeight: ::c_int,
    lfSmCaptionFont: ::LOGFONTW,
    iMenuWidth: ::c_int,
    iMenuHeight: ::c_int,
    lfMenuFont: ::LOGFONTW,
    lfStatusFont: ::LOGFONTW,
    lfMessageFont: ::LOGFONTW,
    iPaddedBorderWidth: ::c_int,
}}
pub type LPNONCLIENTMETRICSW = *mut NONCLIENTMETRICSW;
//12869
pub const MONITOR_DEFAULTTONULL: ::DWORD = 0x00000000;
pub const MONITOR_DEFAULTTOPRIMARY: ::DWORD = 0x00000001;
pub const MONITOR_DEFAULTTONEAREST: ::DWORD = 0x00000002;
//12900
pub const MONITORINFOF_PRIMARY: ::DWORD = 1;
pub const CCHDEVICENAME: usize = 32;
STRUCT!{struct MONITORINFO {
    cbSize: ::DWORD,
    rcMonitor: ::RECT,
    rcWork: ::RECT,
    dwFlags: ::DWORD,
}}
pub type LPMONITORINFO = *mut MONITORINFO;
STRUCT!{struct MONITORINFOEXA {
    cbSize: ::DWORD,
    rcMonitor: ::RECT,
    rcWork: ::RECT,
    dwFlags: ::DWORD,
    szDevice: [::CHAR; ::CCHDEVICENAME],
}}
pub type LPMONITORINFOEXA = *mut MONITORINFOEXA;
STRUCT!{struct MONITORINFOEXW {
    cbSize: ::DWORD,
    rcMonitor: ::RECT,
    rcWork: ::RECT,
    dwFlags: ::DWORD,
    szDevice: [::WCHAR; ::CCHDEVICENAME],
}}
pub type LPMONITORINFOEXW = *mut MONITORINFOEXW;
//12971
pub type MONITORENUMPROC = Option<unsafe extern "system" fn(
    ::HMONITOR, ::HDC, ::LPRECT, ::LPARAM,
) -> ::BOOL>;
//14098
DECLARE_HANDLE!(HRAWINPUT, HRAWINPUT__);
pub fn GET_RAWINPUT_CODE_WPARAM(wParam: ::WPARAM) -> ::WPARAM { wParam & 0xff }
pub const RIM_INPUT: ::WPARAM = 0;
pub const RIM_INPUTSINK: ::WPARAM = 1;
STRUCT!{struct RAWINPUTHEADER {
    dwType: ::DWORD,
    dwSize: ::DWORD,
    hDevice: ::HANDLE,
    wParam: ::WPARAM,
}}
pub type PRAWINPUTHEADER = *mut RAWINPUTHEADER;
pub type LPRAWINPUTHEADER = *mut RAWINPUTHEADER;
pub const RIM_TYPEMOUSE: ::DWORD = 0;
pub const RIM_TYPEKEYBOARD: ::DWORD = 1;
pub const RIM_TYPEHID: ::DWORD = 2;
STRUCT!{struct RAWMOUSE {
    usFlags: ::USHORT,
    memory_padding: ::USHORT, // 16bit Padding for 32bit align in following union
    usButtonFlags: ::USHORT,
    usButtonData: ::USHORT,
    ulRawButtons: ::ULONG,
    lLastX: ::LONG,
    lLastY: ::LONG,
    ulExtraInformation: ::ULONG,
}}
pub type PRAWMOUSE = *mut RAWMOUSE;
pub type LPRAWMOUSE = *mut RAWMOUSE;
pub const RI_MOUSE_LEFT_BUTTON_DOWN: ::USHORT = 0x0001;
pub const RI_MOUSE_LEFT_BUTTON_UP: ::USHORT = 0x0002;
pub const RI_MOUSE_RIGHT_BUTTON_DOWN: ::USHORT = 0x0004;
pub const RI_MOUSE_RIGHT_BUTTON_UP: ::USHORT = 0x0008;
pub const RI_MOUSE_MIDDLE_BUTTON_DOWN: ::USHORT = 0x0010;
pub const RI_MOUSE_MIDDLE_BUTTON_UP: ::USHORT = 0x0020;
pub const RI_MOUSE_BUTTON_1_DOWN: ::USHORT = RI_MOUSE_LEFT_BUTTON_DOWN;
pub const RI_MOUSE_BUTTON_1_UP: ::USHORT = RI_MOUSE_LEFT_BUTTON_UP;
pub const RI_MOUSE_BUTTON_2_DOWN: ::USHORT = RI_MOUSE_RIGHT_BUTTON_DOWN;
pub const RI_MOUSE_BUTTON_2_UP: ::USHORT = RI_MOUSE_RIGHT_BUTTON_UP;
pub const RI_MOUSE_BUTTON_3_DOWN: ::USHORT = RI_MOUSE_MIDDLE_BUTTON_DOWN;
pub const RI_MOUSE_BUTTON_3_UP: ::USHORT = RI_MOUSE_MIDDLE_BUTTON_UP;
pub const RI_MOUSE_BUTTON_4_DOWN: ::USHORT = 0x0040;
pub const RI_MOUSE_BUTTON_4_UP: ::USHORT = 0x0080;
pub const RI_MOUSE_BUTTON_5_DOWN: ::USHORT = 0x0100;
pub const RI_MOUSE_BUTTON_5_UP: ::USHORT = 0x0200;
pub const RI_MOUSE_WHEEL: ::USHORT = 0x0400;
pub const MOUSE_MOVE_RELATIVE: ::USHORT = 0;
pub const MOUSE_MOVE_ABSOLUTE: ::USHORT = 1;
pub const MOUSE_VIRTUAL_DESKTOP: ::USHORT = 0x02;
pub const MOUSE_ATTRIBUTES_CHANGED: ::USHORT = 0x04;
pub const MOUSE_MOVE_NOCOALESCE: ::USHORT = 0x08;
STRUCT!{struct RAWKEYBOARD {
    MakeCode: ::USHORT,
    Flags: ::USHORT,
    Reserved: ::USHORT,
    VKey: ::USHORT,
    Message: ::UINT,
    ExtraInformation: ::ULONG,
}}
pub type PRAWKEYBOARD = *mut RAWKEYBOARD;
pub type LPRAWKEYBOARD = *mut RAWKEYBOARD;
pub const KEYBOARD_OVERRUN_MAKE_CODE: ::DWORD = 0xFF;
pub const RI_KEY_MAKE: ::DWORD = 0;
pub const RI_KEY_BREAK: ::DWORD = 1;
pub const RI_KEY_E0: ::DWORD = 2;
pub const RI_KEY_E1: ::DWORD = 4;
pub const RI_KEY_TERMSRV_SET_LED: ::DWORD = 8;
pub const RI_KEY_TERMSRV_SHADOW: ::DWORD = 0x10;
STRUCT!{struct RAWHID {
    dwSizeHid: ::DWORD,
    dwCount: ::DWORD,
    bRawData: [::BYTE; 0],
}}
pub type PRAWHID = *mut RAWHID;
pub type LPRAWHID = *mut RAWHID;
STRUCT!{struct RAWINPUT {
    header: RAWINPUTHEADER,
    mouse: RAWMOUSE,
}}
UNION!(RAWINPUT, mouse, mouse, mouse_mut, RAWMOUSE);
UNION!(RAWINPUT, mouse, keyboard, keyboard_mut, RAWKEYBOARD);
UNION!(RAWINPUT, mouse, hid, hid_mut, RAWHID);
#[test]
fn test_RAWINPUT() {
    use std::mem::{size_of, align_of};
    assert!(size_of::<RAWMOUSE>() >= size_of::<RAWMOUSE>());
    assert!(size_of::<RAWMOUSE>() >= size_of::<RAWKEYBOARD>());
    assert!(size_of::<RAWMOUSE>() >= size_of::<RAWHID>());
    assert!(align_of::<RAWMOUSE>() >= align_of::<RAWMOUSE>());
    assert!(align_of::<RAWMOUSE>() >= align_of::<RAWKEYBOARD>());
    assert!(align_of::<RAWMOUSE>() >= align_of::<RAWHID>());
}
pub type PRAWINPUT = *mut RAWINPUT;
pub type LPRAWINPUT = *mut RAWINPUT;
pub const RID_INPUT: ::DWORD = 0x10000003;
pub const RID_HEADER: ::DWORD = 0x10000005;
pub const RIDI_PREPARSEDDATA: ::DWORD = 0x20000005;
pub const RIDI_DEVICENAME: ::DWORD = 0x20000007;
pub const RIDI_DEVICEINFO: ::DWORD = 0x2000000b;
STRUCT!{struct RID_DEVICE_INFO_MOUSE {
    dwId: ::DWORD,
    dwNumberOfButtons: ::DWORD,
    dwSampleRate: ::DWORD,
    fHasHorizontalWheel: ::BOOL,
}}
pub type PRID_DEVICE_INFO_MOUSE = *mut RID_DEVICE_INFO_MOUSE;
STRUCT!{struct RID_DEVICE_INFO_KEYBOARD {
    dwType: ::DWORD,
    dwSubType: ::DWORD,
    dwKeyboardMode: ::DWORD,
    dwNumberOfFunctionKeys: ::DWORD,
    dwNumberOfIndicators: ::DWORD,
    dwNumberOfKeysTotal: ::DWORD,
}}
pub type PRID_DEVICE_INFO_KEYBOARD = *mut RID_DEVICE_INFO_KEYBOARD;
STRUCT!{struct RID_DEVICE_INFO_HID {
    dwVendorId: ::DWORD,
    dwProductId: ::DWORD,
    dwVersionNumber: ::DWORD,
    usUsagePage: ::USHORT,
    usUsage: ::USHORT,
}}
pub type PRID_DEVICE_INFO_HID = *mut RID_DEVICE_INFO_HID;
STRUCT!{struct RID_DEVICE_INFO {
    cbSize: ::DWORD,
    dwType: ::DWORD,
    keyboard: RID_DEVICE_INFO_KEYBOARD,
}}
UNION!(RID_DEVICE_INFO, keyboard, mouse, mouse_mut, RID_DEVICE_INFO_MOUSE);
UNION!(RID_DEVICE_INFO, keyboard, keyboard, keyboard_mut, RID_DEVICE_INFO_KEYBOARD);
UNION!(RID_DEVICE_INFO, keyboard, hid, hid_mut, RID_DEVICE_INFO_HID);
#[test]
fn test_RID_DEVICE_INFO() {
    use std::mem::{size_of, align_of};
    assert!(size_of::<RID_DEVICE_INFO_KEYBOARD>() >= size_of::<RID_DEVICE_INFO_MOUSE>());
    assert!(size_of::<RID_DEVICE_INFO_KEYBOARD>() >= size_of::<RID_DEVICE_INFO_KEYBOARD>());
    assert!(size_of::<RID_DEVICE_INFO_KEYBOARD>() >= size_of::<RID_DEVICE_INFO_HID>());
    assert!(align_of::<RID_DEVICE_INFO_KEYBOARD>() >= align_of::<RID_DEVICE_INFO_MOUSE>());
    assert!(align_of::<RID_DEVICE_INFO_KEYBOARD>()
        >= align_of::<RID_DEVICE_INFO_KEYBOARD>());
    assert!(align_of::<RID_DEVICE_INFO_KEYBOARD>() >= align_of::<RID_DEVICE_INFO_HID>());
}
pub type PRID_DEVICE_INFO = *mut RID_DEVICE_INFO;
pub type LPRID_DEVICE_INFO = *mut RID_DEVICE_INFO;
STRUCT!{struct RAWINPUTDEVICE {
    usUsagePage: ::USHORT,
    usUsage: ::USHORT,
    dwFlags: ::DWORD,
    hwndTarget: ::HWND,
}}
pub type PRAWINPUTDEVICE = *mut RAWINPUTDEVICE;
pub type LPRAWINPUTDEVICE = *mut RAWINPUTDEVICE;
pub type PCRAWINPUTDEVICE = *const RAWINPUTDEVICE;
pub const RIDEV_REMOVE: ::DWORD = 0x00000001;
pub const RIDEV_EXCLUDE: ::DWORD = 0x00000010;
pub const RIDEV_PAGEONLY: ::DWORD = 0x00000020;
pub const RIDEV_NOLEGACY: ::DWORD = 0x00000030;
pub const RIDEV_INPUTSINK: ::DWORD = 0x00000100;
pub const RIDEV_CAPTUREMOUSE: ::DWORD = 0x00000200;
pub const RIDEV_NOHOTKEYS: ::DWORD = 0x00000200;
pub const RIDEV_APPKEYS: ::DWORD = 0x00000400;
pub const RIDEV_EXINPUTSINK: ::DWORD = 0x00001000;
pub const RIDEV_DEVNOTIFY: ::DWORD = 0x00002000;
pub const RIDEV_EXMODEMASK: ::DWORD = 0x000000F0;
pub const GIDC_ARRIVAL: ::DWORD = 1;
pub const GIDC_REMOVAL: ::DWORD = 2;
STRUCT!{struct RAWINPUTDEVICELIST {
    hDevice: ::HANDLE,
    dwType: ::DWORD,
}}
pub type PRAWINPUTDEVICELIST = *mut RAWINPUTDEVICELIST;
STRUCT!{struct CHANGEFILTERSTRUCT {
    cbSize: ::DWORD,
    ExtStatus: ::DWORD,
}}
pub type PCHANGEFILTERSTRUCT = *mut CHANGEFILTERSTRUCT;
STRUCT!{struct DLGTEMPLATE {
    style: ::DWORD,
    dwExtendedStyle: ::DWORD,
    cdit: ::WORD,
    x: ::c_short,
    y: ::c_short,
    cx: ::c_short,
    cy: ::c_short,
}}
pub type LPDLGTEMPLATEA = *mut DLGTEMPLATE;
pub type LPDLGTEMPLATEW = *mut DLGTEMPLATE;
pub type LPCDLGTEMPLATEA = *const DLGTEMPLATE;
pub type LPCDLGTEMPLATEW = *const DLGTEMPLATE;
STRUCT!{struct DRAWTEXTPARAMS {
    cbSize: ::UINT,
    iTabLength: ::c_int,
    iLeftMargin: ::c_int,
    iRightMargin: ::c_int,
    uiLengthDrawn: ::UINT,
}}
pub type LPDRAWTEXTPARAMS = *mut DRAWTEXTPARAMS;
STRUCT!{struct ACCEL {
    fVirt: ::BYTE,
    key: ::WORD,
    cmd: ::WORD,
}}
pub type LPACCEL = *mut ACCEL;
STRUCT!{struct MENUITEMINFOA {
    cbSize: ::UINT,
    fMask: ::UINT,
    fType: ::UINT,
    fState: ::UINT,
    wID: ::UINT,
    hSubMenu: ::HMENU,
    hbmpChecked: ::HBITMAP,
    hbmpUnchecked: ::HBITMAP,
    dwItemData: ::ULONG_PTR,
    dwTypeData: ::LPSTR,
    cch: ::UINT,
    hbmpItem: ::HBITMAP,
}}
pub type LPMENUITEMINFOA = *mut MENUITEMINFOA;
pub type LPCMENUITEMINFOA = *const MENUITEMINFOA;
STRUCT!{struct MENUITEMINFOW {
    cbSize: ::UINT,
    fMask: ::UINT,
    fType: ::UINT,
    fState: ::UINT,
    wID: ::UINT,
    hSubMenu: ::HMENU,
    hbmpChecked: ::HBITMAP,
    hbmpUnchecked: ::HBITMAP,
    dwItemData: ::ULONG_PTR,
    dwTypeData: ::LPWSTR,
    cch: ::UINT,
    hbmpItem: ::HBITMAP,
}}
pub type LPMENUITEMINFOW = *mut MENUITEMINFOW;
pub type LPCMENUITEMINFOW = *const MENUITEMINFOW;
STRUCT!{nodebug struct MSGBOXPARAMSA {
    cbSize: ::UINT,
    hwndOwner: ::HWND,
    hInstance: ::HINSTANCE,
    lpszText: ::LPCSTR,
    lpszCaption: ::LPCSTR,
    dwStyle: ::DWORD,
    lpszIcon: ::LPCSTR,
    dwContextHelpId: ::DWORD_PTR,
    lpfnMsgBoxCallback: ::MSGBOXCALLBACK,
    dwLanguageId: ::DWORD,
}}
pub type PMSGBOXPARAMSA = *mut MSGBOXPARAMSA;
pub type LPMSGBOXPARAMSA = *mut MSGBOXPARAMSA;
STRUCT!{nodebug struct MSGBOXPARAMSW {
    cbSize: ::UINT,
    hwndOwner: ::HWND,
    hInstance: ::HINSTANCE,
    lpszText: ::LPCWSTR,
    lpszCaption: ::LPCWSTR,
    dwStyle: ::DWORD,
    lpszIcon: ::LPCWSTR,
    dwContextHelpId: ::DWORD_PTR,
    lpfnMsgBoxCallback: ::MSGBOXCALLBACK,
    dwLanguageId: ::DWORD,
}}
pub type PMSGBOXPARAMSW = *mut MSGBOXPARAMSW;
pub type LPMSGBOXPARAMSW = *mut MSGBOXPARAMSW;
STRUCT!{struct HELPINFO {
    cbSize: ::UINT,
    iContextType: ::c_int,
    iCtrlId: ::c_int,
    hItemHandle: ::HANDLE,
    dwContextId: ::DWORD,
    MousePos: ::POINT,
}}
pub type LPHELPINFO = *mut HELPINFO;
#[allow(trivial_numeric_casts)]
pub fn GET_WHEEL_DELTA_WPARAM(wParam: ::WPARAM) -> ::c_short {
    ::HIWORD(wParam as ::DWORD) as ::c_short
}
#[allow(trivial_numeric_casts)]
pub fn GET_KEYSTATE_WPARAM(wparam: ::WPARAM) -> ::c_int {
    ::LOWORD(wparam as ::DWORD) as ::c_short as ::c_int
}
#[allow(trivial_numeric_casts)]
pub fn GET_XBUTTON_WPARAM(wparam: ::WPARAM) -> ::c_int {
    ::HIWORD(wparam as ::DWORD) as ::c_int
}
pub const SIF_RANGE: ::UINT = 0x0001;
pub const SIF_PAGE: ::UINT = 0x0002;
pub const SIF_POS: ::UINT = 0x0004;
pub const SIF_DISABLENOSCROLL: ::UINT = 0x0008;
pub const SIF_TRACKPOS: ::UINT = 0x0010;
pub const SIF_ALL: ::UINT = SIF_RANGE | SIF_PAGE | SIF_POS | SIF_TRACKPOS;
pub const SW_SCROLLCHILDREN: ::UINT = 0x0001;
pub const SW_INVALIDATE: ::UINT = 0x0002;
pub const SW_ERASE: ::UINT = 0x0004;
pub const SW_SMOOTHSCROLL: ::UINT = 0x0010;
pub const SB_LINEUP: ::c_int = 0;
pub const SB_LINELEFT: ::c_int = 0;
pub const SB_LINEDOWN: ::c_int = 1;
pub const SB_LINERIGHT: ::c_int = 1;
pub const SB_PAGEUP: ::c_int = 2;
pub const SB_PAGELEFT: ::c_int = 2;
pub const SB_PAGEDOWN: ::c_int = 3;
pub const SB_PAGERIGHT: ::c_int = 3;
pub const SB_THUMBPOSITION: ::c_int = 4;
pub const SB_THUMBTRACK: ::c_int = 5;
pub const SB_TOP: ::c_int = 6;
pub const SB_LEFT: ::c_int = 6;
pub const SB_BOTTOM: ::c_int = 7;
pub const SB_RIGHT: ::c_int = 7;
pub const SB_ENDSCROLL: ::c_int = 8;
pub const LR_DEFAULTCOLOR: ::UINT = 0x00000000;
pub const LR_MONOCHROME: ::UINT = 0x00000001;
pub const LR_COLOR: ::UINT = 0x00000002;
pub const LR_COPYRETURNORG: ::UINT = 0x00000004;
pub const LR_COPYDELETEORG: ::UINT = 0x00000008;
pub const LR_LOADFROMFILE: ::UINT = 0x00000010;
pub const LR_LOADTRANSPARENT: ::UINT = 0x00000020;
pub const LR_DEFAULTSIZE: ::UINT = 0x00000040;
pub const LR_VGACOLOR: ::UINT = 0x00000080;
pub const LR_LOADMAP3DCOLORS: ::UINT = 0x00001000;
pub const LR_CREATEDIBSECTION: ::UINT = 0x00002000;
pub const LR_COPYFROMRESOURCE: ::UINT = 0x00004000;
pub const LR_SHARED: ::UINT = 0x00008000;
pub const IMAGE_BITMAP: ::UINT = 0;
pub const IMAGE_ICON: ::UINT = 1;
pub const IMAGE_CURSOR: ::UINT = 2;
pub const IMAGE_ENHMETAFILE: ::UINT = 3;
pub const DT_TOP: ::UINT = 0x00000000;
pub const DT_LEFT: ::UINT = 0x00000000;
pub const DT_CENTER: ::UINT = 0x00000001;
pub const DT_RIGHT: ::UINT = 0x00000002;
pub const DT_VCENTER: ::UINT = 0x00000004;
pub const DT_BOTTOM: ::UINT = 0x00000008;
pub const DT_WORDBREAK: ::UINT = 0x00000010;
pub const DT_SINGLELINE: ::UINT = 0x00000020;
pub const DT_EXPANDTABS: ::UINT = 0x00000040;
pub const DT_TABSTOP: ::UINT = 0x00000080;
pub const DT_NOCLIP: ::UINT = 0x00000100;
pub const DT_EXTERNALLEADING: ::UINT = 0x00000200;
pub const DT_CALCRECT: ::UINT = 0x00000400;
pub const DT_NOPREFIX: ::UINT = 0x00000800;
pub const DT_INTERNAL: ::UINT = 0x00001000;
pub const DT_EDITCONTROL: ::UINT = 0x00002000;
pub const DT_PATH_ELLIPSIS: ::UINT = 0x00004000;
pub const DT_END_ELLIPSIS: ::UINT = 0x00008000;
pub const DT_MODIFYSTRING: ::UINT = 0x00010000;
pub const DT_RTLREADING: ::UINT = 0x00020000;
pub const DT_WORD_ELLIPSIS: ::UINT = 0x00040000;
pub const DT_NOFULLWIDTHCHARBREAK: ::UINT = 0x00080000;
pub const DT_HIDEPREFIX: ::UINT = 0x00100000;
pub const DT_PREFIXONLY: ::UINT = 0x00200000;
STRUCT!{struct KBDLLHOOKSTRUCT {
    vkCode: ::DWORD,
    scanCode: ::DWORD,
    flags: ::DWORD,
    time: ::DWORD,
    dwExtraInfo: ::ULONG_PTR,
}}
pub type PKBDLLHOOKSTRUCT = *mut KBDLLHOOKSTRUCT;
pub type LPKBDLLHOOKSTRUCT = *mut KBDLLHOOKSTRUCT;
STRUCT!{struct MSLLHOOKSTRUCT {
    pt: ::POINT,
    mouseData: ::DWORD,
    flags: ::DWORD,
    time: ::DWORD,
    dwExtraInfo: ::ULONG_PTR,
}}
pub type PMSLLHOOKSTRUCT = *mut MSLLHOOKSTRUCT;
pub type LPMSLLHOOKSTRUCT = *mut MSLLHOOKSTRUCT;
pub const WH_MIN: ::c_int = -1;
pub const WH_MSGFILTER: ::c_int = -1;
pub const WH_JOURNALRECORD: ::c_int = 0;
pub const WH_JOURNALPLAYBACK: ::c_int = 1;
pub const WH_KEYBOARD: ::c_int = 2;
pub const WH_GETMESSAGE: ::c_int = 3;
pub const WH_CALLWNDPROC: ::c_int = 4;
pub const WH_CBT: ::c_int = 5;
pub const WH_SYSMSGFILTER: ::c_int = 6;
pub const WH_MOUSE: ::c_int = 7;
pub const WH_HARDWARE: ::c_int = 8;
pub const WH_DEBUG: ::c_int = 9;
pub const WH_SHELL: ::c_int = 10;
pub const WH_FOREGROUNDIDLE: ::c_int = 11;
pub const WH_CALLWNDPROCRET: ::c_int = 12;
pub const WH_KEYBOARD_LL: ::c_int = 13;
pub const WH_MOUSE_LL: ::c_int = 14;
pub const WH_MAX: ::c_int = 14;
pub const WH_MINHOOK: ::c_int = WH_MIN;
pub const WH_MAXHOOK: ::c_int = WH_MAX;
pub const KLF_ACTIVATE: ::UINT = 1;
pub const KLF_SUBSTITUTE_OK: ::UINT = 2;
pub const KLF_UNLOADPREVIOUS: ::UINT = 4;
pub const KLF_REORDER: ::UINT = 8;
pub const KLF_REPLACELANG: ::UINT = 16;
pub const KLF_NOTELLSHELL: ::UINT = 128;
pub const KLF_SETFORPROCESS: ::UINT = 256;
//RedrawWindow() flags
pub const RDW_INVALIDATE: ::UINT = 0x0001;
pub const RDW_INTERNALPAINT: ::UINT = 0x0002;
pub const RDW_ERASE: ::UINT = 0x0004;
pub const RDW_VALIDATE: ::UINT = 0x0008;
pub const RDW_NOINTERNALPAINT: ::UINT = 0x0010;
pub const RDW_NOERASE: ::UINT = 0x0020;
pub const RDW_NOCHILDREN: ::UINT = 0x0040;
pub const RDW_ALLCHILDREN: ::UINT = 0x0080;
pub const RDW_UPDATENOW: ::UINT = 0x0100;
pub const RDW_ERASENOW: ::UINT = 0x0200;
pub const RDW_FRAME: ::UINT = 0x0400;
pub const RDW_NOFRAME: ::UINT = 0x0800;
STRUCT!{struct MEASUREITEMSTRUCT {
    CtlType: ::UINT,
    CtlID: ::UINT,
    itemID: ::UINT,
    itemWidth: ::UINT,
    itemHeight: ::UINT,
    itemData: ::ULONG_PTR,
}}
pub type LPMEASUREITEMSTRUCT = *mut MEASUREITEMSTRUCT;
STRUCT!{struct DRAWITEMSTRUCT {
    CtlType: ::UINT,
    CtlID: ::UINT,
    itemID: ::UINT,
    itemAction: ::UINT,
    itemState: ::UINT,
    hwndItem: ::HWND,
    hDC: ::HDC,
    rcItem: ::RECT,
    itemData: ::ULONG_PTR,
}}
pub type LPDRAWITEMSTRUCT = *mut DRAWITEMSTRUCT;
STRUCT!{struct DELETEITEMSTRUCT {
    CtlType: ::UINT,
    CtlID: ::UINT,
    itemID: ::UINT,
    hwndItem: ::HWND,
    itemData: ::ULONG_PTR,
}}
pub type LPDELETEITEMSTRUCT = *mut DELETEITEMSTRUCT;
STRUCT!{struct COMPAREITEMSTRUCT {
    CtlType: ::UINT,
    CtlID: ::UINT,
    hwndItem: ::HWND,
    itemID1: ::UINT,
    itemData1: ::ULONG_PTR,
    itemID2: ::UINT,
    itemData2: ::ULONG_PTR,
    dwLocaleId: ::DWORD,
}}
pub type LPCOMPAREITEMSTRUCT = *mut COMPAREITEMSTRUCT;
/* Image type */
pub const DST_COMPLEX: ::UINT = 0x0000;
pub const DST_TEXT: ::UINT = 0x0001;
pub const DST_PREFIXTEXT: ::UINT = 0x0002;
pub const DST_ICON: ::UINT = 0x0003;
pub const DST_BITMAP: ::UINT = 0x0004;
pub const DI_MASK: ::UINT = 0x0001;
pub const DI_IMAGE: ::UINT = 0x0002;
pub const DI_NORMAL: ::UINT = 0x0003;
pub const DI_COMPAT: ::UINT = 0x0004;
pub const DI_DEFAULTSIZE: ::UINT = 0x0008;
// if WINVER >= 0x0601
// GetSystemMetrics(SM_DIGITIZER) flag values
pub const NID_INTEGRATED_TOUCH: ::UINT = 0x00000001;
pub const NID_EXTERNAL_TOUCH: ::UINT = 0x00000002;
pub const NID_INTEGRATED_PEN: ::UINT = 0x00000004;
pub const NID_EXTERNAL_PEN: ::UINT = 0x00000008;
pub const NID_MULTI_INPUT: ::UINT = 0x00000040;
pub const NID_READY: ::UINT = 0x00000080;
// end if WINVER >= 0x0601

// System Menu Command Values
//
pub const SC_SIZE: ::WPARAM = 0xF000;
pub const SC_MOVE: ::WPARAM = 0xF010;
pub const SC_MINIMIZE: ::WPARAM = 0xF020;
pub const SC_MAXIMIZE: ::WPARAM = 0xF030;
pub const SC_NEXTWINDOW: ::WPARAM = 0xF040;
pub const SC_PREVWINDOW: ::WPARAM = 0xF050;
pub const SC_CLOSE: ::WPARAM = 0xF060;
pub const SC_VSCROLL: ::WPARAM = 0xF070;
pub const SC_HSCROLL: ::WPARAM = 0xF080;
pub const SC_MOUSEMENU: ::WPARAM = 0xF090;
pub const SC_KEYMENU: ::WPARAM = 0xF100;
pub const SC_ARRANGE: ::WPARAM = 0xF110;
pub const SC_RESTORE: ::WPARAM = 0xF120;
pub const SC_TASKLIST: ::WPARAM = 0xF130;
pub const SC_SCREENSAVE: ::WPARAM = 0xF140;
pub const SC_HOTKEY: ::WPARAM = 0xF150;
// if WINVER >= 0x0400
pub const SC_DEFAULT: ::WPARAM = 0xF160;
pub const SC_MONITORPOWER: ::WPARAM = 0xF170;
pub const SC_CONTEXTHELP: ::WPARAM = 0xF180;
pub const SC_SEPARATOR: ::WPARAM = 0xF00F;
// endif WINVER >= 0x0400
