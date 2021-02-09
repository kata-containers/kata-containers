// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! ApiSet Contract for api-ms-win-core-file-l1
pub const CREATE_NEW: ::DWORD = 1;
pub const CREATE_ALWAYS: ::DWORD = 2;
pub const OPEN_EXISTING: ::DWORD = 3;
pub const OPEN_ALWAYS: ::DWORD = 4;
pub const TRUNCATE_EXISTING: ::DWORD = 5;
pub const INVALID_FILE_SIZE: ::DWORD = 0xFFFFFFFF;
pub const INVALID_SET_FILE_POINTER: ::DWORD = 0xFFFFFFFF;
pub const INVALID_FILE_ATTRIBUTES: ::DWORD = 0xFFFFFFFF;
STRUCT!{struct WIN32_FILE_ATTRIBUTE_DATA {
    dwFileAttributes: ::DWORD,
    ftCreationTime: ::FILETIME,
    ftLastAccessTime: ::FILETIME,
    ftLastWriteTime: ::FILETIME,
    nFileSizeHigh: ::DWORD,
    nFileSizeLow: ::DWORD,
}}
pub type LPWIN32_FILE_ATTRIBUTE_DATA = *mut WIN32_FILE_ATTRIBUTE_DATA;
STRUCT!{struct BY_HANDLE_FILE_INFORMATION {
    dwFileAttributes: ::DWORD,
    ftCreationTime: ::FILETIME,
    ftLastAccessTime: ::FILETIME,
    ftLastWriteTime: ::FILETIME,
    dwVolumeSerialNumber: ::DWORD,
    nFileSizeHigh: ::DWORD,
    nFileSizeLow: ::DWORD,
    nNumberOfLinks: ::DWORD,
    nFileIndexHigh: ::DWORD,
    nFileIndexLow: ::DWORD,
}}
pub type PBY_HANDLE_FILE_INFORMATION = *mut BY_HANDLE_FILE_INFORMATION;
pub type LPBY_HANDLE_FILE_INFORMATION = *mut BY_HANDLE_FILE_INFORMATION;
STRUCT!{struct CREATEFILE2_EXTENDED_PARAMETERS {
    dwSize: ::DWORD,
    dwFileAttributes: ::DWORD,
    dwFileFlags: ::DWORD,
    dwSecurityQosFlags: ::DWORD,
    lpSecurityAttributes: ::LPSECURITY_ATTRIBUTES,
    hTemplateFile: ::HANDLE,
}}
pub type PCREATEFILE2_EXTENDED_PARAMETERS = *mut CREATEFILE2_EXTENDED_PARAMETERS;
pub type LPCREATEFILE2_EXTENDED_PARAMETERS = *mut CREATEFILE2_EXTENDED_PARAMETERS;
ENUM!{enum PRIORITY_HINT {
    IoPriorityHintVeryLow = 0,
    IoPriorityHintLow = 1,
    IoPriorityHintNormal = 2,
    MaximumIoPriorityHintType = 3,
}}
STRUCT!{struct FILE_BASIC_INFO {
    CreationTime: ::LARGE_INTEGER,
    LastAccessTime: ::LARGE_INTEGER,
    LastWriteTime: ::LARGE_INTEGER,
    ChangeTime: ::LARGE_INTEGER,
    FileAttributes: ::DWORD,
}}
STRUCT!{struct FILE_STANDARD_INFO {
    AllocationSize: ::LARGE_INTEGER,
    EndOfFile: ::LARGE_INTEGER,
    NumberOfLinks: ::DWORD,
    DeletePending: ::BOOLEAN,
    Directory: ::BOOLEAN,
}}
STRUCT!{struct FILE_NAME_INFO {
    FileNameLength: ::DWORD,
    FileName: [::WCHAR; 0],
}}
STRUCT!{struct FILE_RENAME_INFO {
    ReplaceIfExists: ::BOOL,
    RootDirectory: ::HANDLE,
    FileNameLength: ::DWORD,
    FileName: [::WCHAR; 0],
}}
STRUCT!{struct FILE_DISPOSITION_INFO {
    DeleteFile: ::BOOL,
}}
STRUCT!{struct FILE_ALLOCATION_INFO {
    AllocationSize: ::LARGE_INTEGER,
}}
STRUCT!{struct FILE_END_OF_FILE_INFO {
    EndOfFile: ::LARGE_INTEGER,
}}
STRUCT!{struct FILE_STREAM_INFO {
    NextEntryOffset: ::DWORD,
    StreamNameLength: ::DWORD,
    StreamSize: ::DWORD,
    StreamAllocationSize: ::DWORD,
    StreamName: [::WCHAR; 0],
}}
STRUCT!{struct FILE_COMPRESSION_INFO {
    CompressedFileSize: ::LARGE_INTEGER,
    CompressionFormat: ::WORD,
    CompressionUnitShift: ::UCHAR,
    ChunkShift: ::UCHAR,
    ClusterShift: ::UCHAR,
    Reserved: [::UCHAR; 3],
}}
STRUCT!{struct FILE_ATTRIBUTE_TAG_INFO {
    NextEntryOffset: ::DWORD,
    ReparseTag: ::DWORD,
}}
STRUCT!{struct FILE_ID_BOTH_DIR_INFO {
    NextEntryOffset: ::DWORD,
    FileIndex: ::DWORD,
    CreationTime: ::LARGE_INTEGER,
    LastAccessTime: ::LARGE_INTEGER,
    LastWriteTime: ::LARGE_INTEGER,
    ChangeTime: ::LARGE_INTEGER,
    EndOfFile: ::LARGE_INTEGER,
    AllocationSize: ::LARGE_INTEGER,
    FileAttributes: ::DWORD,
    FileNameLength: ::DWORD,
    EaSize: ::DWORD,
    ShortNameLength: ::CCHAR,
    ShortName: [::WCHAR; 12],
    FileId: ::LARGE_INTEGER,
    FileName: [::WCHAR; 0],
}}
STRUCT!{struct FILE_IO_PRIORITY_HINT_INFO {
    PriorityHint: ::PRIORITY_HINT,
}}
STRUCT!{struct FILE_FULL_DIR_INFO {
    NextEntryOffset: ::ULONG,
    FileIndex: ::ULONG,
    CreationTime: ::LARGE_INTEGER,
    LastAccessTime: ::LARGE_INTEGER,
    LastWriteTime: ::LARGE_INTEGER,
    ChangeTime: ::LARGE_INTEGER,
    EndOfFile: ::LARGE_INTEGER,
    AllocationSize: ::LARGE_INTEGER,
    FileAttributes: ::ULONG,
    FileNameLength: ::ULONG,
    EaSize: ::ULONG,
    FileName: [::WCHAR; 0],
}}
STRUCT!{struct FILE_STORAGE_INFO {
    LogicalBytesPerSector: ::ULONG,
    PhysicalBytesPerSectorForAtomicity: ::ULONG,
    PhysicalBytesPerSectorForPerformance: ::ULONG,
    FileSystemEffectivePhysicalBytesPerSectorForAtomicity: ::ULONG,
    Flags: ::ULONG,
    ByteOffsetForSectorAlignment: ::ULONG,
    ByteOffsetForPartitionAlignment: ::ULONG,
}}
STRUCT!{struct FILE_ALIGNMENT_INFO {
    AlignmentRequirement: ::ULONG,
}}
STRUCT!{struct FILE_ID_INFO {
    VolumeSerialNumber: ::ULONGLONG,
    FileId: ::FILE_ID_128,
}}
