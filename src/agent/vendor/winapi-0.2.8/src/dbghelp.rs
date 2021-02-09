// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! DbgHelp include file
#[cfg(target_arch = "x86_64")]
STRUCT!{struct LOADED_IMAGE {
    ModuleName: ::PSTR,
    hFile: ::HANDLE,
    MappedAddress: ::PUCHAR,
    FileHeader: ::PIMAGE_NT_HEADERS64,
    LastRvaSection: ::PIMAGE_SECTION_HEADER,
    NumberOfSections: ::ULONG,
    Sections: ::PIMAGE_SECTION_HEADER,
    Characteristics: ::ULONG,
    fSystemImage: ::BOOLEAN,
    fDOSImage: ::BOOLEAN,
    fReadOnly: ::BOOLEAN,
    Version: ::UCHAR,
    Links: ::LIST_ENTRY,
    SizeOfImage: ::ULONG,
}}
#[cfg(target_arch = "x86")]
STRUCT!{struct LOADED_IMAGE {
    ModuleName: ::PSTR,
    hFile: ::HANDLE,
    MappedAddress: ::PUCHAR,
    FileHeader: ::PIMAGE_NT_HEADERS32,
    LastRvaSection: ::PIMAGE_SECTION_HEADER,
    NumberOfSections: ::ULONG,
    Sections: ::PIMAGE_SECTION_HEADER,
    Characteristics: ::ULONG,
    fSystemImage: ::BOOLEAN,
    fDOSImage: ::BOOLEAN,
    fReadOnly: ::BOOLEAN,
    Version: ::UCHAR,
    Links: ::LIST_ENTRY,
    SizeOfImage: ::ULONG,
}}
pub const MAX_SYM_NAME: usize = 2000;
pub const ERROR_IMAGE_NOT_STRIPPED: ::DWORD = 0x8800;
pub const ERROR_NO_DBG_POINTER: ::DWORD = 0x8801;
pub const ERROR_NO_PDB_POINTER: ::DWORD = 0x8802;
pub type PFIND_DEBUG_FILE_CALLBACK = Option<unsafe extern "system" fn(
    FileHandle: ::HANDLE, FileName: ::PCSTR, CallerData: ::PVOID,
) -> ::BOOL>;
pub type PFIND_DEBUG_FILE_CALLBACKW = Option<unsafe extern "system" fn(
    FileHandle: ::HANDLE, FileName: ::PCWSTR, CallerData: ::PVOID,
) -> ::BOOL>;
pub type PFINDFILEINPATHCALLBACK = Option<unsafe extern "system" fn(
    filename: ::PCSTR, context: ::PVOID,
) -> ::BOOL>;
pub type PFINDFILEINPATHCALLBACKW = Option<unsafe extern "system" fn(
    filename: ::PCWSTR, context: ::PVOID,
) -> ::BOOL>;
pub type PFIND_EXE_FILE_CALLBACK = Option<unsafe extern "system" fn(
    FileHandle: ::HANDLE, FileName: ::PCSTR, CallerData: ::PVOID,
) -> ::BOOL>;
pub type PFIND_EXE_FILE_CALLBACKW = Option<unsafe extern "system" fn(
    FileHandle: ::HANDLE, FileName: ::PCWSTR, CallerData: ::PVOID,
) -> ::BOOL>;
#[cfg(target_arch = "x86")]
STRUCT!{struct IMAGE_DEBUG_INFORMATION {
    List: ::LIST_ENTRY,
    ReservedSize: ::DWORD,
    ReservedMappedBase: ::PVOID,
    ReservedMachine: ::USHORT,
    ReservedCharacteristics: ::USHORT,
    ReservedCheckSum: ::DWORD,
    ImageBase: ::DWORD,
    SizeOfImage: ::DWORD,
    ReservedNumberOfSections: ::DWORD,
    ReservedSections: ::PIMAGE_SECTION_HEADER,
    ReservedExportedNamesSize: ::DWORD,
    ReservedExportedNames: ::PSTR,
    ReservedNumberOfFunctionTableEntries: ::DWORD,
    ReservedFunctionTableEntries: ::PIMAGE_FUNCTION_ENTRY,
    ReservedLowestFunctionStartingAddress: ::DWORD,
    ReservedHighestFunctionEndingAddress: ::DWORD,
    ReservedNumberOfFpoTableEntries: ::DWORD,
    ReservedFpoTableEntries: ::PFPO_DATA,
    SizeOfCoffSymbols: ::DWORD,
    CoffSymbols: ::PIMAGE_COFF_SYMBOLS_HEADER,
    ReservedSizeOfCodeViewSymbols: ::DWORD,
    ReservedCodeViewSymbols: ::PVOID,
    ImageFilePath: ::PSTR,
    ImageFileName: ::PSTR,
    ReservedDebugFilePath: ::PSTR,
    ReservedTimeDateStamp: ::DWORD,
    ReservedRomImage: ::BOOL,
    ReservedDebugDirectory: ::PIMAGE_DEBUG_DIRECTORY,
    ReservedNumberOfDebugDirectories: ::DWORD,
    ReservedOriginalFunctionTableBaseAddress: ::DWORD,
    Reserved: [::DWORD; 2],
}}
#[cfg(target_arch = "x86")]
pub type PIMAGE_DEBUG_INFORMATION = *mut IMAGE_DEBUG_INFORMATION;
pub type PENUMDIRTREE_CALLBACK = Option<unsafe extern "system" fn(
    FilePath: ::PCSTR, CallerData: ::PVOID,
) -> ::BOOL>;
pub type PENUMDIRTREE_CALLBACKW = Option<unsafe extern "system" fn(
    FilePath: ::PCWSTR, CallerData: ::PVOID,
) -> ::BOOL>;
pub const UNDNAME_COMPLETE: ::DWORD = 0x0000;
pub const UNDNAME_NO_LEADING_UNDERSCORES: ::DWORD = 0x0001;
pub const UNDNAME_NO_MS_KEYWORDS: ::DWORD = 0x0002;
pub const UNDNAME_NO_FUNCTION_RETURNS: ::DWORD = 0x0004;
pub const UNDNAME_NO_ALLOCATION_MODEL: ::DWORD = 0x0008;
pub const UNDNAME_NO_ALLOCATION_LANGUAGE: ::DWORD = 0x0010;
pub const UNDNAME_NO_MS_THISTYPE: ::DWORD = 0x0020;
pub const UNDNAME_NO_CV_THISTYPE: ::DWORD = 0x0040;
pub const UNDNAME_NO_THISTYPE: ::DWORD = 0x0060;
pub const UNDNAME_NO_ACCESS_SPECIFIERS: ::DWORD = 0x0080;
pub const UNDNAME_NO_THROW_SIGNATURES: ::DWORD = 0x0100;
pub const UNDNAME_NO_MEMBER_TYPE: ::DWORD = 0x0200;
pub const UNDNAME_NO_RETURN_UDT_MODEL: ::DWORD = 0x0400;
pub const UNDNAME_32_BIT_DECODE: ::DWORD = 0x0800;
pub const UNDNAME_NAME_ONLY: ::DWORD = 0x1000;
pub const UNDNAME_NO_ARGUMENTS: ::DWORD = 0x2000;
pub const UNDNAME_NO_SPECIAL_SYMS: ::DWORD = 0x4000;
pub const DBHHEADER_DEBUGDIRS: ::DWORD = 0x1;
pub const DBHHEADER_CVMISC: ::DWORD = 0x2;
pub const DBHHEADER_PDBGUID: ::DWORD = 0x3;
STRUCT!{struct MODLOAD_DATA {
    ssize: ::DWORD,
    ssig: ::DWORD,
    data: ::PVOID,
    size: ::DWORD,
    flags: ::DWORD,
}}
pub type PMODLOAD_DATA = *mut MODLOAD_DATA;
STRUCT!{struct MODLOAD_CVMISC {
    oCV: ::DWORD,
    cCV: ::size_t,
    oMisc: ::DWORD,
    cMisc: ::size_t,
    dtImage: ::DWORD,
    cImage: ::DWORD,
}}
pub type PMODLOAD_CVMISC = *mut MODLOAD_CVMISC;
STRUCT!{struct MODLOAD_PDBGUID_PDBAGE {
    PdbGuid: ::GUID,
    PdbAge: ::DWORD,
}}
pub type PMODLOAD_PDBGUID_PDBAGE = *mut MODLOAD_PDBGUID_PDBAGE;
ENUM!{enum ADDRESS_MODE {
    AddrMode1616,
    AddrMode1632,
    AddrModeReal,
    AddrModeFlat,
}}
STRUCT!{struct ADDRESS64 {
    Offset: ::DWORD64,
    Segment: ::WORD,
    Mode: ::ADDRESS_MODE,
}}
pub type LPADDRESS64 = *mut ADDRESS64;
#[cfg(target_arch = "x86_64")]
pub type ADDRESS = ADDRESS64;
#[cfg(target_arch = "x86_64")]
pub type LPADDRESS = LPADDRESS64;
#[cfg(target_arch = "x86")]
STRUCT!{struct ADDRESS {
    Offset: ::DWORD,
    Segment: ::WORD,
    Mode: ::ADDRESS_MODE,
}}
#[cfg(target_arch = "x86")]
pub type LPADDRESS = *mut ADDRESS;
STRUCT!{struct KDHELP64 {
    Thread: ::DWORD64,
    ThCallbackStack: ::DWORD,
    ThCallbackBStore: ::DWORD,
    NextCallback: ::DWORD,
    FramePointer: ::DWORD,
    KiCallUserMode: ::DWORD64,
    KeUserCallbackDispatcher: ::DWORD64,
    SystemRangeStart: ::DWORD64,
    KiUserExceptionDispatcher: ::DWORD64,
    StackBase: ::DWORD64,
    StackLimit: ::DWORD64,
    BuildVersion: ::DWORD,
    Reserved0: ::DWORD,
    Reserved1: [::DWORD64; 4],
}}
pub type PKDHELP64 = *mut KDHELP64;
#[cfg(target_arch = "x86_64")]
pub type KDHELP = KDHELP64;
#[cfg(target_arch = "x86_64")]
pub type PKDHELP = PKDHELP64;
#[cfg(target_arch = "x86")]
STRUCT!{struct KDHELP {
    Thread: ::DWORD,
    ThCallbackStack: ::DWORD,
    NextCallback: ::DWORD,
    FramePointer: ::DWORD,
    KiCallUserMode: ::DWORD,
    KeUserCallbackDispatcher: ::DWORD,
    SystemRangeStart: ::DWORD,
    ThCallbackBStore: ::DWORD,
    KiUserExceptionDispatcher: ::DWORD,
    StackBase: ::DWORD,
    StackLimit: ::DWORD,
    Reserved: [::DWORD; 5],
}}
#[cfg(target_arch = "x86")]
pub type PKDHELP = *mut KDHELP;
STRUCT!{struct STACKFRAME64 {
    AddrPC: ::ADDRESS64,
    AddrReturn: ::ADDRESS64,
    AddrFrame: ::ADDRESS64,
    AddrStack: ::ADDRESS64,
    AddrBStore: ::ADDRESS64,
    FuncTableEntry: ::PVOID,
    Params: [::DWORD64; 4],
    Far: ::BOOL,
    Virtual: ::BOOL,
    Reserved: [::DWORD64; 3],
    KdHelp: ::KDHELP64,
}}
pub type LPSTACKFRAME64 = *mut STACKFRAME64;
pub const INLINE_FRAME_CONTEXT_INIT: ::DWORD = 0;
pub const INLINE_FRAME_CONTEXT_IGNORE: ::DWORD = 0xFFFFFFFF;
STRUCT!{struct STACKFRAME_EX {
    AddrPC: ::ADDRESS64,
    AddrReturn: ::ADDRESS64,
    AddrFrame: ::ADDRESS64,
    AddrStack: ::ADDRESS64,
    AddrBStore: ::ADDRESS64,
    FuncTableEntry: ::PVOID,
    Params: [::DWORD64; 4],
    Far: ::BOOL,
    Virtual: ::BOOL,
    Reserved: [::DWORD64; 3],
    KdHelp: ::KDHELP64,
    StackFrameSize: ::DWORD,
    InlineFrameContext: ::DWORD,
}}
pub type LPSTACKFRAME_EX = *mut STACKFRAME_EX;
#[cfg(target_arch = "x86_64")]
pub type STACKFRAME = STACKFRAME64;
#[cfg(target_arch = "x86_64")]
pub type LPSTACKFRAME = LPSTACKFRAME64;
#[cfg(target_arch = "x86")]
STRUCT!{struct STACKFRAME {
    AddrPC: ::ADDRESS,
    AddrReturn: ::ADDRESS,
    AddrFrame: ::ADDRESS,
    AddrStack: ::ADDRESS,
    FuncTableEntry: ::PVOID,
    Params: [::DWORD; 4],
    Far: ::BOOL,
    Virtual: ::BOOL,
    Reserved: [::DWORD; 3],
    KdHelp: ::KDHELP,
    AddrBStore: ::ADDRESS,
}}
#[cfg(target_arch = "x86")]
pub type LPSTACKFRAME = *mut STACKFRAME;
pub type PREAD_PROCESS_MEMORY_ROUTINE64 = Option<unsafe extern "system" fn(
    hProcess: ::HANDLE, qwBaseAddress: ::DWORD64, lpBuffer: ::PVOID, nSize: ::DWORD,
    lpNumberOfBytesRead: ::LPDWORD,
) -> ::BOOL>;
pub type PFUNCTION_TABLE_ACCESS_ROUTINE64 = Option<unsafe extern "system" fn(
    ahProcess: ::HANDLE, AddrBase: ::DWORD64,
) -> ::PVOID>;
pub type PGET_MODULE_BASE_ROUTINE64 = Option<unsafe extern "system" fn(
    hProcess: ::HANDLE, Address: ::DWORD64,
) -> ::DWORD64>;
pub type PTRANSLATE_ADDRESS_ROUTINE64 = Option<unsafe extern "system" fn(
    hProcess: ::HANDLE, hThread: ::HANDLE, lpaddr: LPADDRESS64,
) -> ::DWORD64>;
pub const SYM_STKWALK_DEFAULT: ::DWORD = 0x00000000;
pub const SYM_STKWALK_FORCE_FRAMEPTR: ::DWORD = 0x00000001;
#[cfg(target_arch = "x86_64")]
pub type PREAD_PROCESS_MEMORY_ROUTINE = PREAD_PROCESS_MEMORY_ROUTINE64;
#[cfg(target_arch = "x86_64")]
pub type PFUNCTION_TABLE_ACCESS_ROUTINE = PFUNCTION_TABLE_ACCESS_ROUTINE64;
#[cfg(target_arch = "x86_64")]
pub type PGET_MODULE_BASE_ROUTINE = PGET_MODULE_BASE_ROUTINE64;
#[cfg(target_arch = "x86_64")]
pub type PTRANSLATE_ADDRESS_ROUTINE = PTRANSLATE_ADDRESS_ROUTINE64;
#[cfg(target_arch = "x86")]
pub type PREAD_PROCESS_MEMORY_ROUTINE = Option<unsafe extern "system" fn(
    hProcess: ::HANDLE, qwBaseAddress: ::DWORD, lpBuffer: ::PVOID, nSize: ::DWORD,
    lpNumberOfBytesRead: ::PDWORD,
) -> ::BOOL>;
#[cfg(target_arch = "x86")]
pub type PFUNCTION_TABLE_ACCESS_ROUTINE = Option<unsafe extern "system" fn(
    ahProcess: ::HANDLE, AddrBase: ::DWORD,
) -> ::PVOID>;
#[cfg(target_arch = "x86")]
pub type PGET_MODULE_BASE_ROUTINE = Option<unsafe extern "system" fn(
    hProcess: ::HANDLE, Address: ::DWORD,
) -> ::DWORD>;
#[cfg(target_arch = "x86")]
pub type PTRANSLATE_ADDRESS_ROUTINE = Option<unsafe extern "system" fn(
    hProcess: ::HANDLE, hThread: ::HANDLE, lpaddr: LPADDRESS,
) -> ::DWORD>;
pub const API_VERSION_NUMBER: ::USHORT = 12;
STRUCT!{struct API_VERSION {
    MajorVersion: ::USHORT,
    MinorVersion: ::USHORT,
    Revision: ::USHORT,
    Reserved: ::USHORT,
}}
pub type LPAPI_VERSION = *mut API_VERSION;
STRUCT!{struct SYMBOL_INFOW {
    SizeOfStruct: ::ULONG,
    TypeIndex: ::ULONG,
    Reserved: [::ULONG64; 2],
    Index: ::ULONG,
    Size: ::ULONG,
    ModBase: ::ULONG64,
    Flags: ::ULONG,
    Value: ::ULONG64,
    Address: ::ULONG64,
    Register: ::ULONG,
    Scope: ::ULONG,
    Tag: ::ULONG,
    NameLen: ::ULONG,
    MaxNameLen: ::ULONG,
    Name: [::WCHAR; 1],
}}
pub type PSYMBOL_INFOW = *mut SYMBOL_INFOW;
STRUCT!{struct IMAGEHLP_SYMBOL64 {
    SizeOfStruct: ::DWORD,
    Address: ::DWORD64,
    Size: ::DWORD,
    Flags: ::DWORD,
    MaxNameLength: ::DWORD,
    Name: [::CHAR; 1],
}}
pub type PIMAGEHLP_SYMBOL64 = *mut IMAGEHLP_SYMBOL64;
STRUCT!{struct IMAGEHLP_LINEW64 {
    SizeOfStruct: ::DWORD,
    Key: ::PVOID,
    LineNumber: ::DWORD,
    FileName: ::PWSTR,
    Address: ::DWORD64,
}}
pub type PIMAGEHLP_LINEW64 = *mut IMAGEHLP_LINEW64;
