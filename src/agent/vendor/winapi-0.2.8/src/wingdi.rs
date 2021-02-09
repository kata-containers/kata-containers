// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! GDI procedure declarations, constant definitions and macros
pub const DISPLAY_DEVICE_ATTACHED_TO_DESKTOP: ::DWORD = 0x00000001;
pub const DISPLAY_DEVICE_MULTI_DRIVER: ::DWORD = 0x00000002;
pub const DISPLAY_DEVICE_PRIMARY_DEVICE: ::DWORD = 0x00000004;
pub const DISPLAY_DEVICE_MIRRORING_DRIVER: ::DWORD = 0x00000008;
pub const DISPLAY_DEVICE_VGA_COMPATIBLE: ::DWORD = 0x00000010;
pub const DISPLAY_DEVICE_REMOVABLE: ::DWORD = 0x00000020;
pub const DISPLAY_DEVICE_ACC_DRIVER: ::DWORD = 0x00000040;
pub const DISPLAY_DEVICE_MODESPRUNED: ::DWORD = 0x08000000;
pub const DISPLAY_DEVICE_REMOTE: ::DWORD = 0x04000000;
pub const DISPLAY_DEVICE_DISCONNECT: ::DWORD = 0x02000000;
pub const DISPLAY_DEVICE_TS_COMPATIBLE: ::DWORD = 0x00200000;
pub const DISPLAY_DEVICE_UNSAFE_MODES_ON: ::DWORD = 0x00080000;
pub const DISPLAY_DEVICE_ACTIVE: ::DWORD = 0x00000001;
pub const DISPLAY_DEVICE_ATTACHED: ::DWORD = 0x00000002;
pub const DM_ORIENTATION: ::DWORD = 0x00000001;
pub const DM_PAPERSIZE: ::DWORD = 0x00000002;
pub const DM_PAPERLENGTH: ::DWORD = 0x00000004;
pub const DM_PAPERWIDTH: ::DWORD = 0x00000008;
pub const DM_SCALE: ::DWORD = 0x00000010;
pub const DM_POSITION: ::DWORD = 0x00000020;
pub const DM_NUP: ::DWORD = 0x00000040;
pub const DM_DISPLAYORIENTATION: ::DWORD = 0x00000080;
pub const DM_COPIES: ::DWORD = 0x00000100;
pub const DM_DEFAULTSOURCE: ::DWORD = 0x00000200;
pub const DM_PRINTQUALITY: ::DWORD = 0x00000400;
pub const DM_COLOR: ::DWORD = 0x00000800;
pub const DM_DUPLEX: ::DWORD = 0x00001000;
pub const DM_YRESOLUTION: ::DWORD = 0x00002000;
pub const DM_TTOPTION: ::DWORD = 0x00004000;
pub const DM_COLLATE: ::DWORD = 0x00008000;
pub const DM_FORMNAME: ::DWORD = 0x00010000;
pub const DM_LOGPIXELS: ::DWORD = 0x00020000;
pub const DM_BITSPERPEL: ::DWORD = 0x00040000;
pub const DM_PELSWIDTH: ::DWORD = 0x00080000;
pub const DM_PELSHEIGHT: ::DWORD = 0x00100000;
pub const DM_DISPLAYFLAGS: ::DWORD = 0x00200000;
pub const DM_DISPLAYFREQUENCY: ::DWORD = 0x00400000;
pub const DM_ICMMETHOD: ::DWORD = 0x00800000;
pub const DM_ICMINTENT: ::DWORD = 0x01000000;
pub const DM_MEDIATYPE: ::DWORD = 0x02000000;
pub const DM_DITHERTYPE: ::DWORD = 0x04000000;
pub const DM_PANNINGWIDTH: ::DWORD = 0x08000000;
pub const DM_PANNINGHEIGHT: ::DWORD = 0x10000000;
pub const DM_DISPLAYFIXEDOUTPUT: ::DWORD = 0x20000000;
pub const PFD_TYPE_RGBA: ::BYTE = 0;
pub const PFD_TYPE_COLORINDEX: ::BYTE = 1;
pub const PFD_MAIN_PLANE: ::BYTE = 0;
pub const PFD_OVERLAY_PLANE: ::BYTE = 1;
pub const PFD_UNDERLAY_PLANE: ::BYTE = 0xFF;
pub const PFD_DOUBLEBUFFER: ::DWORD = 0x00000001;
pub const PFD_STEREO: ::DWORD = 0x00000002;
pub const PFD_DRAW_TO_WINDOW: ::DWORD = 0x00000004;
pub const PFD_DRAW_TO_BITMAP: ::DWORD = 0x00000008;
pub const PFD_SUPPORT_GDI: ::DWORD = 0x00000010;
pub const PFD_SUPPORT_OPENGL: ::DWORD = 0x00000020;
pub const PFD_GENERIC_FORMAT: ::DWORD = 0x00000040;
pub const PFD_NEED_PALETTE: ::DWORD = 0x00000080;
pub const PFD_NEED_SYSTEM_PALETTE: ::DWORD = 0x00000100;
pub const PFD_SWAP_EXCHANGE: ::DWORD = 0x00000200;
pub const PFD_SWAP_COPY: ::DWORD = 0x00000400;
pub const PFD_SWAP_LAYER_BUFFERS: ::DWORD = 0x00000800;
pub const PFD_GENERIC_ACCELERATED: ::DWORD = 0x00001000;
pub const PFD_SUPPORT_DIRECTDRAW: ::DWORD = 0x00002000;
pub const PFD_DIRECT3D_ACCELERATED: ::DWORD = 0x00004000;
pub const PFD_SUPPORT_COMPOSITION: ::DWORD = 0x00008000;
pub const PFD_DEPTH_DONTCARE: ::DWORD = 0x20000000;
pub const PFD_DOUBLEBUFFER_DONTCARE: ::DWORD = 0x40000000;
pub const PFD_STEREO_DONTCARE: ::DWORD = 0x80000000;
pub const CCHFORMNAME: usize = 32;
STRUCT!{struct DEVMODEA {
    dmDeviceName: [::CHAR; ::CCHDEVICENAME],
    dmSpecVersion: ::WORD,
    dmDriverVersion: ::WORD,
    dmSize: ::WORD,
    dmDriverExtra: ::WORD,
    dmFields: ::DWORD,
    union1: [u8; 16],
    dmColor: ::c_short,
    dmDuplex: ::c_short,
    dmYResolution: ::c_short,
    dmTTOption: ::c_short,
    dmCollate: ::c_short,
    dmFormName: [::CHAR; CCHFORMNAME],
    dmLogPixels: ::WORD,
    dmBitsPerPel: ::DWORD,
    dmPelsWidth: ::DWORD,
    dmPelsHeight: ::DWORD,
    dmDisplayFlags: ::DWORD,
    dmDisplayFrequency: ::DWORD,
    dmICMMethod: ::DWORD,
    dmICMIntent: ::DWORD,
    dmMediaType: ::DWORD,
    dmDitherType: ::DWORD,
    dmReserved1: ::DWORD,
    dmReserved2: ::DWORD,
    dmPanningWidth: ::DWORD,
    dmPanningHeight: ::DWORD,
}}
pub type PDEVMODEA = *mut DEVMODEA;
pub type NPDEVMODEA = *mut DEVMODEA;
pub type LPDEVMODEA = *mut DEVMODEA;
STRUCT!{struct DEVMODEW {
    dmDeviceName: [::WCHAR; ::CCHDEVICENAME],
    dmSpecVersion: ::WORD,
    dmDriverVersion: ::WORD,
    dmSize: ::WORD,
    dmDriverExtra: ::WORD,
    dmFields: ::DWORD,
    union1: [u8; 16],
    dmColor: ::c_short,
    dmDuplex: ::c_short,
    dmYResolution: ::c_short,
    dmTTOption: ::c_short,
    dmCollate: ::c_short,
    dmFormName: [::WCHAR; CCHFORMNAME],
    dmLogPixels: ::WORD,
    dmBitsPerPel: ::DWORD,
    dmPelsWidth: ::DWORD,
    dmPelsHeight: ::DWORD,
    dmDisplayFlags: ::DWORD,
    dmDisplayFrequency: ::DWORD,
    dmICMMethod: ::DWORD,
    dmICMIntent: ::DWORD,
    dmMediaType: ::DWORD,
    dmDitherType: ::DWORD,
    dmReserved1: ::DWORD,
    dmReserved2: ::DWORD,
    dmPanningWidth: ::DWORD,
    dmPanningHeight: ::DWORD,
}}
pub type PDEVMODEW = *mut DEVMODEW;
pub type NPDEVMODEW = *mut DEVMODEW;
pub type LPDEVMODEW = *mut DEVMODEW;
STRUCT!{nodebug struct DISPLAY_DEVICEW {
    cb: ::DWORD,
    DeviceName: [::WCHAR; 32],
    DeviceString: [::WCHAR; 128],
    StateFlags: ::DWORD,
    DeviceID: [::WCHAR; 128],
    DeviceKey: [::WCHAR; 128],
}}
pub type PDISPLAY_DEVICEW = *mut DISPLAY_DEVICEW;
pub type LPDISPLAY_DEVICEW = *mut DISPLAY_DEVICEW;
STRUCT!{nodebug struct DISPLAY_DEVICEA {
    cb: ::DWORD,
    DeviceName: [::CHAR; 32],
    DeviceString: [::CHAR; 128],
    StateFlags: ::DWORD,
    DeviceID: [::CHAR; 128],
    DeviceKey: [::CHAR; 128],
}}
pub type PDISPLAY_DEVICEA = *mut DISPLAY_DEVICEA;
pub type LPDISPLAY_DEVICEA = *mut DISPLAY_DEVICEA;
STRUCT!{struct PIXELFORMATDESCRIPTOR {
    nSize: ::WORD,
    nVersion: ::WORD,
    dwFlags: ::DWORD,
    iPixelType: ::BYTE,
    cColorBits: ::BYTE,
    cRedBits: ::BYTE,
    cRedShift: ::BYTE,
    cGreenBits: ::BYTE,
    cGreenShift: ::BYTE,
    cBlueBits: ::BYTE,
    cBlueShift: ::BYTE,
    cAlphaBits: ::BYTE,
    cAlphaShift: ::BYTE,
    cAccumBits: ::BYTE,
    cAccumRedBits: ::BYTE,
    cAccumGreenBits: ::BYTE,
    cAccumBlueBits: ::BYTE,
    cAccumAlphaBits: ::BYTE,
    cDepthBits: ::BYTE,
    cStencilBits: ::BYTE,
    cAuxBuffers: ::BYTE,
    iLayerType: ::BYTE,
    bReserved: ::BYTE,
    dwLayerMask: ::DWORD,
    dwVisibleMask: ::DWORD,
    dwDamageMask: ::DWORD,
}}
pub type PPIXELFORMATDESCRIPTOR = *mut PIXELFORMATDESCRIPTOR;
pub type LPPIXELFORMATDESCRIPTOR = *mut PIXELFORMATDESCRIPTOR;
pub const R2_BLACK: ::c_int = 1;
pub const R2_NOTMERGEPEN: ::c_int = 2;
pub const R2_MASKNOTPEN: ::c_int = 3;
pub const R2_NOTCOPYPEN: ::c_int = 4;
pub const R2_MASKPENNOT: ::c_int = 5;
pub const R2_NOT: ::c_int = 6;
pub const R2_XORPEN: ::c_int = 7;
pub const R2_NOTMASKPEN: ::c_int = 8;
pub const R2_MASKPEN: ::c_int = 9;
pub const R2_NOTXORPEN: ::c_int = 10;
pub const R2_NOP: ::c_int = 11;
pub const R2_MERGENOTPEN: ::c_int = 12;
pub const R2_COPYPEN: ::c_int = 13;
pub const R2_MERGEPENNOT: ::c_int = 14;
pub const R2_MERGEPEN: ::c_int = 15;
pub const R2_WHITE: ::c_int = 16;
pub const R2_LAST: ::c_int = 16;
//83
pub const SRCCOPY: ::DWORD = 0x00CC0020;
pub const SRCPAINT: ::DWORD = 0x00EE0086;
pub const SRCAND: ::DWORD = 0x008800C6;
pub const SRCINVERT: ::DWORD = 0x00660046;
pub const SRCERASE: ::DWORD = 0x00440328;
pub const NOTSRCCOPY: ::DWORD = 0x00330008;
pub const NOTSRCERASE: ::DWORD = 0x001100A6;
pub const MERGECOPY: ::DWORD = 0x00C000CA;
pub const MERGEPAINT: ::DWORD = 0x00BB0226;
pub const PATCOPY: ::DWORD = 0x00F00021;
pub const PATPAINT: ::DWORD = 0x00FB0A09;
pub const PATINVERT: ::DWORD = 0x005A0049;
pub const DSTINVERT: ::DWORD = 0x00550009;
pub const BLACKNESS: ::DWORD = 0x00000042;
pub const WHITENESS: ::DWORD = 0x00FF0062;
//121
// fnCombineMode values for CombineRgn
pub const RGN_AND: ::c_int = 1;
pub const RGN_OR: ::c_int = 2;
pub const RGN_XOR: ::c_int = 3;
pub const RGN_DIFF: ::c_int = 4;
pub const RGN_COPY: ::c_int = 5;
pub const RGN_MIN: ::c_int = RGN_AND;
pub const RGN_MAX: ::c_int = RGN_COPY;
//572 (Win 7 SDK)
STRUCT!{struct BITMAP {
    bmType: ::LONG,
    bmWidth: ::LONG,
    bmHeight: ::LONG,
    bmWidthBytes: ::LONG,
    bmPlanes: ::WORD,
    bmBitsPixel: ::WORD,
    bmBits: ::LPVOID,
}}
pub type PBITMAP = *mut BITMAP;
pub type NPBITMAP = *mut BITMAP;
pub type LPBITMAP = *mut BITMAP;
STRUCT!{struct RGBQUAD {
    rgbBlue: ::BYTE,
    rgbGreen: ::BYTE,
    rgbRed: ::BYTE,
    rgbReserved: ::BYTE,
}}
pub type LPRGBQUAD = *mut RGBQUAD;
pub const CS_ENABLE: ::DWORD = 0x00000001;
pub const CS_DISABLE: ::DWORD = 0x00000002;
pub const CS_DELETE_TRANSFORM: ::DWORD = 0x00000003;
pub const LCS_SIGNATURE: ::DWORD = 0x5053_4F43; // 'PSOC'
pub const LCS_sRGB: LCSCSTYPE = 0x7352_4742; // 'sRGB'
pub const LCS_WINDOWS_COLOR_SPACE: LCSCSTYPE = 0x5769_6E20; // 'Win '
pub type LCSCSTYPE = ::LONG;
pub const LCS_CALIBRATED_RGB: LCSCSTYPE = 0x00000000;
pub type LCSGAMUTMATCH = ::LONG;
pub const LCS_GM_BUSINESS: LCSGAMUTMATCH = 0x00000001;
pub const LCS_GM_GRAPHICS: LCSGAMUTMATCH = 0x00000002;
pub const LCS_GM_IMAGES: LCSGAMUTMATCH = 0x00000004;
pub const LCS_GM_ABS_COLORIMETRIC: LCSGAMUTMATCH = 0x00000008;
pub const CM_OUT_OF_GAMUT: ::BYTE = 255;
pub const CM_IN_GAMUT: ::BYTE = 0;
pub const ICM_ADDPROFILE: ::UINT = 1;
pub const ICM_DELETEPROFILE: ::UINT = 2;
pub const ICM_QUERYPROFILE: ::UINT = 3;
pub const ICM_SETDEFAULTPROFILE: ::UINT = 4;
pub const ICM_REGISTERICMATCHER: ::UINT = 5;
pub const ICM_UNREGISTERICMATCHER: ::UINT = 6;
pub const ICM_QUERYMATCH: ::UINT = 7;
pub type FXPT16DOT16 = ::c_long;
pub type LPFXPT16DOT16 = *mut ::c_long;
pub type FXPT2DOT30 = ::c_long;
pub type LPFXPT2DOT30 = *mut ::c_long;
STRUCT!{struct CIEXYZ {
    ciexyzX: FXPT2DOT30,
    ciexyzY: FXPT2DOT30,
    ciexyzZ: FXPT2DOT30,
}}
pub type LPCIEXYZ = *mut CIEXYZ;
STRUCT!{struct CIEXYZTRIPLE {
    ciexyzRed: CIEXYZ,
    ciexyzGreen: CIEXYZ,
    ciexyzBlue: CIEXYZ,
}}
pub type LPCIEXYZTRIPLE = *mut CIEXYZTRIPLE;
//716 (Win 7 SDK)
STRUCT!{struct BITMAPINFOHEADER {
    biSize: ::DWORD,
    biWidth: ::LONG,
    biHeight: ::LONG,
    biPlanes: ::WORD,
    biBitCount: ::WORD,
    biCompression: ::DWORD,
    biSizeImage: ::DWORD,
    biXPelsPerMeter: ::LONG,
    biYPelsPerMeter: ::LONG,
    biClrUsed: ::DWORD,
    biClrImportant: ::DWORD,
}}
pub type LPBITMAPINFOHEADER = *mut BITMAPINFOHEADER;
pub type PBITMAPINFOHEADER = *mut BITMAPINFOHEADER;
STRUCT!{struct BITMAPV5HEADER {
    bV5Size: ::DWORD,
    bV5Width: ::LONG,
    bV5Height: ::LONG,
    bV5Planes: ::WORD,
    bV5BitCount: ::WORD,
    bV5Compression: ::DWORD,
    bV5SizeImage: ::DWORD,
    bV5XPelsPerMeter: ::LONG,
    bV5YPelsPerMeter: ::LONG,
    bV5ClrUsed: ::DWORD,
    bV5ClrImportant: ::DWORD,
    bV5RedMask: ::DWORD,
    bV5GreenMask: ::DWORD,
    bV5BlueMask: ::DWORD,
    bV5AlphaMask: ::DWORD,
    bV5CSType: ::LONG, // LONG to match LOGCOLORSPACE
    bV5Endpoints: CIEXYZTRIPLE,
    bV5GammaRed: ::DWORD,
    bV5GammaGreen: ::DWORD,
    bV5GammaBlue: ::DWORD,
    bV5Intent: ::LONG, // LONG to match LOGCOLORSPACE
    bV5ProfileData: ::DWORD,
    bV5ProfileSize: ::DWORD,
    bV5Reserved: ::DWORD,
}}
pub type LPBITMAPV5HEADER = *mut BITMAPV5HEADER;
pub type PBITMAPV5HEADER = *mut BITMAPV5HEADER;
pub const PROFILE_LINKED: ::LONG = 0x4C49_4E4B; // 'LINK'
pub const PROFILE_EMBEDDED: ::LONG = 0x4D42_4544; // 'MBED'
pub const BI_RGB: ::DWORD = 0;
pub const BI_RLE8: ::DWORD = 1;
pub const BI_RLE4: ::DWORD = 2;
pub const BI_BITFIELDS: ::DWORD = 3;
pub const BI_JPEG: ::DWORD = 4;
pub const BI_PNG: ::DWORD = 5;
STRUCT!{struct BITMAPINFO {
    bmiHeader: BITMAPINFOHEADER,
    bmiColors: [RGBQUAD; 0],
}}
pub type LPBITMAPINFO = *mut BITMAPINFO;
pub type PBITMAPINFO = *mut BITMAPINFO;
//1438
pub const LF_FACESIZE: usize = 32;
STRUCT!{nodebug struct LOGFONTA {
    lfHeight: ::LONG,
    lfWidth: ::LONG,
    lfEscapement: ::LONG,
    lfOrientation: ::LONG,
    lfWeight: ::LONG,
    lfItalic: ::BYTE,
    lfUnderline: ::BYTE,
    lfStrikeOut: ::BYTE,
    lfCharSet: ::BYTE,
    lfOutPrecision: ::BYTE,
    lfClipPrecision: ::BYTE,
    lfQuality: ::BYTE,
    lfPitchAndFamily: ::BYTE,
    lfFaceName: [::CHAR; LF_FACESIZE],
}}
pub type LPLOGFONTA = *mut LOGFONTA;
STRUCT!{nodebug struct LOGFONTW {
    lfHeight: ::LONG,
    lfWidth: ::LONG,
    lfEscapement: ::LONG,
    lfOrientation: ::LONG,
    lfWeight: ::LONG,
    lfItalic: ::BYTE,
    lfUnderline: ::BYTE,
    lfStrikeOut: ::BYTE,
    lfCharSet: ::BYTE,
    lfOutPrecision: ::BYTE,
    lfClipPrecision: ::BYTE,
    lfQuality: ::BYTE,
    lfPitchAndFamily: ::BYTE,
    lfFaceName: [::WCHAR; LF_FACESIZE],
}}
pub type LPLOGFONTW = *mut LOGFONTW;
//1595
#[inline]
pub fn RGB (r: ::BYTE, g: ::BYTE, b: ::BYTE) -> ::COLORREF {
  r as ::COLORREF | ((g as ::COLORREF) << 8) | ((b as ::COLORREF) << 16)
}
//
pub const DRIVERVERSION: ::c_int = 0;
pub const TECHNOLOGY: ::c_int = 2;
pub const HORZSIZE: ::c_int = 4;
pub const VERTSIZE: ::c_int = 6;
pub const HORZRES: ::c_int = 8;
pub const VERTRES: ::c_int = 10;
pub const BITSPIXEL: ::c_int = 12;
pub const PLANES: ::c_int = 14;
pub const NUMBRUSHES: ::c_int = 16;
pub const NUMPENS: ::c_int = 18;
pub const NUMMARKERS: ::c_int = 20;
pub const NUMFONTS: ::c_int = 22;
pub const NUMCOLORS: ::c_int = 24;
pub const PDEVICESIZE: ::c_int = 26;
pub const CURVECAPS: ::c_int = 28;
pub const LINECAPS: ::c_int = 30;
pub const POLYGONALCAPS: ::c_int = 32;
pub const TEXTCAPS: ::c_int = 34;
pub const CLIPCAPS: ::c_int = 36;
pub const RASTERCAPS: ::c_int = 38;
pub const ASPECTX: ::c_int = 40;
pub const ASPECTY: ::c_int = 42;
pub const ASPECTXY: ::c_int = 44;
pub const LOGPIXELSX: ::c_int = 88;
pub const LOGPIXELSY: ::c_int = 90;
pub const SIZEPALETTE: ::c_int = 104;
pub const NUMRESERVED: ::c_int = 106;
pub const COLORRES: ::c_int = 108;
pub const PHYSICALWIDTH: ::c_int = 110;
pub const PHYSICALHEIGHT: ::c_int = 111;
pub const PHYSICALOFFSETX: ::c_int = 112;
pub const PHYSICALOFFSETY: ::c_int = 113;
pub const SCALINGFACTORX: ::c_int = 114;
pub const SCALINGFACTORY: ::c_int = 115;
pub const VREFRESH: ::c_int = 116;
pub const DESKTOPVERTRES: ::c_int = 117;
pub const DESKTOPHORZRES: ::c_int = 118;
pub const BLTALIGNMENT: ::c_int = 119;
pub const SHADEBLENDCAPS: ::c_int = 120;
pub const COLORMGMTCAPS: ::c_int = 121;
//1906
pub const DIB_RGB_COLORS: ::UINT = 0;
pub const DIB_PAL_COLORS: ::UINT = 1;
pub const CBM_INIT: ::DWORD = 4;
STRUCT!{struct RGNDATAHEADER {
    dwSize: ::DWORD,
    iType: ::DWORD,
    nCount: ::DWORD,
    nRgnSize: ::DWORD,
    rcBound: ::RECT,
}}
pub type PRGNDATAHEADER = *mut RGNDATAHEADER;
STRUCT!{nodebug struct RGNDATA {
    rdh: RGNDATAHEADER,
    Buffer: [::c_char; 0],
}}
pub type PRGNDATA = *mut RGNDATA;
pub type NPRGNDATA = *mut RGNDATA;
pub type LPRGNDATA = *mut RGNDATA;
STRUCT!{struct PALETTEENTRY {
    peRed: ::BYTE,
    peGreen: ::BYTE,
    peBlue: ::BYTE,
    peFlags: ::BYTE,
}}
pub type PPALETTEENTRY = *mut PALETTEENTRY;
pub type LPPALETTEENTRY = *mut PALETTEENTRY;
//2824 (Win 7 SDK)
STRUCT!{struct ABC {
    abcA: ::c_int,
    abcB: ::UINT,
    abcC: ::c_int,
}}
pub type PABC = *mut ABC;
pub type NPABC = *mut ABC;
pub type LPABC = *mut ABC;
STRUCT!{struct ABCFLOAT {
    abcfA: ::FLOAT,
    abcfB: ::FLOAT,
    abcfC: ::FLOAT,
}}
pub type PABCFLOAT = *mut ABCFLOAT;
pub type NPABCFLOAT = *mut ABCFLOAT;
pub type LPABCFLOAT = *mut ABCFLOAT;
//3581
pub type LINEDDAPROC = Option<unsafe extern "system" fn(::c_int, ::c_int, ::LPARAM)>;
STRUCT!{struct XFORM {
    eM11: ::FLOAT,
    eM12: ::FLOAT,
    eM21: ::FLOAT,
    eM22: ::FLOAT,
    eDx: ::FLOAT,
    eDy: ::FLOAT,
}}
pub type PXFORM = *mut XFORM;
pub type LPXFORM = *mut XFORM;
STRUCT!{struct LOGBRUSH {
    lbStyle: ::UINT,
    lbColor: ::COLORREF,
    lbHatch: ::ULONG_PTR,
}}
pub type PLOGBRUSH = *mut LOGBRUSH;
STRUCT!{nodebug struct LOGCOLORSPACEA {
    lcsSignature: ::DWORD,
    lcsVersion: ::DWORD,
    lcsSize: ::DWORD,
    lcsCSType: LCSCSTYPE,
    lcsIntent: LCSGAMUTMATCH,
    lcsEndpoints: CIEXYZTRIPLE,
    lcsGammaRed: ::DWORD,
    lcsGammaGreen: ::DWORD,
    lcsGammaBlue: ::DWORD,
    lcsFilename: [::CHAR; ::MAX_PATH],
}}
pub type LPLOGCOLORSPACEA = *mut LOGCOLORSPACEA;
STRUCT!{nodebug struct LOGCOLORSPACEW {
    lcsSignature: ::DWORD,
    lcsVersion: ::DWORD,
    lcsSize: ::DWORD,
    lcsCSType: LCSCSTYPE,
    lcsIntent: LCSGAMUTMATCH,
    lcsEndpoints: CIEXYZTRIPLE,
    lcsGammaRed: ::DWORD,
    lcsGammaGreen: ::DWORD,
    lcsGammaBlue: ::DWORD,
    lcsFilename: [::WCHAR; ::MAX_PATH],
}}
pub type LPLOGCOLORSPACEW = *mut LOGCOLORSPACEW;
pub const LF_FULLFACESIZE: usize = 64;
STRUCT!{nodebug struct ENUMLOGFONTEXA {
    elfLogFont: LOGFONTA,
    elfFullName: [::BYTE; LF_FULLFACESIZE],
    elfStyle: [::BYTE; LF_FACESIZE],
    elfScript: [::BYTE; LF_FACESIZE],
}}
pub type LPENUMLOGFONTEXA = *mut ENUMLOGFONTEXA;
STRUCT!{nodebug struct ENUMLOGFONTEXW {
    elfLogFont: LOGFONTW,
    elfFullName: [::WCHAR; LF_FULLFACESIZE],
    elfStyle: [::WCHAR; LF_FACESIZE],
    elfScript: [::WCHAR; LF_FACESIZE],
}}
pub type LPENUMLOGFONTEXW = *mut ENUMLOGFONTEXW;
pub const MM_MAX_NUMAXES: usize = 16;
STRUCT!{struct DESIGNVECTOR {
    dvReserved: ::DWORD,
    dvNumAxes: ::DWORD,
    dvValues: [::LONG; MM_MAX_NUMAXES],
}}
pub type PDESIGNVECTOR = *mut DESIGNVECTOR;
pub type LPDESIGNVECTOR = *mut DESIGNVECTOR;
STRUCT!{nodebug struct ENUMLOGFONTEXDVA {
    elfEnumLogfontEx: ENUMLOGFONTEXA,
    elfDesignVector: DESIGNVECTOR,
}}
pub type PENUMLOGFONTEXDVA = *mut ENUMLOGFONTEXDVA;
pub type LPENUMLOGFONTEXDVA = *mut ENUMLOGFONTEXDVA;
STRUCT!{nodebug struct ENUMLOGFONTEXDVW {
    elfEnumLogfontEx: ENUMLOGFONTEXW,
    elfDesignVector: DESIGNVECTOR,
}}
pub type PENUMLOGFONTEXDVW = *mut ENUMLOGFONTEXDVW;
pub type LPENUMLOGFONTEXDVW = *mut ENUMLOGFONTEXDVW;
STRUCT!{struct LOGPALETTE {
    palVersion: ::WORD,
    palNumEntries: ::WORD,
    palPalEntry: [PALETTEENTRY; 1],
}}
pub type PLOGPALETTE = *mut LOGPALETTE;
pub type NPLOGPALETTE = *mut LOGPALETTE;
pub type LPLOGPALETTE = *mut LOGPALETTE;
STRUCT!{struct LOGPEN {
    lopnStyle: ::UINT,
    lopnWidth: ::POINT,
    lopnColor: ::COLORREF,
}}
pub type PLOGPEN = *mut LOGPEN;
pub type NPLOGPEN = *mut LOGPEN;
pub type LPLOGPEN = *mut LOGPEN;
STRUCT!{struct BLENDFUNCTION {
    BlendOp: ::BYTE,
    BlendFlags: ::BYTE,
    SourceConstantAlpha: ::BYTE,
    AlphaFormat: ::BYTE,
}}
pub type PBLENDFUNCTION = *mut BLENDFUNCTION;
pub const TMPF_FIXED_PITCH: ::BYTE = 0x01;
pub const TMPF_VECTOR: ::BYTE = 0x02;
pub const TMPF_DEVICE: ::BYTE = 0x08;
pub const TMPF_TRUETYPE: ::BYTE = 0x04;
STRUCT!{struct TEXTMETRICA {
    tmHeight: ::LONG,
    tmAscent: ::LONG,
    tmDescent: ::LONG,
    tmInternalLeading: ::LONG,
    tmExternalLeading: ::LONG,
    tmAveCharWidth: ::LONG,
    tmMaxCharWidth: ::LONG,
    tmWeight: ::LONG,
    tmOverhang: ::LONG,
    tmDigitizedAspectX: ::LONG,
    tmDigitizedAspectY: ::LONG,
    tmFirstChar: ::BYTE,
    tmLastChar: ::BYTE,
    tmDefaultChar: ::BYTE,
    tmBreakChar: ::BYTE,
    tmItalic: ::BYTE,
    tmUnderlined: ::BYTE,
    tmStruckOut: ::BYTE,
    tmPitchAndFamily: ::BYTE,
    tmCharSet: ::BYTE,
}}
pub type PTEXTMETRICA = *mut TEXTMETRICA;
pub type NPTEXTMETRICA = *mut TEXTMETRICA;
pub type LPTEXTMETRICA = *mut TEXTMETRICA;
STRUCT!{struct TEXTMETRICW {
    tmHeight: ::LONG,
    tmAscent: ::LONG,
    tmDescent: ::LONG,
    tmInternalLeading: ::LONG,
    tmExternalLeading: ::LONG,
    tmAveCharWidth: ::LONG,
    tmMaxCharWidth: ::LONG,
    tmWeight: ::LONG,
    tmOverhang: ::LONG,
    tmDigitizedAspectX: ::LONG,
    tmDigitizedAspectY: ::LONG,
    tmFirstChar: ::WCHAR,
    tmLastChar: ::WCHAR,
    tmDefaultChar: ::WCHAR,
    tmBreakChar: ::WCHAR,
    tmItalic: ::BYTE,
    tmUnderlined: ::BYTE,
    tmStruckOut: ::BYTE,
    tmPitchAndFamily: ::BYTE,
    tmCharSet: ::BYTE,
}}
pub type PTEXTMETRICW = *mut TEXTMETRICW;
pub type NPTEXTMETRICW = *mut TEXTMETRICW;
pub type LPTEXTMETRICW = *mut TEXTMETRICW;
pub const TA_NOUPDATECP: ::UINT = 0;
pub const TA_UPDATECP: ::UINT = 1;
pub const TA_LEFT: ::UINT = 0;
pub const TA_RIGHT: ::UINT = 2;
pub const TA_CENTER: ::UINT = 6;
pub const TA_TOP: ::UINT = 0;
pub const TA_BOTTOM: ::UINT = 8;
pub const TA_BASELINE: ::UINT = 24;
pub const TA_RTLREADING: ::UINT = 256;
pub const TA_MASK: ::UINT = TA_BASELINE + TA_CENTER + TA_UPDATECP + TA_RTLREADING;
pub const WHITE_BRUSH: ::c_int = 0;
pub const LTGRAY_BRUSH: ::c_int = 1;
pub const GRAY_BRUSH: ::c_int = 2;
pub const DKGRAY_BRUSH: ::c_int = 3;
pub const BLACK_BRUSH: ::c_int = 4;
pub const NULL_BRUSH: ::c_int = 5;
pub const HOLLOW_BRUSH: ::c_int = 5;
pub const WHITE_PEN: ::c_int = 6;
pub const BLACK_PEN: ::c_int = 7;
pub const NULL_PEN: ::c_int = 8;
pub const OEM_FIXED_FONT: ::c_int = 10;
pub const ANSI_FIXED_FONT: ::c_int = 11;
pub const ANSI_VAR_FONT: ::c_int = 12;
pub const SYSTEM_FONT: ::c_int = 13;
pub const DEVICE_DEFAULT_FONT: ::c_int = 14;
pub const DEFAULT_PALETTE: ::c_int = 15;
pub const SYSTEM_FIXED_FONT: ::c_int = 16;
pub const DEFAULT_GUI_FONT: ::c_int = 17;
pub const DC_BRUSH: ::c_int = 18;
pub const DC_PEN: ::c_int = 19;
pub const STOCK_LAST: ::c_int = 19;pub const PS_SOLID: ::c_int = 0;
pub const PS_DASH: ::c_int = 1;
pub const PS_DOT: ::c_int = 2;
pub const PS_DASHDOT: ::c_int = 3;
pub const PS_DASHDOTDOT: ::c_int = 4;
pub const PS_NULL: ::c_int = 5;
pub const PS_INSIDEFRAME: ::c_int = 6;
pub const PS_USERSTYLE: ::c_int = 7;
pub const PS_ALTERNATE: ::c_int = 8;
pub const TRANSPARENT: ::c_int = 1;
pub const OPAQUE: ::c_int = 2;
pub const BKMODE_LAST: ::c_int = 2;
pub const MM_TEXT: ::c_int = 1;
pub const MM_LOMETRIC: ::c_int = 2;
pub const MM_HIMETRIC: ::c_int = 3;
pub const MM_LOENGLISH: ::c_int = 4;
pub const MM_HIENGLISH: ::c_int = 5;
pub const MM_TWIPS: ::c_int = 6;
pub const MM_ISOTROPIC: ::c_int = 7;
pub const MM_ANISOTROPIC: ::c_int = 8;
pub const ALTERNATE: ::c_int = 1;
pub const WINDING: ::c_int = 2;
pub const POLYFILL_LAST: ::c_int = 2;
pub const OUT_DEFAULT_PRECIS: ::DWORD = 0;
pub const OUT_STRING_PRECIS: ::DWORD = 1;
pub const OUT_CHARACTER_PRECIS: ::DWORD = 2;
pub const OUT_STROKE_PRECIS: ::DWORD = 3;
pub const OUT_TT_PRECIS: ::DWORD = 4;
pub const OUT_DEVICE_PRECIS: ::DWORD = 5;
pub const OUT_RASTER_PRECIS: ::DWORD = 6;
pub const OUT_TT_ONLY_PRECIS: ::DWORD = 7;
pub const OUT_OUTLINE_PRECIS: ::DWORD = 8;
pub const OUT_SCREEN_OUTLINE_PRECIS: ::DWORD = 9;
pub const OUT_PS_ONLY_PRECIS: ::DWORD = 10;
pub const CLIP_DEFAULT_PRECIS: ::DWORD = 0;
pub const CLIP_CHARACTER_PRECIS: ::DWORD = 1;
pub const CLIP_STROKE_PRECIS: ::DWORD = 2;
pub const CLIP_MASK: ::DWORD = 0xf;
pub const CLIP_LH_ANGLES: ::DWORD = 1 << 4;
pub const CLIP_TT_ALWAYS: ::DWORD = 2 << 4;
pub const CLIP_DFA_DISABLE: ::DWORD = 4 << 4;
pub const CLIP_EMBEDDED: ::DWORD = 8 << 4;
pub const DEFAULT_QUALITY: ::DWORD = 0;
pub const DRAFT_QUALITY: ::DWORD = 1;
pub const PROOF_QUALITY: ::DWORD = 2;
pub const NONANTIALIASED_QUALITY: ::DWORD = 3;
pub const ANTIALIASED_QUALITY: ::DWORD = 4;
pub const CLEARTYPE_QUALITY: ::DWORD = 5;
pub const CLEARTYPE_NATURAL_QUALITY: ::DWORD = 6;
pub const DEFAULT_PITCH: ::DWORD = 0;
pub const FIXED_PITCH: ::DWORD = 1;
pub const VARIABLE_PITCH: ::DWORD = 2;
pub const FF_DONTCARE: ::DWORD = 0 << 4;
pub const FF_ROMAN: ::DWORD = 1 << 4;
pub const FF_SWISS: ::DWORD = 2 << 4;
pub const FF_MODERN: ::DWORD = 3 << 4;
pub const FF_SCRIPT: ::DWORD = 4 << 4;
pub const FF_DECORATIVE: ::DWORD = 5 << 4;
pub const MONO_FONT: ::DWORD = 8;
pub const ANSI_CHARSET: ::DWORD = 0;
pub const DEFAULT_CHARSET: ::DWORD = 1;
pub const SYMBOL_CHARSET: ::DWORD = 2;
pub const SHIFTJIS_CHARSET: ::DWORD = 128;
pub const HANGEUL_CHARSET: ::DWORD = 129;
pub const HANGUL_CHARSET: ::DWORD = 129;
pub const GB2312_CHARSET: ::DWORD = 134;
pub const CHINESEBIG5_CHARSET: ::DWORD = 136;
pub const OEM_CHARSET: ::DWORD = 255;
pub const JOHAB_CHARSET: ::DWORD = 130;
pub const HEBREW_CHARSET: ::DWORD = 177;
pub const ARABIC_CHARSET: ::DWORD = 178;
pub const GREEK_CHARSET: ::DWORD = 161;
pub const TURKISH_CHARSET: ::DWORD = 162;
pub const VIETNAMESE_CHARSET: ::DWORD = 163;
pub const THAI_CHARSET: ::DWORD = 222;
pub const EASTEUROPE_CHARSET: ::DWORD = 238;
pub const RUSSIAN_CHARSET: ::DWORD = 204;
pub const MAC_CHARSET: ::DWORD = 77;
pub const BALTIC_CHARSET: ::DWORD = 186;
pub const FS_LATIN1: ::DWORD = 0x00000001;
pub const FS_LATIN2: ::DWORD = 0x00000002;
pub const FS_CYRILLIC: ::DWORD = 0x00000004;
pub const FS_GREEK: ::DWORD = 0x00000008;
pub const FS_TURKISH: ::DWORD = 0x00000010;
pub const FS_HEBREW: ::DWORD = 0x00000020;
pub const FS_ARABIC: ::DWORD = 0x00000040;
pub const FS_BALTIC: ::DWORD = 0x00000080;
pub const FS_VIETNAMESE: ::DWORD = 0x00000100;
pub const FS_THAI: ::DWORD = 0x00010000;
pub const FS_JISJAPAN: ::DWORD = 0x00020000;
pub const FS_CHINESESIMP: ::DWORD = 0x00040000;
pub const FS_WANSUNG: ::DWORD = 0x00080000;
pub const FS_CHINESETRAD: ::DWORD = 0x00100000;
pub const FS_JOHAB: ::DWORD = 0x00200000;
pub const FS_SYMBOL: ::DWORD = 0x80000000;
pub const FW_DONTCARE: ::c_int = 0;
pub const FW_THIN: ::c_int = 100;
pub const FW_EXTRALIGHT: ::c_int = 200;
pub const FW_LIGHT: ::c_int = 300;
pub const FW_NORMAL: ::c_int = 400;
pub const FW_MEDIUM: ::c_int = 500;
pub const FW_SEMIBOLD: ::c_int = 600;
pub const FW_BOLD: ::c_int = 700;
pub const FW_EXTRABOLD: ::c_int = 800;
pub const FW_HEAVY: ::c_int = 900;
pub const FW_ULTRALIGHT: ::c_int = FW_EXTRALIGHT;
pub const FW_REGULAR: ::c_int = FW_NORMAL;
pub const FW_DEMIBOLD: ::c_int = FW_SEMIBOLD;
pub const FW_ULTRABOLD: ::c_int = FW_EXTRABOLD;
pub const FW_BLACK: ::c_int = FW_HEAVY;
pub type COLOR16 = ::c_ushort;
STRUCT!{struct TRIVERTEX {
    x: ::LONG,
    y: ::LONG,
    Red: COLOR16,
    Green: COLOR16,
    Blue: COLOR16,
    Alpha: COLOR16,
}}
pub type PTRIVERTEX = *mut TRIVERTEX;
pub type LPTRIVERTEX = *mut TRIVERTEX;
STRUCT!{struct GRADIENT_RECT {
    UpperLeft: ::ULONG,
    LowerRight: ::ULONG,
}}
pub type PGRADIENT_RECT = *mut GRADIENT_RECT;
pub type LPGRADIENT_RECT = *mut GRADIENT_RECT;
/* Object Definitions for EnumObjects() */
pub const OBJ_PEN: ::UINT = 1;
pub const OBJ_BRUSH: ::UINT = 2;
pub const OBJ_DC: ::UINT = 3;
pub const OBJ_METADC: ::UINT = 4;
pub const OBJ_PAL: ::UINT = 5;
pub const OBJ_FONT: ::UINT = 6;
pub const OBJ_BITMAP: ::UINT = 7;
pub const OBJ_REGION: ::UINT = 8;
pub const OBJ_METAFILE: ::UINT = 9;
pub const OBJ_MEMDC: ::UINT = 10;
pub const OBJ_EXTPEN: ::UINT = 11;
pub const OBJ_ENHMETADC: ::UINT = 12;
pub const OBJ_ENHMETAFILE: ::UINT = 13;
pub const OBJ_COLORSPACE: ::UINT = 14;
pub const GDI_OBJ_LAST: ::UINT = OBJ_COLORSPACE;
STRUCT!{struct COLORADJUSTMENT {
    caSize: ::WORD,
    caFlags: ::WORD,
    caIlluminantIndex: ::WORD,
    caRedGamma: ::WORD,
    caGreenGamma: ::WORD,
    caBlueGamma: ::WORD,
    caReferenceBlack: ::WORD,
    caReferenceWhite: ::WORD,
    caContrast: ::SHORT,
    caBrightness: ::SHORT,
    caColorfulness: ::SHORT,
    caRedGreenTint: ::SHORT,
}}
pub type PCOLORADJUSTMENT = *mut COLORADJUSTMENT;
pub type LPCOLORADJUSTMENT = *mut COLORADJUSTMENT;
pub type OLDFONTENUMPROCA = Option<unsafe extern "system" fn(
    *const LOGFONTA, *const ::VOID, ::DWORD, ::LPARAM
) -> ::c_int>;
pub type OLDFONTENUMPROCW = Option<unsafe extern "system" fn(
    *const LOGFONTW, *const ::VOID, ::DWORD, ::LPARAM
) -> ::c_int>;
pub type FONTENUMPROCA = OLDFONTENUMPROCA;
pub type FONTENUMPROCW = OLDFONTENUMPROCW;
STRUCT!{struct WCRANGE {
    wcLow: ::WCHAR,
    cGlyphs: ::USHORT,
}}
pub type PWCRANGE = *mut WCRANGE;
pub type LPWCRANGE = *mut WCRANGE;
STRUCT!{struct GLYPHSET {
    cbThis: ::DWORD,
    flAccel: ::DWORD,
    cGlyphsSupported: ::DWORD,
    cRanges: ::DWORD,
    ranges: [WCRANGE;1],
}}
pub type PGLYPHSET = *mut GLYPHSET;
pub type LPGLYPHSET = *mut GLYPHSET;
pub type ABORTPROC = Option<unsafe extern "system" fn(::HDC, ::c_int) -> ::BOOL>;
STRUCT!{struct DOCINFOA {
    cbSize: ::c_int,
    lpszDocName: ::LPCSTR,
    lpszOutput: ::LPCSTR,
    lpszDatatype: ::LPCSTR,
    fwType: ::DWORD,
}}
pub type LPDOCINFOA = *mut DOCINFOA;
STRUCT!{struct DOCINFOW {
    cbSize: ::c_int,
    lpszDocName: ::LPCWSTR,
    lpszOutput: ::LPCWSTR,
    lpszDatatype: ::LPCWSTR,
    fwType: ::DWORD,
}}
pub type LPDOCINFOW = *mut DOCINFOW;
pub type ICMENUMPROCA = Option<unsafe extern "system" fn(::LPSTR, ::LPARAM) -> ::c_int>;
pub type ICMENUMPROCW = Option<unsafe extern "system" fn(::LPWSTR, ::LPARAM) -> ::c_int>;
STRUCT!{struct HANDLETABLE {
    objectHandle: [::HGDIOBJ; 1],
}}
pub type LPHANDLETABLE = *mut HANDLETABLE;
pub type PHANDLETABLE = *mut HANDLETABLE;
STRUCT!{struct METARECORD {
    rdSize: ::DWORD,
    rdFunction: ::WORD,
    rdParm: [::WORD; 1],
}}
pub type PMETARECORD = *mut METARECORD;
pub type LPMETARECORD = *mut METARECORD;
pub type MFENUMPROC = Option<unsafe extern "system" fn(
    hdc: ::HDC, lpht: *mut ::HANDLETABLE, lpMR: *mut ::METARECORD, nObj: ::c_int, param: ::LPARAM
) -> ::c_int>;
pub type GOBJENUMPROC = Option<unsafe extern "system" fn(::LPVOID, ::LPARAM) -> ::c_int>;
STRUCT!{struct GCP_RESULTSA {
    lStructSize: ::DWORD,
    lpOutString: ::LPSTR,
    lpOrder: *const ::UINT,
    lpDx: *const ::c_int,
    lpCaretPos: *const ::c_int,
    lpClass: ::LPSTR,
    lpGlyphs: ::LPWSTR,
    nGlyphs: ::UINT,
    nMaxFit: ::c_int,
}}
pub type LPGCP_RESULTSA = *mut GCP_RESULTSA;
STRUCT!{struct GCP_RESULTSW {
    lStructSize: ::DWORD,
    lpOutString: ::LPWSTR,
    lpOrder: *const ::UINT,
    lpDx: *const ::c_int,
    lpCaretPos: *const ::c_int,
    lpClass: ::LPSTR,
    lpGlyphs: ::LPWSTR,
    nGlyphs: ::UINT,
    nMaxFit: ::c_int,
}}
pub type LPGCP_RESULTSW = *mut GCP_RESULTSW;
STRUCT!{struct FONTSIGNATURE {
    fsUsb: [::DWORD; 4],
    fsCsb: [::DWORD; 2],
}}
pub type LPFONTSIGNATURE = *mut FONTSIGNATURE;
pub type PFONTSIGNATURE = *mut FONTSIGNATURE;
STRUCT!{struct POLYTEXTA {
    x: ::c_int,
    y: ::c_int,
    n: ::UINT,
    lpstr: ::LPCSTR,
    uiFlags: ::UINT,
    rcl: ::RECT,
    pdx: *const ::c_int,
}}
pub type PPOLYTEXTA = *mut POLYTEXTA;
pub type NPPOLYTEXTA = *mut POLYTEXTA;
pub type LPPOLYTEXTA = *mut POLYTEXTA;
STRUCT!{struct POLYTEXTW {
    x: ::c_int,
    y: ::c_int,
    n: ::UINT,
    lpstr: ::LPCWSTR,
    uiFlags: ::UINT,
    rcl: ::RECT,
    pdx: *const ::c_int,
}}
pub type PPOLYTEXTW = *mut POLYTEXTW;
pub type NPPOLYTEXTW = *mut POLYTEXTW;
pub type LPPOLYTEXTW = *mut POLYTEXTW;
STRUCT!{struct CHARSETINFO {
    ciCharset: ::UINT,
    ciACP: ::UINT,
    fs: ::FONTSIGNATURE,
}}
pub type PCHARSETINFO = *mut CHARSETINFO;
pub type NPCHARSETINFO = *mut CHARSETINFO;
pub type LPCHARSETINFO = *mut CHARSETINFO;
pub const GRADIENT_FILL_RECT_H: ::ULONG = 0x00000000;
pub const GRADIENT_FILL_RECT_V: ::ULONG = 0x00000001;
pub const GRADIENT_FILL_TRIANGLE: ::ULONG = 0x00000002;
pub const GRADIENT_FILL_OP_FLAG: ::ULONG = 0x000000ff;
STRUCT!{struct LAYERPLANEDESCRIPTOR {
    nSize: ::WORD,
    nVersion: ::WORD,
    dwFlags: ::DWORD,
    iPixelType: ::BYTE,
    cColorBits: ::BYTE,
    cRedBits: ::BYTE,
    cRedShift: ::BYTE,
    cGreenBits: ::BYTE,
    cGreenShift: ::BYTE,
    cBlueBits: ::BYTE,
    cBlueShift: ::BYTE,
    cAlphaBits: ::BYTE,
    cAlphaShift: ::BYTE,
    cAccumBits: ::BYTE,
    cAccumRedBits: ::BYTE,
    cAccumGreenBits: ::BYTE,
    cAccumBlueBits: ::BYTE,
    cAccumAlphaBits: ::BYTE,
    cDepthBits: ::BYTE,
    cStencilBits: ::BYTE,
    cAuxBuffers: ::BYTE,
    iLayerPlane: ::BYTE,
    bReserved: ::BYTE,
    crTransparent: ::COLORREF,
}}
pub type PLAYERPLANEDESCRIPTOR = *mut LAYERPLANEDESCRIPTOR;
pub type LPLAYERPLANEDESCRIPTOR = *mut LAYERPLANEDESCRIPTOR;
STRUCT!{struct ENHMETAHEADER {
    iType: ::DWORD,
    nSize: ::DWORD,
    rclBounds: ::RECTL,
    rclFrame: ::RECTL,
    dSignature: ::DWORD,
    nVersion: ::DWORD,
    nBytes: ::DWORD,
    nRecords: ::DWORD,
    nHandles: ::WORD,
    sReserved: ::WORD,
    nDescription: ::DWORD,
    offDescription: ::DWORD,
    nPalEntries: ::DWORD,
    szlDevice: ::SIZEL,
    szlMillimeters: ::SIZEL,
    cbPixelFormat: ::DWORD,
    offPixelFormat: ::DWORD,
    bOpenGL: ::DWORD,
    szlMicrometers: ::SIZEL,
}}
pub type PENHMETAHEADER = *mut ENHMETAHEADER;
pub type LPENHMETAHEADER = *mut ENHMETAHEADER;
STRUCT!{struct FIXED {
    fract: ::WORD,
    value: ::c_short,
}}
STRUCT!{struct MAT2 {
    eM11: FIXED,
    eM12: FIXED,
    eM21: FIXED,
    eM22: FIXED,
}}
pub type LPMAT2 = *mut MAT2;
STRUCT!{struct GLYPHMETRICS {
    gmBlackBoxX: ::UINT,
    gmBlackBoxY: ::UINT,
    gmptGlyphOrigin: ::POINT,
    gmCellIncX: ::c_short,
    gmCellIncY: ::c_short,
}}
pub type LPGLYPHMETRICS = *mut GLYPHMETRICS;
STRUCT!{struct KERNINGPAIR {
     wFirst: ::WORD,
     wSecond: ::WORD,
     iKernAmount: ::c_int,
}}
pub type LPKERNINGPAIR = *mut KERNINGPAIR;
STRUCT!{struct PANOSE {
    bFamilyType: ::BYTE,
    bSerifStyle: ::BYTE,
    bWeight: ::BYTE,
    bProportion: ::BYTE,
    bContrast: ::BYTE,
    bStrokeVariation: ::BYTE,
    bArmStyle: ::BYTE,
    bLetterform: ::BYTE,
    bMidline: ::BYTE,
    bXHeight: ::BYTE,
}}
pub type LPPANOSE = *mut PANOSE;
STRUCT!{struct OUTLINETEXTMETRICA {
    otmSize: ::UINT,
    otmTextMetrics: TEXTMETRICA,
    otmFiller: ::BYTE,
    otmPanoseNumber: ::PANOSE,
    otmfsSelection: ::UINT,
    otmfsType: ::UINT,
    otmsCharSlopeRise: ::c_int,
    otmsCharSlopeRun: ::c_int,
    otmItalicAngle: ::c_int,
    otmEMSquare: ::UINT,
    otmAscent: ::c_int,
    otmDescent: ::c_int,
    otmLineGap: ::UINT,
    otmsCapEmHeight: ::UINT,
    otmsXHeight: ::UINT,
    otmrcFontBox: ::RECT,
    otmMacAscent: ::c_int,
    otmMacDescent: ::c_int,
    otmMacLineGap: ::UINT,
    otmusMinimumPPEM: ::UINT,
    otmptSubscriptSize: ::POINT,
    otmptSubscriptOffset: ::POINT,
    otmptSuperscriptSize: ::POINT,
    otmptSuperscriptOffset: ::POINT,
    otmsStrikeoutSize: ::UINT,
    otmsStrikeoutPosition: ::c_int,
    otmsUnderscoreSize: ::c_int,
    otmsUnderscorePosition: ::c_int,
    otmpFamilyName: ::PSTR,
    otmpFaceName: ::PSTR,
    otmpStyleName: ::PSTR,
    otmpFullName: ::PSTR,
}}
pub type POUTLINETEXTMETRICA = *mut OUTLINETEXTMETRICA;
pub type NPOUTLINETEXTMETRICA = *mut OUTLINETEXTMETRICA;
pub type LPOUTLINETEXTMETRICA = *mut OUTLINETEXTMETRICA;
STRUCT!{struct OUTLINETEXTMETRICW {
    otmSize: ::UINT,
    otmTextMetrics: TEXTMETRICW,
    otmFiller: ::BYTE,
    otmPanoseNumber: ::PANOSE,
    otmfsSelection: ::UINT,
    otmfsType: ::UINT,
    otmsCharSlopeRise: ::c_int,
    otmsCharSlopeRun: ::c_int,
    otmItalicAngle: ::c_int,
    otmEMSquare: ::UINT,
    otmAscent: ::c_int,
    otmDescent: ::c_int,
    otmLineGap: ::UINT,
    otmsCapEmHeight: ::UINT,
    otmsXHeight: ::UINT,
    otmrcFontBox: ::RECT,
    otmMacAscent: ::c_int,
    otmMacDescent: ::c_int,
    otmMacLineGap: ::UINT,
    otmusMinimumPPEM: ::UINT,
    otmptSubscriptSize: ::POINT,
    otmptSubscriptOffset: ::POINT,
    otmptSuperscriptSize: ::POINT,
    otmptSuperscriptOffset: ::POINT,
    otmsStrikeoutSize: ::UINT,
    otmsStrikeoutPosition: ::c_int,
    otmsUnderscoreSize: ::c_int,
    otmsUnderscorePosition: ::c_int,
    otmpFamilyName: ::PSTR,
    otmpFaceName: ::PSTR,
    otmpStyleName: ::PSTR,
    otmpFullName: ::PSTR,
}}
pub type POUTLINETEXTMETRICW = *mut OUTLINETEXTMETRICW;
pub type NPOUTLINETEXTMETRICW = *mut OUTLINETEXTMETRICW;
pub type LPOUTLINETEXTMETRICW = *mut OUTLINETEXTMETRICW;
STRUCT!{struct RASTERIZER_STATUS {
    nSize: ::c_short,
    wFlags: ::c_short,
    nLanguageID: ::c_short,
}}
pub type LPRASTERIZER_STATUS = *mut RASTERIZER_STATUS;
STRUCT!{struct ENHMETARECORD {
    iType: ::DWORD,
    nSize: ::DWORD,
    dParm: [::DWORD; 1],
}}
pub type PENHMETARECORD = *mut ENHMETARECORD;
pub type LPENHMETARECORD = *mut ENHMETARECORD;
STRUCT!{struct METAFILEPICT {
    mm: ::LONG,
    xExt: ::LONG,
    yExt: ::LONG,
    hMF: ::HMETAFILE,
}}
pub type LPMETAFILEPICT = *mut METAFILEPICT;
STRUCT!{struct POINTFLOAT {
    x: ::FLOAT,
    y: ::FLOAT,
}}
pub type PPOINTFLOAT = *mut POINTFLOAT;
STRUCT!{struct GLYPHMETRICSFLOAT {
    gmfBlackBoxX: ::FLOAT,
    gmfBlackBoxY: ::FLOAT,
    gmfptGlyphOrigin: POINTFLOAT,
    gmfCellIncX: ::FLOAT,
    gmfCellIncY: ::FLOAT,
}}
pub type PGLYPHMETRICSFLOAT = *mut GLYPHMETRICSFLOAT;
pub type LPGLYPHMETRICSFLOAT = *mut GLYPHMETRICSFLOAT;
pub const DT_PLOTTER: ::c_int = 0;
pub const DT_RASDISPLAY: ::c_int = 1;
pub const DT_RASPRINTER: ::c_int = 2;
pub const DT_RASCAMERA: ::c_int = 3;
pub const DT_CHARSTREAM: ::c_int = 4;
pub const DT_METAFILE: ::c_int = 5;
pub const DT_DISPFILE: ::c_int = 6;
pub const CLR_INVALID: ::COLORREF = 0xFFFFFFFF;
pub const ETO_OPAQUE: ::UINT = 0x0002;
pub const ETO_CLIPPED: ::UINT = 0x0004;
pub const ETO_GLYPH_INDEX: ::UINT = 0x0010;
pub const ETO_RTLREADING: ::UINT = 0x0080;
pub const ETO_NUMERICSLOCAL: ::UINT = 0x0400;
pub const ETO_NUMERICSLATIN: ::UINT = 0x0800;
pub const ETO_IGNORELANGUAGE: ::UINT = 0x1000;
pub const ETO_PDY: ::UINT = 0x2000;
pub const ETO_REVERSE_INDEX_MAP: ::UINT = 0x10000;
STRUCT!{struct EXTLOGPEN {
    elpPenStyle: ::DWORD,
    elpWidth: ::DWORD,
    elpBrushStyle: ::UINT,
    elpColor: ::COLORREF,
    elpHatch: ::ULONG_PTR,
    elpNumEntries: ::DWORD,
    elpStyleEntry: [::DWORD; 1],
}}
pub type PEXTLOGPEN = *mut EXTLOGPEN;
pub type NPEXTLOGPEN = *mut EXTLOGPEN;
pub type LPEXTLOGPEN = *mut EXTLOGPEN;
pub type ENHMFENUMPROC = Option<unsafe extern "system" fn(
    hdc: ::HDC, lpht: HANDLETABLE, lpmr: *const ENHMETARECORD, nHandles: ::c_int, data: ::LPARAM
) -> ::c_int>;
/* Metafile Functions */
pub const META_SETBKCOLOR: ::WORD = 0x0201;
pub const META_SETBKMODE: ::WORD = 0x0102;
pub const META_SETMAPMODE: ::WORD = 0x0103;
pub const META_SETROP2: ::WORD = 0x0104;
pub const META_SETRELABS: ::WORD = 0x0105;
pub const META_SETPOLYFILLMODE: ::WORD = 0x0106;
pub const META_SETSTRETCHBLTMODE: ::WORD = 0x0107;
pub const META_SETTEXTCHAREXTRA: ::WORD = 0x0108;
pub const META_SETTEXTCOLOR: ::WORD = 0x0209;
pub const META_SETTEXTJUSTIFICATION: ::WORD = 0x020A;
pub const META_SETWINDOWORG: ::WORD = 0x020B;
pub const META_SETWINDOWEXT: ::WORD = 0x020C;
pub const META_SETVIEWPORTORG: ::WORD = 0x020D;
pub const META_SETVIEWPORTEXT: ::WORD = 0x020E;
pub const META_OFFSETWINDOWORG: ::WORD = 0x020F;
pub const META_SCALEWINDOWEXT: ::WORD = 0x0410;
pub const META_OFFSETVIEWPORTORG: ::WORD = 0x0211;
pub const META_SCALEVIEWPORTEXT: ::WORD = 0x0412;
pub const META_LINETO: ::WORD = 0x0213;
pub const META_MOVETO: ::WORD = 0x0214;
pub const META_EXCLUDECLIPRECT: ::WORD = 0x0415;
pub const META_INTERSECTCLIPRECT: ::WORD = 0x0416;
pub const META_ARC: ::WORD = 0x0817;
pub const META_ELLIPSE: ::WORD = 0x0418;
pub const META_FLOODFILL: ::WORD = 0x0419;
pub const META_PIE: ::WORD = 0x081A;
pub const META_RECTANGLE: ::WORD = 0x041B;
pub const META_ROUNDRECT: ::WORD = 0x061C;
pub const META_PATBLT: ::WORD = 0x061D;
pub const META_SAVEDC: ::WORD = 0x001E;
pub const META_SETPIXEL: ::WORD = 0x041F;
pub const META_OFFSETCLIPRGN: ::WORD = 0x0220;
pub const META_TEXTOUT: ::WORD = 0x0521;
pub const META_BITBLT: ::WORD = 0x0922;
pub const META_STRETCHBLT: ::WORD = 0x0B23;
pub const META_POLYGON: ::WORD = 0x0324;
pub const META_POLYLINE: ::WORD = 0x0325;
pub const META_ESCAPE: ::WORD = 0x0626;
pub const META_RESTOREDC: ::WORD = 0x0127;
pub const META_FILLREGION: ::WORD = 0x0228;
pub const META_FRAMEREGION: ::WORD = 0x0429;
pub const META_INVERTREGION: ::WORD = 0x012A;
pub const META_PAINTREGION: ::WORD = 0x012B;
pub const META_SELECTCLIPREGION: ::WORD = 0x012C;
pub const META_SELECTOBJECT: ::WORD = 0x012D;
pub const META_SETTEXTALIGN: ::WORD = 0x012E;
pub const META_CHORD: ::WORD = 0x0830;
pub const META_SETMAPPERFLAGS: ::WORD = 0x0231;
pub const META_EXTTEXTOUT: ::WORD = 0x0a32;
pub const META_SETDIBTODEV: ::WORD = 0x0d33;
pub const META_SELECTPALETTE: ::WORD = 0x0234;
pub const META_REALIZEPALETTE: ::WORD = 0x0035;
pub const META_ANIMATEPALETTE: ::WORD = 0x0436;
pub const META_SETPALENTRIES: ::WORD = 0x0037;
pub const META_POLYPOLYGON: ::WORD = 0x0538;
pub const META_RESIZEPALETTE: ::WORD = 0x0139;
pub const META_DIBBITBLT: ::WORD = 0x0940;
pub const META_DIBSTRETCHBLT: ::WORD = 0x0b41;
pub const META_DIBCREATEPATTERNBRUSH: ::WORD = 0x0142;
pub const META_STRETCHDIB: ::WORD = 0x0f43;
pub const META_EXTFLOODFILL: ::WORD = 0x0548;
pub const META_SETLAYOUT: ::WORD = 0x0149;
pub const META_DELETEOBJECT: ::WORD = 0x01f0;
pub const META_CREATEPALETTE: ::WORD = 0x00f7;
pub const META_CREATEPATTERNBRUSH: ::WORD = 0x01F9;
pub const META_CREATEPENINDIRECT: ::WORD = 0x02FA;
pub const META_CREATEFONTINDIRECT: ::WORD = 0x02FB;
pub const META_CREATEBRUSHINDIRECT: ::WORD = 0x02FC;
pub const META_CREATEREGION: ::WORD = 0x06FF;
