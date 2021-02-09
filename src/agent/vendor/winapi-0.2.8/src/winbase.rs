// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! This module defines the 32-Bit Windows Base APIs
pub const FILE_BEGIN: ::DWORD = 0;
pub const FILE_CURRENT: ::DWORD = 1;
pub const FILE_END: ::DWORD = 2;
pub const WAIT_FAILED: ::DWORD = 0xFFFFFFFF;
pub const WAIT_OBJECT_0: ::DWORD = ::STATUS_WAIT_0 as ::DWORD;
pub const WAIT_ABANDONED: ::DWORD = ::STATUS_ABANDONED_WAIT_0 as ::DWORD;
pub const WAIT_ABANDONED_0: ::DWORD = ::STATUS_ABANDONED_WAIT_0 as ::DWORD;
pub const WAIT_IO_COMPLETION: ::DWORD = ::STATUS_USER_APC as ::DWORD;
pub const FILE_FLAG_WRITE_THROUGH: ::DWORD = 0x80000000;
pub const FILE_FLAG_OVERLAPPED: ::DWORD = 0x40000000;
pub const FILE_FLAG_NO_BUFFERING: ::DWORD = 0x20000000;
pub const FILE_FLAG_RANDOM_ACCESS: ::DWORD = 0x10000000;
pub const FILE_FLAG_SEQUENTIAL_SCAN: ::DWORD = 0x08000000;
pub const FILE_FLAG_DELETE_ON_CLOSE: ::DWORD = 0x04000000;
pub const FILE_FLAG_BACKUP_SEMANTICS: ::DWORD = 0x02000000;
pub const FILE_FLAG_POSIX_SEMANTICS: ::DWORD = 0x01000000;
pub const FILE_FLAG_SESSION_AWARE: ::DWORD = 0x00800000;
pub const FILE_FLAG_OPEN_REPARSE_POINT: ::DWORD = 0x00200000;
pub const FILE_FLAG_OPEN_NO_RECALL: ::DWORD = 0x00100000;
pub const FILE_FLAG_FIRST_PIPE_INSTANCE: ::DWORD = 0x00080000;
pub const FILE_FLAG_OPEN_REQUIRING_OPLOCK: ::DWORD = 0x00040000;
pub const PROGRESS_CONTINUE: ::DWORD = 0;
pub const PROGRESS_CANCEL: ::DWORD = 1;
pub const PROGRESS_STOP: ::DWORD = 2;
pub const PROGRESS_QUIET: ::DWORD = 3;
pub const CALLBACK_CHUNK_FINISHED: ::DWORD = 0x00000000;
pub const CALLBACK_STREAM_SWITCH: ::DWORD = 0x00000001;
pub const COPY_FILE_FAIL_IF_EXISTS: ::DWORD = 0x00000001;
pub const COPY_FILE_RESTARTABLE: ::DWORD = 0x00000002;
pub const COPY_FILE_OPEN_SOURCE_FOR_WRITE: ::DWORD = 0x00000004;
pub const COPY_FILE_ALLOW_DECRYPTED_DESTINATION: ::DWORD = 0x00000008;
pub const COPY_FILE_COPY_SYMLINK: ::DWORD = 0x00000800;
pub const COPY_FILE_NO_BUFFERING: ::DWORD = 0x00001000;
pub const COPY_FILE_REQUEST_SECURITY_PRIVILEGES: ::DWORD = 0x00002000;
pub const COPY_FILE_RESUME_FROM_PAUSE: ::DWORD = 0x00004000;
pub const COPY_FILE_NO_OFFLOAD: ::DWORD = 0x00040000;
pub const REPLACEFILE_WRITE_THROUGH: ::DWORD = 0x00000001;
pub const REPLACEFILE_IGNORE_MERGE_ERRORS: ::DWORD = 0x00000002;
pub const REPLACEFILE_IGNORE_ACL_ERRORS: ::DWORD = 0x00000004;
pub const PIPE_ACCESS_INBOUND: ::DWORD = 0x00000001;
pub const PIPE_ACCESS_OUTBOUND: ::DWORD = 0x00000002;
pub const PIPE_ACCESS_DUPLEX: ::DWORD = 0x00000003;
pub const PIPE_CLIENT_END: ::DWORD = 0x00000000;
pub const PIPE_SERVER_END: ::DWORD = 0x00000001;
pub const PIPE_WAIT: ::DWORD = 0x00000000;
pub const PIPE_NOWAIT: ::DWORD = 0x00000001;
pub const PIPE_READMODE_BYTE: ::DWORD = 0x00000000;
pub const PIPE_READMODE_MESSAGE: ::DWORD = 0x00000002;
pub const PIPE_TYPE_BYTE: ::DWORD = 0x00000000;
pub const PIPE_TYPE_MESSAGE: ::DWORD = 0x00000004;
pub const PIPE_ACCEPT_REMOTE_CLIENTS: ::DWORD = 0x00000000;
pub const PIPE_REJECT_REMOTE_CLIENTS: ::DWORD = 0x00000008;
pub const PIPE_UNLIMITED_INSTANCES: ::DWORD = 255;
//270
pub const SECURITY_CONTEXT_TRACKING: ::DWORD = 0x00040000;
pub const SECURITY_EFFECTIVE_ONLY: ::DWORD = 0x00080000;
pub const SECURITY_SQOS_PRESENT: ::DWORD = 0x00100000;
pub const SECURITY_VALID_SQOS_FLAGS: ::DWORD = 0x001F0000;
//282
pub type PFIBER_START_ROUTINE = Option<unsafe extern "system" fn(lpFiberParameter: ::LPVOID)>;
pub type LPFIBER_START_ROUTINE = PFIBER_START_ROUTINE;
pub type PFIBER_CALLOUT_ROUTINE = Option<unsafe extern "system" fn(
    lpParameter: ::LPVOID,
) -> ::LPVOID>;
//299
pub type LPLDT_ENTRY = ::LPVOID; // TODO - fix this for 32-bit
//405
STRUCT!{struct COMMPROP {
    wPacketLength: ::WORD,
    wPacketVersion: ::WORD,
    dwServiceMask: ::DWORD,
    dwReserved1: ::DWORD,
    dwMaxTxQueue: ::DWORD,
    dwMaxRxQueue: ::DWORD,
    dwMaxBaud: ::DWORD,
    dwProvSubType: ::DWORD,
    dwProvCapabilities: ::DWORD,
    dwSettableParams: ::DWORD,
    dwSettableBaud: ::DWORD,
    wSettableData: ::WORD,
    wSettableStopParity: ::WORD,
    dwCurrentTxQueue: ::DWORD,
    dwCurrentRxQueue: ::DWORD,
    dwProvSpec1: ::DWORD,
    dwProvSpec2: ::DWORD,
    wcProvChar: [::WCHAR; 1],
}}
pub type LPCOMMPROP = *mut COMMPROP;
//432
STRUCT!{struct COMSTAT {
    BitFields: ::DWORD,
    cbInQue: ::DWORD,
    cbOutQue : ::DWORD,
}}
BITFIELD!(COMSTAT BitFields: ::DWORD [
    fCtsHold set_fCtsHold[0..1],
    fDsrHold set_fDsrHold[1..2],
    fRlsdHold set_fRlsdHold[2..3],
    fXoffHold set_fXoffHold[3..4],
    fXoffSent set_fXoffSent[4..5],
    fEof set_fEof[5..6],
    fTxim set_fTxim[6..7],
    fReserved set_fReserved[7..32],
]);
pub type LPCOMSTAT = *mut COMSTAT;
//460
STRUCT!{struct DCB {
    DCBlength: ::DWORD,
    BaudRate: ::DWORD,
    BitFields: ::DWORD,
    wReserved: ::WORD,
    XonLim: ::WORD,
    XoffLim: ::WORD,
    ByteSize: ::BYTE,
    Parity: ::BYTE,
    StopBits: ::BYTE,
    XonChar: ::c_char,
    XoffChar: ::c_char,
    ErrorChar: ::c_char,
    EofChar: ::c_char,
    EvtChar: ::c_char,
    wReserved1: ::WORD,
}}
BITFIELD!(DCB BitFields: ::DWORD [
    fBinary set_fBinary[0..1],
    fParity set_fParity[1..2],
    fOutxCtsFlow set_fOutxCtsFlow[2..3],
    fOutxDsrFlow set_fOutxDsrFlow[3..4],
    fDtrControl set_fDtrControl[4..6],
    fDsrSensitivity set_fDsrSensitivity[6..7],
    fTXContinueOnXoff set_fTXContinueOnXoff[7..8],
    fOutX set_fOutX[8..9],
    fInX set_fInX[9..10],
    fErrorChar set_fErrorChar[10..11],
    fNull set_fNull[11..12],
    fRtsControl set_fRtsControl[12..14],
    fAbortOnError set_fAbortOnError[14..15],
    fDummy2 set_fDummy2[15..32],
]);
pub type LPDCB = *mut DCB;
STRUCT!{struct COMMTIMEOUTS {
    ReadIntervalTimeout: ::DWORD,
    ReadTotalTimeoutMultiplier: ::DWORD,
    ReadTotalTimeoutConstant: ::DWORD,
    WriteTotalTimeoutMultiplier: ::DWORD,
    WriteTotalTimeoutConstant: ::DWORD,
}}
pub type LPCOMMTIMEOUTS = *mut COMMTIMEOUTS;
STRUCT!{struct COMMCONFIG {
    dwSize: ::DWORD,
    wVersion: ::WORD,
    wReserved: ::WORD,
    dcb: DCB,
    dwProviderSubType: ::DWORD,
    dwProviderOffset: ::DWORD,
    dwProviderSize: ::DWORD,
    wcProviderData: [::WCHAR; 1],
}}
pub type LPCOMMCONFIG = *mut COMMCONFIG;
//547
STRUCT!{struct MEMORYSTATUS {
    dwLength: ::DWORD,
    dwMemoryLoad: ::DWORD,
    dwTotalPhys: ::SIZE_T,
    dwAvailPhys: ::SIZE_T,
    dwTotalPageFile: ::SIZE_T,
    dwAvailPageFile: ::SIZE_T,
    dwTotalVirtual: ::SIZE_T,
    dwAvailVirtual: ::SIZE_T,
}}
pub type LPMEMORYSTATUS = *mut MEMORYSTATUS;
//568
pub const DEBUG_PROCESS: ::DWORD = 0x00000001;
pub const DEBUG_ONLY_THIS_PROCESS: ::DWORD = 0x00000002;
pub const CREATE_SUSPENDED: ::DWORD = 0x00000004;
pub const DETACHED_PROCESS: ::DWORD = 0x00000008;
pub const CREATE_NEW_CONSOLE: ::DWORD = 0x00000010;
pub const NORMAL_PRIORITY_CLASS: ::DWORD = 0x00000020;
pub const IDLE_PRIORITY_CLASS: ::DWORD = 0x00000040;
pub const HIGH_PRIORITY_CLASS: ::DWORD = 0x00000080;
pub const REALTIME_PRIORITY_CLASS: ::DWORD = 0x00000100;
pub const CREATE_NEW_PROCESS_GROUP: ::DWORD = 0x00000200;
pub const CREATE_UNICODE_ENVIRONMENT: ::DWORD = 0x00000400;
pub const CREATE_SEPARATE_WOW_VDM: ::DWORD = 0x00000800;
pub const CREATE_SHARED_WOW_VDM: ::DWORD = 0x00001000;
pub const CREATE_FORCEDOS: ::DWORD = 0x00002000;
pub const BELOW_NORMAL_PRIORITY_CLASS: ::DWORD = 0x00004000;
pub const ABOVE_NORMAL_PRIORITY_CLASS: ::DWORD = 0x00008000;
pub const INHERIT_PARENT_AFFINITY: ::DWORD = 0x00010000;
pub const INHERIT_CALLER_PRIORITY: ::DWORD = 0x00020000;
pub const CREATE_PROTECTED_PROCESS: ::DWORD = 0x00040000;
pub const EXTENDED_STARTUPINFO_PRESENT: ::DWORD = 0x00080000;
pub const PROCESS_MODE_BACKGROUND_BEGIN: ::DWORD = 0x00100000;
pub const PROCESS_MODE_BACKGROUND_END: ::DWORD = 0x00200000;
pub const CREATE_BREAKAWAY_FROM_JOB: ::DWORD = 0x01000000;
pub const CREATE_PRESERVE_CODE_AUTHZ_LEVEL: ::DWORD = 0x02000000;
pub const CREATE_DEFAULT_ERROR_MODE: ::DWORD = 0x04000000;
pub const CREATE_NO_WINDOW: ::DWORD = 0x08000000;
pub const PROFILE_USER: ::DWORD = 0x10000000;
pub const PROFILE_KERNEL: ::DWORD = 0x20000000;
pub const PROFILE_SERVER: ::DWORD = 0x40000000;
pub const CREATE_IGNORE_SYSTEM_DEFAULT: ::DWORD = 0x80000000;
//618
pub const THREAD_PRIORITY_LOWEST: ::DWORD = ::THREAD_BASE_PRIORITY_MIN;
pub const THREAD_PRIORITY_BELOW_NORMAL: ::DWORD = THREAD_PRIORITY_LOWEST + 1;
pub const THREAD_PRIORITY_NORMAL: ::DWORD = 0;
pub const THREAD_PRIORITY_HIGHEST: ::DWORD = ::THREAD_BASE_PRIORITY_MAX;
pub const THREAD_PRIORITY_ABOVE_NORMAL: ::DWORD = THREAD_PRIORITY_HIGHEST - 1;
pub const THREAD_PRIORITY_ERROR_RETURN: ::DWORD = ::MAXLONG as ::DWORD;
pub const THREAD_PRIORITY_TIME_CRITICAL: ::DWORD = ::THREAD_BASE_PRIORITY_LOWRT;
pub const THREAD_PRIORITY_IDLE: ::DWORD = ::THREAD_BASE_PRIORITY_IDLE;
pub const THREAD_MODE_BACKGROUND_BEGIN: ::DWORD = 0x00010000;
pub const THREAD_MODE_BACKGROUND_END: ::DWORD = 0x00020000;
//666
pub const DRIVE_UNKNOWN: ::DWORD = 0;
pub const DRIVE_NO_ROOT_DIR: ::DWORD = 1;
pub const DRIVE_REMOVABLE: ::DWORD = 2;
pub const DRIVE_FIXED: ::DWORD = 3;
pub const DRIVE_REMOTE: ::DWORD = 4;
pub const DRIVE_CDROM: ::DWORD = 5;
pub const DRIVE_RAMDISK: ::DWORD = 6;
pub const FILE_TYPE_UNKNOWN: ::DWORD = 0x0000;
pub const FILE_TYPE_DISK: ::DWORD = 0x0001;
pub const FILE_TYPE_CHAR: ::DWORD = 0x0002;
pub const FILE_TYPE_PIPE: ::DWORD = 0x0003;
pub const FILE_TYPE_REMOTE: ::DWORD = 0x8000;
pub const STD_INPUT_HANDLE: ::DWORD = 0xFFFFFFF6;
pub const STD_OUTPUT_HANDLE: ::DWORD = 0xFFFFFFF5;
pub const STD_ERROR_HANDLE: ::DWORD = 0xFFFFFFF4;
pub const NOPARITY: ::DWORD = 0;
pub const ODDPARITY: ::DWORD = 1;
pub const EVENPARITY: ::DWORD = 2;
pub const MARKPARITY: ::DWORD = 3;
pub const SPACEPARITY: ::DWORD = 4;
pub const ONESTOPBIT: ::DWORD = 0;
pub const ONE5STOPBITS: ::DWORD = 1;
pub const TWOSTOPBITS: ::DWORD = 2;
pub const IGNORE: ::DWORD = 0;
pub const INFINITE: ::DWORD = 0xFFFFFFFF;
//1729
pub const SEM_FAILCRITICALERRORS: ::UINT = 0x0001;
pub const SEM_NOGPFAULTERRORBOX: ::UINT = 0x0002;
pub const SEM_NOALIGNMENTFAULTEXCEPT: ::UINT = 0x0004;
pub const SEM_NOOPENFILEERRORBOX: ::UINT = 0x8000;
//2320
pub const FORMAT_MESSAGE_IGNORE_INSERTS: ::DWORD = 0x00000200;
pub const FORMAT_MESSAGE_FROM_STRING: ::DWORD = 0x00000400;
pub const FORMAT_MESSAGE_FROM_HMODULE: ::DWORD = 0x00000800;
pub const FORMAT_MESSAGE_FROM_SYSTEM: ::DWORD = 0x00001000;
pub const FORMAT_MESSAGE_ARGUMENT_ARRAY: ::DWORD = 0x00002000;
pub const FORMAT_MESSAGE_MAX_WIDTH_MASK: ::DWORD = 0x000000FF;
pub const FORMAT_MESSAGE_ALLOCATE_BUFFER: ::DWORD = 0x00000100;
//2873
pub const STARTF_USESHOWWINDOW: ::DWORD = 0x00000001;
pub const STARTF_USESIZE: ::DWORD = 0x00000002;
pub const STARTF_USEPOSITION: ::DWORD = 0x00000004;
pub const STARTF_USECOUNTCHARS: ::DWORD = 0x00000008;
pub const STARTF_USEFILLATTRIBUTE: ::DWORD = 0x00000010;
pub const STARTF_RUNFULLSCREEN: ::DWORD = 0x00000020;
pub const STARTF_FORCEONFEEDBACK: ::DWORD = 0x00000040;
pub const STARTF_FORCEOFFFEEDBACK: ::DWORD = 0x00000080;
pub const STARTF_USESTDHANDLES: ::DWORD = 0x00000100;
pub const STARTF_USEHOTKEY: ::DWORD = 0x00000200;
pub const STARTF_TITLEISLINKNAME: ::DWORD = 0x00000800;
pub const STARTF_TITLEISAPPID: ::DWORD = 0x00001000;
pub const STARTF_PREVENTPINNING: ::DWORD = 0x00002000;
pub const STARTF_UNTRUSTEDSOURCE: ::DWORD = 0x00008000;
//5002
pub type LPPROGRESS_ROUTINE = Option<unsafe extern "system" fn(
    TotalFileSize: ::LARGE_INTEGER, TotalBytesTransferred: ::LARGE_INTEGER,
    StreamSize: ::LARGE_INTEGER, StreamBytesTransferred: ::LARGE_INTEGER, dwStreamNumber: ::DWORD,
    dwCallbackReason: ::DWORD, hSourceFile: ::HANDLE, hDestinationFile: ::HANDLE, lpData: ::LPVOID,
) -> ::DWORD>;
//5095
ENUM!{enum COPYFILE2_MESSAGE_TYPE {
    COPYFILE2_CALLBACK_NONE = 0,
    COPYFILE2_CALLBACK_CHUNK_STARTED,
    COPYFILE2_CALLBACK_CHUNK_FINISHED,
    COPYFILE2_CALLBACK_STREAM_STARTED,
    COPYFILE2_CALLBACK_STREAM_FINISHED,
    COPYFILE2_CALLBACK_POLL_CONTINUE,
    COPYFILE2_CALLBACK_ERROR,
    COPYFILE2_CALLBACK_MAX,
}}
ENUM!{enum COPYFILE2_MESSAGE_ACTION {
    COPYFILE2_PROGRESS_CONTINUE = 0,
    COPYFILE2_PROGRESS_CANCEL,
    COPYFILE2_PROGRESS_STOP,
    COPYFILE2_PROGRESS_QUIET,
    COPYFILE2_PROGRESS_PAUSE,
}}
ENUM!{enum COPYFILE2_COPY_PHASE {
    COPYFILE2_PHASE_NONE = 0,
    COPYFILE2_PHASE_PREPARE_SOURCE,
    COPYFILE2_PHASE_PREPARE_DEST,
    COPYFILE2_PHASE_READ_SOURCE,
    COPYFILE2_PHASE_WRITE_DESTINATION,
    COPYFILE2_PHASE_SERVER_COPY,
    COPYFILE2_PHASE_NAMEGRAFT_COPY,
    COPYFILE2_PHASE_MAX,
}}
//5129
STRUCT!{struct COPYFILE2_MESSAGE_ChunkStarted {
    dwStreamNumber: ::DWORD,
    dwReserved: ::DWORD,
    hSourceFile: ::HANDLE,
    hDestinationFile: ::HANDLE,
    uliChunkNumber: ::ULARGE_INTEGER,
    uliChunkSize: ::ULARGE_INTEGER,
    uliStreamSize: ::ULARGE_INTEGER,
    uliTotalFileSize: ::ULARGE_INTEGER,
}}
STRUCT!{struct COPYFILE2_MESSAGE_ChunkFinished {
    dwStreamNumber: ::DWORD,
    dwFlags: ::DWORD,
    hSourceFile: ::HANDLE,
    hDestinationFile: ::HANDLE,
    uliChunkNumber: ::ULARGE_INTEGER,
    uliChunkSize: ::ULARGE_INTEGER,
    uliStreamSize: ::ULARGE_INTEGER,
    uliStreamBytesTransferred: ::ULARGE_INTEGER,
    uliTotalFileSize: ::ULARGE_INTEGER,
    uliTotalBytesTransferred: ::ULARGE_INTEGER,
}}
STRUCT!{struct COPYFILE2_MESSAGE_StreamStarted {
    dwStreamNumber: ::DWORD,
    dwReserved: ::DWORD,
    hSourceFile: ::HANDLE,
    hDestinationFile: ::HANDLE,
    uliStreamSize: ::ULARGE_INTEGER,
    uliTotalFileSize: ::ULARGE_INTEGER,
}}
STRUCT!{struct COPYFILE2_MESSAGE_StreamFinished {
    dwStreamNumber: ::DWORD,
    dwReserved: ::DWORD,
    hSourceFile: ::HANDLE,
    hDestinationFile: ::HANDLE,
    uliStreamSize: ::ULARGE_INTEGER,
    uliStreamBytesTransferred: ::ULARGE_INTEGER,
    uliTotalFileSize: ::ULARGE_INTEGER,
    uliTotalBytesTransferred: ::ULARGE_INTEGER,
}}
STRUCT!{struct COPYFILE2_MESSAGE_PollContinue {
    dwReserved: ::DWORD,
}}
STRUCT!{struct COPYFILE2_MESSAGE_Error {
    CopyPhase: COPYFILE2_COPY_PHASE,
    dwStreamNumber: ::DWORD,
    hrFailure: ::HRESULT,
    dwReserved: ::DWORD,
    uliChunkNumber: ::ULARGE_INTEGER,
    uliStreamSize: ::ULARGE_INTEGER,
    uliStreamBytesTransferred: ::ULARGE_INTEGER,
    uliTotalFileSize: ::ULARGE_INTEGER,
    uliTotalBytesTransferred: ::ULARGE_INTEGER,
}}
#[cfg(target_arch="x86")]
STRUCT!{struct COPYFILE2_MESSAGE {
    Type: COPYFILE2_MESSAGE_TYPE,
    dwPadding: ::DWORD,
    Info: [u64; 8],
}}
#[cfg(target_arch="x86_64")]
STRUCT!{struct COPYFILE2_MESSAGE {
    Type: COPYFILE2_MESSAGE_TYPE,
    dwPadding: ::DWORD,
    Info: [u64; 9],
}}
UNION!{COPYFILE2_MESSAGE, Info, ChunkStarted, ChunkStarted_mut, COPYFILE2_MESSAGE_ChunkStarted}
UNION!{COPYFILE2_MESSAGE, Info, ChunkFinished, ChunkFinished_mut, COPYFILE2_MESSAGE_ChunkFinished}
UNION!{COPYFILE2_MESSAGE, Info, StreamStarted, StreamStarted_mut, COPYFILE2_MESSAGE_StreamStarted}
UNION!{COPYFILE2_MESSAGE, Info, StreamFinished, StreamFinished_mut,
    COPYFILE2_MESSAGE_StreamFinished}
UNION!{COPYFILE2_MESSAGE, Info, PollContinue, PollContinue_mut, COPYFILE2_MESSAGE_PollContinue}
UNION!{COPYFILE2_MESSAGE, Info, Error, Error_mut, COPYFILE2_MESSAGE_Error}
pub type PCOPYFILE2_PROGRESS_ROUTINE = Option<unsafe extern "system" fn(
    pMessage: *const COPYFILE2_MESSAGE, pvCallbackContext: ::PVOID,
) -> COPYFILE2_MESSAGE_ACTION>;
STRUCT!{nodebug struct COPYFILE2_EXTENDED_PARAMETERS {
    dwSize: ::DWORD,
    dwCopyFlags: ::DWORD,
    pfCancel: *mut ::BOOL,
    pProgressRoutine: PCOPYFILE2_PROGRESS_ROUTINE,
    pvCallbackContext: ::PVOID,
}}
//5377
pub const MOVEFILE_REPLACE_EXISTING: ::DWORD = 0x00000001;
pub const MOVEFILE_COPY_ALLOWED: ::DWORD = 0x00000002;
pub const MOVEFILE_DELAY_UNTIL_REBOOT: ::DWORD = 0x00000004;
pub const MOVEFILE_WRITE_THROUGH: ::DWORD = 0x00000008;
pub const MOVEFILE_CREATE_HARDLINK: ::DWORD = 0x00000010;
pub const MOVEFILE_FAIL_IF_NOT_TRACKABLE: ::DWORD = 0x00000020;
//7176
pub const HW_PROFILE_GUIDLEN: usize = 39;
//pub const MAX_PROFILE_LEN: usize = 80;
pub const DOCKINFO_UNDOCKED: ::DWORD = 0x1;
pub const DOCKINFO_DOCKED: ::DWORD = 0x2;
pub const DOCKINFO_USER_SUPPLIED: ::DWORD = 0x4;
pub const DOCKINFO_USER_UNDOCKED: ::DWORD = DOCKINFO_USER_SUPPLIED | DOCKINFO_UNDOCKED;
pub const DOCKINFO_USER_DOCKED: ::DWORD = DOCKINFO_USER_SUPPLIED | DOCKINFO_DOCKED;
STRUCT!{nodebug struct HW_PROFILE_INFOA {
    dwDockInfo: ::DWORD,
    szHwProfileGuid: [::CHAR; HW_PROFILE_GUIDLEN],
    szHwProfileName: [::CHAR; ::MAX_PROFILE_LEN],
}}
pub type LPHW_PROFILE_INFOA = *mut HW_PROFILE_INFOA;
STRUCT!{nodebug struct HW_PROFILE_INFOW {
    dwDockInfo: ::DWORD,
    szHwProfileGuid: [::WCHAR; HW_PROFILE_GUIDLEN],
    szHwProfileName: [::WCHAR; ::MAX_PROFILE_LEN],
}}
pub type LPHW_PROFILE_INFOW = *mut HW_PROFILE_INFOW;
//7574
STRUCT!{struct ACTCTXA {
    cbSize: ::ULONG,
    dwFlags: ::DWORD,
    lpSource: ::LPCSTR,
    wProcessorArchitecture: ::USHORT,
    wLangId: ::LANGID,
    lpAssemblyDirectory: ::LPCSTR,
    lpResourceName: ::LPCSTR,
    lpApplicationName: ::LPCSTR,
    hModule: ::HMODULE,
}}
pub type PACTCTXA = *mut ACTCTXA;
STRUCT!{struct ACTCTXW {
    cbSize: ::ULONG,
    dwFlags: ::DWORD,
    lpSource: ::LPCWSTR,
    wProcessorArchitecture: ::USHORT,
    wLangId: ::LANGID,
    lpAssemblyDirectory: ::LPCWSTR,
    lpResourceName: ::LPCWSTR,
    lpApplicationName: ::LPCWSTR,
    hModule: ::HMODULE,
}}
pub type PACTCTXW = *mut ACTCTXW;
pub type PCACTCTXA = *const ACTCTXA;
pub type PCACTCTXW = *const ACTCTXW;
//
pub type PUMS_CONTEXT = *mut ::c_void;
pub type PUMS_COMPLETION_LIST = *mut ::c_void;
pub type UMS_THREAD_INFO_CLASS = ::RTL_UMS_THREAD_INFO_CLASS;
pub type PUMS_THREAD_INFO_CLASS = *mut UMS_THREAD_INFO_CLASS;
pub type PUMS_SCHEDULER_ENTRY_POINT = ::PRTL_UMS_SCHEDULER_ENTRY_POINT;
STRUCT!{nodebug struct UMS_SCHEDULER_STARTUP_INFO {
    UmsVersion: ::ULONG,
    CompletionList: PUMS_COMPLETION_LIST,
    SchedulerProc: PUMS_SCHEDULER_ENTRY_POINT,
    SchedulerParam: ::PVOID,
}}
pub type PUMS_SCHEDULER_STARTUP_INFO = *mut UMS_SCHEDULER_STARTUP_INFO;
STRUCT!{struct UMS_SYSTEM_THREAD_INFORMATION {
    UmsVersion: ::ULONG,
    BitFields: ::ULONG,
}}
BITFIELD!(UMS_SYSTEM_THREAD_INFORMATION BitFields: ::ULONG [
    IsUmsSchedulerThread set_IsUmsSchedulerThread[0..1],
    IsUmsWorkerThread set_IsUmsWorkerThread[1..2],
]);
UNION!(
    UMS_SYSTEM_THREAD_INFORMATION, BitFields, ThreadUmsFlags, ThreadUmsFlags_mut,
    ::ULONG
);
pub type PUMS_SYSTEM_THREAD_INFORMATION = *mut UMS_SYSTEM_THREAD_INFORMATION;
STRUCT!{struct ACTCTX_SECTION_KEYED_DATA_ASSEMBLY_METADATA {
    lpInformation: ::PVOID,
    lpSectionBase: ::PVOID,
    ulSectionLength: ::ULONG,
    lpSectionGlobalDataBase: ::PVOID,
    ulSectionGlobalDataLength: ::ULONG,
}}
pub type PACTCTX_SECTION_KEYED_DATA_ASSEMBLY_METADATA =
    *mut ACTCTX_SECTION_KEYED_DATA_ASSEMBLY_METADATA;
pub type PCACTCTX_SECTION_KEYED_DATA_ASSEMBLY_METADATA =
    *const ACTCTX_SECTION_KEYED_DATA_ASSEMBLY_METADATA;
STRUCT!{struct ACTCTX_SECTION_KEYED_DATA {
    cbSize: ::ULONG,
    ulDataFormatVersion: ::ULONG,
    lpData: ::PVOID,
    ulLength: ::ULONG,
    lpSectionGlobalData: ::PVOID,
    ulSectionGlobalDataLength: ::ULONG,
    lpSectionBase: ::PVOID,
    ulSectionTotalLength: ::ULONG,
    hActCtx: ::HANDLE,
    ulAssemblyRosterIndex: ::ULONG,
    ulFlags: ::ULONG,
    AssemblyMetadata: ACTCTX_SECTION_KEYED_DATA_ASSEMBLY_METADATA,
}}
pub type PACTCTX_SECTION_KEYED_DATA = *mut ACTCTX_SECTION_KEYED_DATA;
pub type PCACTCTX_SECTION_KEYED_DATA = *const ACTCTX_SECTION_KEYED_DATA;
ENUM!{enum STREAM_INFO_LEVELS {
    FindStreamInfoStandard,
    FindStreamInfoMaxInfoLevel,
}}
ENUM!{enum PROCESS_INFORMATION_CLASS {
    ProcessMemoryPriority,
    ProcessInformationClassMax,
}}
ENUM!{enum DEP_SYSTEM_POLICY_TYPE {
    DEPPolicyAlwaysOff = 0,
    DEPPolicyAlwaysOn,
    DEPPolicyOptIn,
    DEPPolicyOptOut,
    DEPTotalPolicyCount,
}}
ENUM!{enum PIPE_ATTRIBUTE_TYPE {
    PipeAttribute,
    PipeConnectionAttribute,
    PipeHandleAttribute,
}}
pub type APPLICATION_RECOVERY_CALLBACK = Option<unsafe extern "system" fn(
    pvParameter: ::PVOID
) -> ::DWORD>;
STRUCT!{struct SYSTEM_POWER_STATUS {
    ACLineStatus: ::BYTE,
    BatteryFlag: ::BYTE,
    BatteryLifePercent: ::BYTE,
    Reserved1: ::BYTE,
    BatteryLifeTime: ::DWORD,
    BatteryFullLifeTime: ::DWORD,
}}
pub type LPSYSTEM_POWER_STATUS = *mut SYSTEM_POWER_STATUS;
pub const OFS_MAXPATHNAME: usize = 128;
STRUCT!{nodebug struct OFSTRUCT {
    cBytes: ::BYTE,
    fFixedDisk: ::BYTE,
    nErrCode: ::WORD,
    Reserved1: ::WORD,
    Reserved2: ::WORD,
    szPathName: [::CHAR; OFS_MAXPATHNAME],
}}
pub type POFSTRUCT = *mut OFSTRUCT;
pub type LPOFSTRUCT = *mut OFSTRUCT;
ENUM!{enum FILE_ID_TYPE {
    FileIdType,
    ObjectIdType,
    ExtendedFileIdType,
    MaximumFileIdType,
}}
STRUCT!{struct FILE_ID_DESCRIPTOR {
    dwSize: ::DWORD,
    Type: FILE_ID_TYPE,
    ObjectId: ::GUID,
}}
UNION!(FILE_ID_DESCRIPTOR, ObjectId, FileId, FileId_mut, ::LARGE_INTEGER);
UNION!(FILE_ID_DESCRIPTOR, ObjectId, ExtendedFileId, ExtendedFileId_mut, ::FILE_ID_128);
pub type LPFILE_ID_DESCRIPTOR = *mut FILE_ID_DESCRIPTOR;
