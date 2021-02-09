// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! FFI bindings to kernel32.
#![cfg(windows)]
extern crate winapi;
use winapi::*;
extern "system" {
    pub fn AcquireSRWLockExclusive(SRWLock: PSRWLOCK);
    pub fn AcquireSRWLockShared(SRWLock: PSRWLOCK);
    pub fn ActivateActCtx(hActCtx: HANDLE, lpCookie: *mut ULONG_PTR) -> BOOL;
    pub fn AddAtomA(lpString: LPCSTR) -> ATOM;
    pub fn AddAtomW(lpString: LPCWSTR) -> ATOM;
    pub fn AddConsoleAliasA(Source: LPSTR, Target: LPSTR, ExeName: LPSTR) -> BOOL;
    pub fn AddConsoleAliasW(Source: LPWSTR, Target: LPWSTR, ExeName: LPWSTR) -> BOOL;
    pub fn AddDllDirectory(NewDirectory: PCWSTR) -> DLL_DIRECTORY_COOKIE;
    pub fn AddIntegrityLabelToBoundaryDescriptor(
        BoundaryDescriptor: *mut HANDLE, IntegrityLabel: PSID,
    ) -> BOOL;
    // pub fn AddLocalAlternateComputerNameA();
    // pub fn AddLocalAlternateComputerNameW();
    pub fn AddRefActCtx(hActCtx: HANDLE);
    pub fn AddResourceAttributeAce(
        pAcl: PACL, dwAceRevision: DWORD, AceFlags: DWORD, AccessMask: DWORD, pSid: PSID,
        pAttributeInfo: PCLAIM_SECURITY_ATTRIBUTES_INFORMATION, pReturnLength: PDWORD,
    ) -> BOOL;
    pub fn AddSIDToBoundaryDescriptor(BoundaryDescriptor: *mut HANDLE, RequiredSid: PSID) -> BOOL;
    pub fn AddScopedPolicyIDAce(
        pAcl: PACL, dwAceRevision: DWORD, AceFlags: DWORD, AccessMask: DWORD, pSid: PSID,
    ) -> BOOL;
    pub fn AddSecureMemoryCacheCallback(pfnCallBack: PSECURE_MEMORY_CACHE_CALLBACK) -> BOOL;
    pub fn AddVectoredContinueHandler(First: ULONG, Handler: PVECTORED_EXCEPTION_HANDLER) -> PVOID;
    pub fn AddVectoredExceptionHandler(
        First: ULONG, Handler: PVECTORED_EXCEPTION_HANDLER,
    ) -> PVOID;
    pub fn AllocConsole() -> BOOL;
    pub fn AllocateUserPhysicalPages(
        hProcess: HANDLE, NumberOfPages: PULONG_PTR, PageArray: PULONG_PTR,
    ) -> BOOL;
    pub fn AllocateUserPhysicalPagesNuma(
        hProcess: HANDLE, NumberOfPages: PULONG_PTR, PageArray: PULONG_PTR, nndPreferred: DWORD,
    ) -> BOOL;
    // pub fn AppXGetOSMaxVersionTested();
    pub fn ApplicationRecoveryFinished(bSuccess: BOOL);
    pub fn ApplicationRecoveryInProgress(pbCancelled: PBOOL) -> HRESULT;
    pub fn AreFileApisANSI() -> BOOL;
    pub fn AssignProcessToJobObject(hJob: HANDLE, hProcess: HANDLE) -> BOOL;
    pub fn AttachConsole(dwProcessId: DWORD) -> BOOL;
    pub fn BackupRead(
        hFile: HANDLE, lpBuffer: LPBYTE, nNumberOfBytesToRead: DWORD, lpNumberOfBytesRead: LPDWORD,
        bAbort: BOOL, bProcessSecurity: BOOL, lpContext: *mut LPVOID,
    ) -> BOOL;
    pub fn BackupSeek(
        hFile: HANDLE, dwLowBytesToSeek: DWORD, dwHighBytesToSeek: DWORD,
        lpdwLowByteSeeked: LPDWORD, lpdwHighByteSeeked: LPDWORD, lpContext: *mut LPVOID,
    ) -> BOOL;
    pub fn BackupWrite(
        hFile: HANDLE, lpBuffer: LPBYTE, nNumberOfBytesToWrite: DWORD,
        lpNumberOfBytesWritten: LPDWORD, bAbort: BOOL, bProcessSecurity: BOOL,
        lpContext: *mut LPVOID,
    ) -> BOOL;
    // pub fn BaseSetLastNTError();
    pub fn Beep(dwFreq: DWORD, dwDuration: DWORD) -> BOOL;
    pub fn BeginUpdateResourceA(pFileName: LPCSTR, bDeleteExistingResources: BOOL) -> HANDLE;
    pub fn BeginUpdateResourceW(pFileName: LPCWSTR, bDeleteExistingResources: BOOL) -> HANDLE;
    pub fn BindIoCompletionCallback(
        FileHandle: HANDLE, Function: LPOVERLAPPED_COMPLETION_ROUTINE, Flags: ULONG,
    ) -> BOOL;
    pub fn BuildCommDCBA(lpDef: LPCSTR, lpDCB: LPDCB) -> BOOL;
    pub fn BuildCommDCBAndTimeoutsA(
        lpDef: LPCSTR, lpDCB: LPDCB, lpCommTimeouts: LPCOMMTIMEOUTS,
    ) -> BOOL;
    pub fn BuildCommDCBAndTimeoutsW(
        lpDef: LPCWSTR, lpDCB: LPDCB, lpCommTimeouts: LPCOMMTIMEOUTS,
    ) -> BOOL;
    pub fn BuildCommDCBW(lpDef: LPCWSTR, lpDCB: LPDCB) -> BOOL;
    pub fn CallNamedPipeA(
        lpNamedPipeName: LPCSTR, lpInBuffer: LPVOID, nInBufferSize: DWORD, lpOutBuffer: LPVOID,
        nOutBufferSize: DWORD, lpBytesRead: LPDWORD, nTimeOut: DWORD,
    ) -> BOOL;
    pub fn CallNamedPipeW(
        lpNamedPipeName: LPCWSTR, lpInBuffer: LPVOID, nInBufferSize: DWORD, lpOutBuffer: LPVOID,
        nOutBufferSize: DWORD, lpBytesRead: LPDWORD, nTimeOut: DWORD,
    ) -> BOOL;
    pub fn CallbackMayRunLong(pci: PTP_CALLBACK_INSTANCE) -> BOOL;
    pub fn CalloutOnFiberStack(
        lpFiber: PVOID, lpStartAddress: PFIBER_CALLOUT_ROUTINE, lpParameter: PVOID,
    ) -> PVOID;
    pub fn CancelDeviceWakeupRequest(hDevice: HANDLE) -> BOOL;
    pub fn CancelIo(hFile: HANDLE) -> BOOL;
    pub fn CancelIoEx(hFile: HANDLE, lpOverlapped: LPOVERLAPPED) -> BOOL;
    pub fn CancelSynchronousIo(hThread: HANDLE) -> BOOL;
    pub fn CancelThreadpoolIo(pio: PTP_IO);
    pub fn CancelTimerQueueTimer(TimerQueue: HANDLE, Timer: HANDLE) -> BOOL;
    pub fn CancelWaitableTimer(hTimer: HANDLE) -> BOOL;
    pub fn CeipIsOptedIn() -> BOOL;
    pub fn ChangeTimerQueueTimer(
        TimerQueue: HANDLE, Timer: HANDLE, DueTime: ULONG, Period: ULONG,
    ) -> BOOL;
    // pub fn CheckElevation();
    // pub fn CheckElevationEnabled();
    pub fn CheckNameLegalDOS8Dot3A(
        lpName: LPCSTR, lpOemName: LPSTR, OemNameSize: DWORD, pbNameContainsSpaces: PBOOL,
        pbNameLegal: PBOOL,
    ) -> BOOL;
    pub fn CheckNameLegalDOS8Dot3W(
        lpName: LPCWSTR, lpOemName: LPSTR, OemNameSize: DWORD, pbNameContainsSpaces: PBOOL,
        pbNameLegal: PBOOL,
    ) -> BOOL;
    pub fn CheckRemoteDebuggerPresent(hProcess: HANDLE, pbDebuggerPresent: PBOOL) -> BOOL;
    pub fn CheckTokenCapability(
        TokenHandle: HANDLE, CapabilitySidToCheck: PSID, HasCapability: PBOOL,
    ) -> BOOL;
    pub fn CheckTokenMembershipEx(
        TokenHandle: HANDLE, SidToCheck: PSID, Flags: DWORD, IsMember: PBOOL,
    ) -> BOOL;
    pub fn ClearCommBreak(hFile: HANDLE) -> BOOL;
    pub fn ClearCommError(hFile: HANDLE, lpErrors: LPDWORD, lpStat: LPCOMSTAT) -> BOOL;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    // pub fn ClosePackageInfo();
    pub fn ClosePrivateNamespace(Handle: HANDLE, Flags: ULONG) -> BOOLEAN;
    // pub fn CloseState();
    pub fn CloseThreadpool(ptpp: PTP_POOL);
    pub fn CloseThreadpoolCleanupGroup(ptpcg: PTP_CLEANUP_GROUP);
    pub fn CloseThreadpoolCleanupGroupMembers(
        ptpcg: PTP_CLEANUP_GROUP, fCancelPendingCallbacks: BOOL, pvCleanupContext: PVOID,
    );
    pub fn CloseThreadpoolIo(pio: PTP_IO);
    pub fn CloseThreadpoolTimer(pti: PTP_TIMER);
    pub fn CloseThreadpoolWait(pwa: PTP_WAIT);
    pub fn CloseThreadpoolWork(pwk: PTP_WORK);
    pub fn CommConfigDialogA(lpszName: LPCSTR, hWnd: HWND, lpCC: LPCOMMCONFIG) -> BOOL;
    pub fn CommConfigDialogW(lpszName: LPCWSTR, hWnd: HWND, lpCC: LPCOMMCONFIG) -> BOOL;
    pub fn CompareFileTime(lpFileTime1: *const FILETIME, lpFileTime2: *const FILETIME) -> LONG;
    pub fn CompareStringA(
        Locale: LCID, dwCmpFlags: DWORD, lpString1: PCNZCH, cchCount1: c_int, lpString2: PCNZCH,
        cchCount2: c_int,
    ) -> c_int;
    pub fn CompareStringEx(
        lpLocaleName: LPCWSTR, dwCmpFlags: DWORD, lpString1: LPCWCH, cchCount1: c_int,
        lpString2: LPCWCH, cchCount2: c_int, lpVersionInformation: LPNLSVERSIONINFO,
        lpReserved: LPVOID, lParam: LPARAM,
    ) -> c_int;
    pub fn CompareStringOrdinal(
        lpString1: LPCWCH, cchCount1: c_int, lpString2: LPCWCH, cchCount2: c_int, bIgnoreCase: BOOL,
    ) -> c_int;
    pub fn CompareStringW(
        Locale: LCID, dwCmpFlags: DWORD, lpString1: PCNZWCH, cchCount1: c_int, lpString2: PCNZWCH,
        cchCount2: c_int,
    ) -> c_int;
    pub fn ConnectNamedPipe(hNamedPipe: HANDLE, lpOverlapped: LPOVERLAPPED) -> BOOL;
    pub fn ContinueDebugEvent(
        dwProcessId: DWORD, dwThreadId: DWORD, dwContinueStatus: DWORD,
    ) -> BOOL;
    pub fn ConvertDefaultLocale(Locale: LCID) -> LCID;
    pub fn ConvertFiberToThread() -> BOOL;
    pub fn ConvertThreadToFiber(lpParameter: LPVOID) -> LPVOID;
    pub fn ConvertThreadToFiberEx(lpParameter: LPVOID, dwFlags: DWORD) -> LPVOID;
    pub fn CopyContext(Destination: PCONTEXT, ContextFlags: DWORD, Source: PCONTEXT) -> BOOL;
    pub fn CopyFile2(
        pwszExistingFileName: PCWSTR, pwszNewFileName: PCWSTR,
        pExtendedParameters: *mut COPYFILE2_EXTENDED_PARAMETERS,
    ) -> HRESULT;
    pub fn CopyFileA(
        lpExistingFileName: LPCSTR, lpNewFileName: LPCSTR, bFailIfExists: BOOL
    ) -> BOOL;
    pub fn CopyFileExA(
        lpExistingFileName: LPCSTR, lpNewFileName: LPCSTR, lpProgressRoutine: LPPROGRESS_ROUTINE,
        lpData: LPVOID, pbCancel: LPBOOL, dwCopyFlags: DWORD,
    ) -> BOOL;
    pub fn CopyFileExW(
        lpExistingFileName: LPCWSTR, lpNewFileName: LPCWSTR, lpProgressRoutine: LPPROGRESS_ROUTINE,
        lpData: LPVOID, pbCancel: LPBOOL, dwCopyFlags: DWORD,
    ) -> BOOL;
    pub fn CopyFileTransactedA(
        lpExistingFileName: LPCWSTR, lpNewFileName: LPCWSTR, lpProgressRoutine: LPPROGRESS_ROUTINE,
        lpData: LPVOID, pbCancel: LPBOOL, dwCopyFlags: DWORD, hTransaction: HANDLE,
    ) -> BOOL;
    pub fn CopyFileTransactedW(
        lpExistingFileName: LPCWSTR, lpNewFileName: LPCWSTR, lpProgressRoutine: LPPROGRESS_ROUTINE,
        lpData: LPVOID, pbCancel: LPBOOL, dwCopyFlags: DWORD, hTransaction: HANDLE,
    ) -> BOOL;
    pub fn CopyFileW(
        lpExistingFileName: LPCWSTR, lpNewFileName: LPCWSTR, bFailIfExists: BOOL
    ) -> BOOL;
    pub fn CreateActCtxA(pActCtx: PCACTCTXA) -> HANDLE;
    pub fn CreateActCtxW(pActCtx: PCACTCTXW) -> HANDLE;
    pub fn CreateBoundaryDescriptorA(Name: LPCSTR, Flags: ULONG) -> HANDLE;
    pub fn CreateBoundaryDescriptorW(Name: LPCWSTR, Flags: ULONG) -> HANDLE;
    pub fn CreateConsoleScreenBuffer(
        dwDesiredAccess: DWORD, dwShareMode: DWORD,
        lpSecurityAttributes: *const SECURITY_ATTRIBUTES, dwFlags: DWORD,
        lpScreenBufferData: LPVOID,
    ) -> HANDLE;
    pub fn CreateDirectoryA(
        lpPathName: LPCSTR, lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
    ) -> BOOL;
    pub fn CreateDirectoryExA(
        lpTemplateDirectory: LPCSTR, lpNewDirectory: LPCSTR,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
    ) -> BOOL;
    pub fn CreateDirectoryExW(
        lpTemplateDirectory: LPCWSTR, lpNewDirectory: LPCWSTR,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
    ) -> BOOL;
    pub fn CreateDirectoryTransactedA(
        lpTemplateDirectory: LPCSTR, lpNewDirectory: LPCSTR,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES, hTransaction: HANDLE,
    ) -> BOOL;
    pub fn CreateDirectoryTransactedW(
        lpTemplateDirectory: LPCWSTR, lpNewDirectory: LPCWSTR,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES, hTransaction: HANDLE,
    ) -> BOOL;
    pub fn CreateDirectoryW(
        lpPathName: LPCWSTR, lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
    ) -> BOOL;
    pub fn CreateEventA(
        lpEventAttributes: LPSECURITY_ATTRIBUTES, bManualReset: BOOL, bInitialState: BOOL,
        lpName: LPCSTR,
    ) -> HANDLE;
    pub fn CreateEventW(
        lpEventAttributes: LPSECURITY_ATTRIBUTES, bManualReset: BOOL, bInitialState: BOOL,
        lpName: LPCWSTR,
    ) -> HANDLE;
    pub fn CreateEventExA(
        lpEventAttributes: LPSECURITY_ATTRIBUTES, lpName: LPCSTR, dwFlags: DWORD,
        dwDesiredAccess: DWORD,
    ) -> HANDLE;
    pub fn CreateEventExW(
        lpEventAttributes: LPSECURITY_ATTRIBUTES, lpName: LPCWSTR, dwFlags: DWORD,
        dwDesiredAccess: DWORD,
    ) -> HANDLE;
    pub fn CreateFiber(
        dwStackSize: SIZE_T, lpStartAddress: LPFIBER_START_ROUTINE, lpParameter: LPVOID,
    ) -> LPVOID;
    pub fn CreateFiberEx(
        dwStackCommitSize: SIZE_T, dwStackReserveSize: SIZE_T, dwFlags: DWORD,
        lpStartAddress: LPFIBER_START_ROUTINE, lpParameter: LPVOID,
    ) -> LPVOID;
    pub fn CreateFile2(
        lpFileName: LPCWSTR, dwDesiredAccess: DWORD, dwShareMode: DWORD,
        dwCreationDisposition: DWORD, pCreateExParams: LPCREATEFILE2_EXTENDED_PARAMETERS,
    ) -> HANDLE;
    pub fn CreateFileA(
        lpFileName: LPCSTR, dwDesiredAccess: DWORD, dwShareMode: DWORD,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES, dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD, hTemplateFile: HANDLE,
    ) -> HANDLE;
    pub fn CreateFileMappingA(
        hFile: HANDLE, lpAttributes: LPSECURITY_ATTRIBUTES, flProtect: DWORD,
        dwMaximumSizeHigh: DWORD, dwMaximumSizeLow: DWORD, lpName: LPCSTR,
    ) -> HANDLE;
    pub fn CreateFileMappingFromApp(
        hFile: HANDLE, SecurityAttributes: PSECURITY_ATTRIBUTES, PageProtection: ULONG,
        MaximumSize: ULONG64, Name: PCWSTR,
    ) -> HANDLE;
    pub fn CreateFileMappingNumaA(
        hFile: HANDLE, lpFileMappingAttributes: LPSECURITY_ATTRIBUTES, flProtect: DWORD,
        dwMaximumSizeHigh: DWORD, dwMaximumSizeLow: DWORD, lpName: LPCSTR, nndPreferred: DWORD,
    ) -> HANDLE;
    pub fn CreateFileMappingNumaW(
        hFile: HANDLE, lpFileMappingAttributes: LPSECURITY_ATTRIBUTES, flProtect: DWORD,
        dwMaximumSizeHigh: DWORD, dwMaximumSizeLow: DWORD, lpName: LPCWSTR, nndPreferred: DWORD,
    ) -> HANDLE;
    pub fn CreateFileMappingW(
        hFile: HANDLE, lpAttributes: LPSECURITY_ATTRIBUTES, flProtect: DWORD,
        dwMaximumSizeHigh: DWORD, dwMaximumSizeLow: DWORD, lpName: LPCWSTR,
    ) -> HANDLE;
    pub fn CreateFileTransactedA(
        lpFileName: LPCSTR, dwDesiredAccess: DWORD, dwShareMode: DWORD,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES, dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD, hTemplateFile: HANDLE, hTransaction: HANDLE,
        pusMiniVersion: PUSHORT, lpExtendedParameter: PVOID,
    ) -> HANDLE;
    pub fn CreateFileTransactedW(
        lpFileName: LPCWSTR, dwDesiredAccess: DWORD, dwShareMode: DWORD,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES, dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD, hTemplateFile: HANDLE, hTransaction: HANDLE,
        pusMiniVersion: PUSHORT, lpExtendedParameter: PVOID,
    ) -> HANDLE;
    pub fn CreateFileW(
        lpFileName: LPCWSTR, dwDesiredAccess: DWORD, dwShareMode: DWORD,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES, dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD, hTemplateFile: HANDLE,
    ) -> HANDLE;
    pub fn CreateHardLinkA(
        lpFileName: LPCSTR, lpExistingFileName: LPCSTR, lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
    ) -> BOOL;
    pub fn CreateHardLinkTransactedA(
        lpFileName: LPCSTR, lpExistingFileName: LPCSTR, lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
        hTransaction: HANDLE,
    ) -> BOOL;
    pub fn CreateHardLinkTransactedW(
        lpFileName: LPCWSTR, lpExistingFileName: LPCWSTR,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES, hTransaction: HANDLE,
    );
    pub fn CreateHardLinkW(
        lpFileName: LPCWSTR, lpExistingFileName: LPCWSTR,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
    ) -> BOOL;
    pub fn CreateIoCompletionPort(
        FileHandle: HANDLE, ExistingCompletionPort: HANDLE, CompletionKey: ULONG_PTR,
        NumberOfConcurrentThreads: DWORD,
    ) -> HANDLE;
    pub fn CreateJobObjectA(lpJobAttributes: LPSECURITY_ATTRIBUTES, lpName: LPCSTR) -> HANDLE;
    pub fn CreateJobObjectW(lpJobAttributes: LPSECURITY_ATTRIBUTES, lpName: LPCWSTR) -> HANDLE;
    pub fn CreateJobSet(NumJob: ULONG, UserJobSet: PJOB_SET_ARRAY, Flags: ULONG) -> BOOL;
    pub fn CreateMailslotA(
        lpName: LPCSTR, nMaxMessageSize: DWORD, lReadTimeout: DWORD,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
    ) -> HANDLE;
    pub fn CreateMailslotW(
        lpName: LPCWSTR, nMaxMessageSize: DWORD, lReadTimeout: DWORD,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
    ) -> HANDLE;
    pub fn CreateMemoryResourceNotification(
        NotificationType: MEMORY_RESOURCE_NOTIFICATION_TYPE,
    ) -> HANDLE;
    pub fn CreateMutexA(
        lpMutexAttributes: LPSECURITY_ATTRIBUTES, bInitialOwner: BOOL, lpName: LPCSTR,
    ) -> HANDLE;
    pub fn CreateMutexExA(
        lpMutexAttributes: LPSECURITY_ATTRIBUTES, lpName: LPCSTR, dwFlags: DWORD,
        dwDesiredAccess: DWORD,
    ) -> HANDLE;
    pub fn CreateMutexExW(
        lpMutexAttributes: LPSECURITY_ATTRIBUTES, lpName: LPCWSTR, dwFlags: DWORD,
        dwDesiredAccess: DWORD,
    ) -> HANDLE;
    pub fn CreateMutexW(
        lpMutexAttributes: LPSECURITY_ATTRIBUTES, bInitialOwner: BOOL, lpName: LPCWSTR,
    ) -> HANDLE;
    pub fn CreateNamedPipeA(
        lpName: LPCSTR, dwOpenMode: DWORD, dwPipeMode: DWORD, nMaxInstances: DWORD,
        nOutBufferSize: DWORD, nInBufferSize: DWORD, nDefaultTimeOut: DWORD,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
    ) -> HANDLE;
    pub fn CreateNamedPipeW(
        lpName: LPCWSTR, dwOpenMode: DWORD, dwPipeMode: DWORD, nMaxInstances: DWORD,
        nOutBufferSize: DWORD, nInBufferSize: DWORD, nDefaultTimeOut: DWORD,
        lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
    ) -> HANDLE;
    pub fn CreatePipe(
        hReadPipe: PHANDLE, hWritePipe: PHANDLE, lpPipeAttributes: LPSECURITY_ATTRIBUTES,
        nSize: DWORD,
    ) -> BOOL;
    pub fn CreatePrivateNamespaceA(
        lpPrivateNamespaceAttributes: LPSECURITY_ATTRIBUTES, lpBoundaryDescriptor: LPVOID,
        lpAliasPrefix: LPCSTR,
    ) -> HANDLE;
    pub fn CreatePrivateNamespaceW(
        lpPrivateNamespaceAttributes: LPSECURITY_ATTRIBUTES, lpBoundaryDescriptor: LPVOID,
        lpAliasPrefix: LPCWSTR,
    ) -> HANDLE;
    pub fn CreateProcessA(
        lpApplicationName: LPCSTR, lpCommandLine: LPSTR, lpProcessAttributes: LPSECURITY_ATTRIBUTES,
        lpThreadAttributes: LPSECURITY_ATTRIBUTES, bInheritHandles: BOOL, dwCreationFlags: DWORD,
        lpEnvironment: LPVOID, lpCurrentDirectory: LPCSTR, lpStartupInfo: LPSTARTUPINFOA,
        lpProcessInformation: LPPROCESS_INFORMATION,
    ) -> BOOL;
    pub fn CreateProcessW(
        lpApplicationName: LPCWSTR, lpCommandLine: LPWSTR,
        lpProcessAttributes: LPSECURITY_ATTRIBUTES, lpThreadAttributes: LPSECURITY_ATTRIBUTES,
        bInheritHandles: BOOL, dwCreationFlags: DWORD, lpEnvironment: LPVOID,
        lpCurrentDirectory: LPCWSTR, lpStartupInfo: LPSTARTUPINFOW,
        lpProcessInformation: LPPROCESS_INFORMATION,
    ) -> BOOL;
    pub fn CreateRemoteThread(
        hProcess: HANDLE, lpThreadAttributes: LPSECURITY_ATTRIBUTES, dwStackSize: SIZE_T,
        lpStartAddress: LPTHREAD_START_ROUTINE, lpParameter: LPVOID, dwCreationFlags: DWORD,
        lpThreadId: LPDWORD,
    ) -> HANDLE;
    pub fn CreateRemoteThreadEx(
        hProcess: HANDLE, lpThreadAttributes: LPSECURITY_ATTRIBUTES, dwStackSize: SIZE_T,
        lpStartAddress: LPTHREAD_START_ROUTINE, lpParameter: LPVOID, dwCreationFlags: DWORD,
        lpAttributeList: LPPROC_THREAD_ATTRIBUTE_LIST, lpThreadId: LPDWORD,
    ) -> HANDLE;
    pub fn CreateSemaphoreA(
        lpSemaphoreAttributes: LPSECURITY_ATTRIBUTES, lInitialCount: LONG, lMaximumCount: LONG,
        lpName: LPCSTR,
    ) -> HANDLE;
    pub fn CreateSemaphoreExA(
        lpSemaphoreAttributes: LPSECURITY_ATTRIBUTES, lInitialCount: LONG, lMaximumCount: LONG,
        lpName: LPCSTR, dwFlags: DWORD, dwDesiredAccess: DWORD,
    ) -> HANDLE;
    pub fn CreateSemaphoreExW(
        lpSemaphoreAttributes: LPSECURITY_ATTRIBUTES, lInitialCount: LONG, lMaximumCount: LONG,
        lpName: LPCWSTR, dwFlags: DWORD, dwDesiredAccess: DWORD,
    ) -> HANDLE;
    pub fn CreateSemaphoreW(
        lpSemaphoreAttributes: LPSECURITY_ATTRIBUTES, lInitialCount: LONG, lMaximumCount: LONG,
        lpName: LPCWSTR,
    ) -> HANDLE;
    pub fn CreateSymbolicLinkA(
        lpSymlinkFileName: LPCSTR, lpTargetFileName: LPCSTR, dwFlags: DWORD,
    ) -> BOOLEAN;
    pub fn CreateSymbolicLinkTransactedA(
        lpSymlinkFileName: LPCSTR, lpTargetFileName: LPCSTR, dwFlags: DWORD, hTransaction: HANDLE,
    ) -> BOOLEAN;
    pub fn CreateSymbolicLinkTransactedW(
        lpSymlinkFileName: LPCWSTR, lpTargetFileName: LPCWSTR, dwFlags: DWORD, hTransaction: HANDLE,
    ) -> BOOLEAN;
    pub fn CreateSymbolicLinkW(
        lpSymlinkFileName: LPCWSTR, lpTargetFileName: LPCWSTR, dwFlags: DWORD,
    ) -> BOOLEAN;
    pub fn CreateTapePartition(
        hDevice: HANDLE, dwPartitionMethod: DWORD, dwCount: DWORD, dwSize: DWORD,
    ) -> DWORD;
    pub fn CreateThread(
        lpThreadAttributes: LPSECURITY_ATTRIBUTES, dwStackSize: SIZE_T,
        lpStartAddress: LPTHREAD_START_ROUTINE, lpParameter: LPVOID, dwCreationFlags: DWORD,
        lpThreadId: LPDWORD,
    ) -> HANDLE;
    pub fn CreateThreadpool(reserved: PVOID) -> PTP_POOL;
    pub fn CreateThreadpoolCleanupGroup() -> PTP_CLEANUP_GROUP;
    pub fn CreateThreadpoolIo(
        fl: HANDLE, pfnio: PTP_WIN32_IO_CALLBACK, pv: PVOID, pcbe: PTP_CALLBACK_ENVIRON,
    ) -> PTP_IO;
    pub fn CreateThreadpoolTimer(
        pfnti: PTP_TIMER_CALLBACK, pv: PVOID, pcbe: PTP_CALLBACK_ENVIRON,
    ) -> PTP_TIMER;
    pub fn CreateThreadpoolWait(
        pfnwa: PTP_WAIT_CALLBACK, pv: PVOID, pcbe: PTP_CALLBACK_ENVIRON,
    ) -> PTP_WAIT;
    pub fn CreateThreadpoolWork(
        pfnwk: PTP_WORK_CALLBACK, pv: PVOID, pcbe: PTP_CALLBACK_ENVIRON,
    ) -> PTP_WORK;
    pub fn CreateTimerQueue() -> HANDLE;
    pub fn CreateTimerQueueTimer(
        phNewTimer: PHANDLE, TimerQueue: HANDLE, Callback: WAITORTIMERCALLBACK, Parameter: PVOID,
        DueTime: DWORD, Period: DWORD, Flags: ULONG,
    ) -> BOOL;
    pub fn CreateToolhelp32Snapshot(dwFlags: DWORD, th32ProcessID: DWORD) -> HANDLE;
    #[cfg(target_arch = "x86_64")]
    pub fn CreateUmsCompletionList(UmsCompletionList: *mut PUMS_COMPLETION_LIST) -> BOOL;
    #[cfg(target_arch = "x86_64")]
    pub fn CreateUmsThreadContext(lpUmsThread: *mut PUMS_CONTEXT) -> BOOL;
    pub fn CreateWaitableTimerA(
        lpTimerAttributes: LPSECURITY_ATTRIBUTES, bManualReset: BOOL, lpTimerName: LPCSTR,
    ) -> HANDLE;
    pub fn CreateWaitableTimerExA(
        lpTimerAttributes: LPSECURITY_ATTRIBUTES, lpTimerName: LPCSTR, dwFlags: DWORD,
        dwDesiredAccess: DWORD,
    ) -> HANDLE;
    pub fn CreateWaitableTimerExW(
        lpTimerAttributes: LPSECURITY_ATTRIBUTES, lpTimerName: LPCWSTR, dwFlags: DWORD,
        dwDesiredAccess: DWORD,
    ) -> HANDLE;
    pub fn CreateWaitableTimerW(
        lpTimerAttributes: LPSECURITY_ATTRIBUTES, bManualReset: BOOL, lpTimerName: LPCWSTR,
    ) -> HANDLE;
    // pub fn CtrlRoutine();
    pub fn DeactivateActCtx(dwFlags: DWORD, ulCookie: ULONG_PTR) -> BOOL;
    pub fn DebugActiveProcess(dwProcessId: DWORD) -> BOOL;
    pub fn DebugActiveProcessStop(dwProcessId: DWORD) -> BOOL;
    pub fn DebugBreak();
    pub fn DebugBreakProcess(Process: HANDLE) -> BOOL;
    pub fn DebugSetProcessKillOnExit(KillOnExit: BOOL) -> BOOL;
    pub fn DecodePointer(Ptr: PVOID) -> PVOID;
    pub fn DecodeSystemPointer(Ptr: PVOID) -> PVOID;
    pub fn DefineDosDeviceA(dwFlags: DWORD, lpDeviceName: LPCSTR, lpTargetPath: LPCSTR) -> BOOL;
    pub fn DefineDosDeviceW(dwFlags: DWORD, lpDeviceName: LPCWSTR, lpTargetPath: LPCWSTR) -> BOOL;
    pub fn DelayLoadFailureHook(pszDllName: LPCSTR, pszProcName: LPCSTR) -> FARPROC;
    pub fn DeleteAtom(nAtom: ATOM) -> ATOM;
    pub fn DeleteBoundaryDescriptor(BoundaryDescriptor: HANDLE);
    pub fn DeleteCriticalSection(lpCriticalSection: LPCRITICAL_SECTION);
    pub fn DeleteFiber(lpFiber: LPVOID);
    pub fn DeleteFileA(lpFileName: LPCSTR) -> BOOL;
    pub fn DeleteFileTransactedA(lpFileName: LPCSTR, hTransaction: HANDLE) -> BOOL;
    pub fn DeleteFileTransactedW(lpFileName: LPCWSTR, hTransaction: HANDLE) -> BOOL;
    pub fn DeleteFileW(lpFileName: LPCWSTR) -> BOOL;
    pub fn DeleteProcThreadAttributeList(lpAttributeList: LPPROC_THREAD_ATTRIBUTE_LIST);
    pub fn DeleteSynchronizationBarrier(lpBarrier: LPSYNCHRONIZATION_BARRIER) -> BOOL;
    pub fn DeleteTimerQueue(TimerQueue: HANDLE) -> BOOL;
    pub fn DeleteTimerQueueEx(TimerQueue: HANDLE, CompletionEvent: HANDLE) -> BOOL;
    pub fn DeleteTimerQueueTimer(
        TimerQueue: HANDLE, Timer: HANDLE, CompletionEvent: HANDLE,
    ) -> BOOL;
    #[cfg(target_arch = "x86_64")]
    pub fn DeleteUmsCompletionList(UmsCompletionList: PUMS_COMPLETION_LIST) -> BOOL;
    #[cfg(target_arch = "x86_64")]
    pub fn DeleteUmsThreadContext(UmsThread: PUMS_CONTEXT) -> BOOL;
    pub fn DeleteVolumeMountPointA(lpszVolumeMountPoint: LPCSTR) -> BOOL;
    pub fn DeleteVolumeMountPointW(lpszVolumeMountPoint: LPCWSTR) -> BOOL;
    #[cfg(target_arch = "x86_64")]
    pub fn DequeueUmsCompletionListItems(
        UmsCompletionList: PUMS_COMPLETION_LIST, WaitTimeOut: DWORD,
        UmsThreadList: *mut PUMS_CONTEXT,
    ) -> BOOL;
    pub fn DeviceIoControl(
        hDevice: HANDLE, dwIoControlCode: DWORD, lpInBuffer: LPVOID, nInBufferSize: DWORD,
        lpOutBuffer: LPVOID, nOutBufferSize: DWORD, lpBytesReturned: LPDWORD,
        lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
    pub fn DisableThreadLibraryCalls(hLibModule: HMODULE) -> BOOL;
    pub fn DisableThreadProfiling(PerformanceDataHandle: HANDLE) -> DWORD;
    pub fn DisassociateCurrentThreadFromCallback(pci: PTP_CALLBACK_INSTANCE);
    pub fn DisconnectNamedPipe(hNamedPipe: HANDLE) -> BOOL;
    pub fn DnsHostnameToComputerNameA(
        Hostname: LPCSTR, ComputerName: LPCSTR, nSize: LPDWORD,
    ) -> BOOL;
    pub fn DnsHostnameToComputerNameExW(
        Hostname: LPCWSTR, ComputerName: LPWSTR, nSize: LPDWORD,
    ) -> BOOL;
    pub fn DnsHostnameToComputerNameW(
        Hostname: LPCWSTR, ComputerName: LPWSTR, nSize: LPDWORD,
    ) -> BOOL;
    pub fn DosDateTimeToFileTime(wFatDate: WORD, wFatTime: WORD, lpFileTime: LPFILETIME) -> BOOL;
    // pub fn DosPathToSessionPathW();
    pub fn DuplicateHandle(
        hSourceProcessHandle: HANDLE, hSourceHandle: HANDLE, hTargetProcessHandle: HANDLE,
        lpTargetHandle: LPHANDLE, dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwOptions: DWORD,
    ) -> BOOL;
    pub fn EnableThreadProfiling(
        ThreadHandle: HANDLE, Flags: DWORD, HardwareCounters: DWORD64,
        PerformanceDataHandle: *mut HANDLE,
    ) -> BOOL;
    pub fn EncodePointer(Ptr: PVOID) -> PVOID;
    pub fn EncodeSystemPointer(Ptr: PVOID) -> PVOID;
    pub fn EndUpdateResourceA(hUpdate: HANDLE, fDiscard: BOOL) -> BOOL;
    pub fn EndUpdateResourceW(hUpdate: HANDLE, fDiscard: BOOL) -> BOOL;
    pub fn EnterCriticalSection(lpCriticalSection: LPCRITICAL_SECTION);
    pub fn EnterSynchronizationBarrier(
        lpBarrier: LPSYNCHRONIZATION_BARRIER, dwFlags: DWORD,
    ) -> BOOL;
    #[cfg(target_arch = "x86_64")]
    pub fn EnterUmsSchedulingMode(SchedulerStartupInfo: PUMS_SCHEDULER_STARTUP_INFO) -> BOOL;
    pub fn EnumCalendarInfoA(
        lpCalInfoEnumProc: CALINFO_ENUMPROCA, Locale: LCID, Calendar: CALID, CalType: CALTYPE,
    ) -> BOOL;
    pub fn EnumCalendarInfoExA(
        lpCalInfoEnumProcEx: CALINFO_ENUMPROCEXA, Locale: LCID, Calendar: CALID, CalType: CALTYPE,
    ) -> BOOL;
    pub fn EnumCalendarInfoExEx(
        pCalInfoEnumProcExEx: CALINFO_ENUMPROCEXEX, lpLocaleName: LPCWSTR, Calendar: CALID,
        lpReserved: LPCWSTR, CalType: CALTYPE, lParam: LPARAM,
    ) -> BOOL;
    pub fn EnumCalendarInfoExW(
        lpCalInfoEnumProcEx: CALINFO_ENUMPROCEXW, Locale: LCID, Calendar: CALID, CalType: CALTYPE,
    ) -> BOOL;
    pub fn EnumCalendarInfoW(
        lpCalInfoEnumProc: CALINFO_ENUMPROCW, Locale: LCID, Calendar: CALID, CalType: CALTYPE,
    ) -> BOOL;
    pub fn EnumDateFormatsA(
        lpDateFmtEnumProc: DATEFMT_ENUMPROCA, Locale: LCID, dwFlags: DWORD,
    ) -> BOOL;
    pub fn EnumDateFormatsExA(
        lpDateFmtEnumProcEx: DATEFMT_ENUMPROCEXA, Locale: LCID, dwFlags: DWORD,
    ) -> BOOL;
    pub fn EnumDateFormatsExEx(
        lpDateFmtEnumProcExEx: DATEFMT_ENUMPROCEXEX, lpLocaleName: LPCWSTR, dwFlags: DWORD,
        lParam: LPARAM,
    ) -> BOOL;
    pub fn EnumDateFormatsExW(
        lpDateFmtEnumProcEx: DATEFMT_ENUMPROCEXW, Locale: LCID, dwFlags: DWORD,
    ) -> BOOL;
    pub fn EnumDateFormatsW(
        lpDateFmtEnumProc: DATEFMT_ENUMPROCW, Locale: LCID, dwFlags: DWORD,
    ) -> BOOL;
    pub fn EnumLanguageGroupLocalesA(
        lpLangGroupLocaleEnumProc: LANGGROUPLOCALE_ENUMPROCA, LanguageGroup: LGRPID, dwFlags: DWORD,
        lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumLanguageGroupLocalesW(
        lpLangGroupLocaleEnumProc: LANGGROUPLOCALE_ENUMPROCW, LanguageGroup: LGRPID, dwFlags: DWORD,
        lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumResourceLanguagesA(
        hModule: HMODULE, lpType: LPCSTR, lpName: LPCSTR, lpEnumFunc: ENUMRESLANGPROCA,
        lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumResourceLanguagesExA(
        hModule: HMODULE, lpType: LPCSTR, lpName: LPCSTR, lpEnumFunc: ENUMRESLANGPROCA,
        lParam: LONG_PTR, dwFlags: DWORD, LangId: LANGID,
    ) -> BOOL;
    pub fn EnumResourceLanguagesExW(
        hModule: HMODULE, lpType: LPCWSTR, lpName: LPCWSTR, lpEnumFunc: ENUMRESLANGPROCW,
        lParam: LONG_PTR, dwFlags: DWORD, LangId: LANGID,
    ) -> BOOL;
    pub fn EnumResourceLanguagesW(
        hModule: HMODULE, lpType: LPCWSTR, lpName: LPCWSTR, lpEnumFunc: ENUMRESLANGPROCW,
        lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumResourceNamesA(
        hModule: HMODULE, lpType: LPCSTR, lpEnumFunc: ENUMRESNAMEPROCA, lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumResourceNamesExA(
        hModule: HMODULE, lpType: LPCSTR, lpEnumFunc: ENUMRESNAMEPROCA, lParam: LONG_PTR,
        dwFlags: DWORD, LangId: LANGID,
    ) -> BOOL;
    pub fn EnumResourceNamesExW(
        hModule: HMODULE, lpType: LPCWSTR, lpEnumFunc: ENUMRESNAMEPROCW, lParam: LONG_PTR,
        dwFlags: DWORD, LangId: LANGID,
    ) -> BOOL;
    pub fn EnumResourceNamesW(
        hModule: HMODULE, lpType: LPCWSTR, lpEnumFunc: ENUMRESNAMEPROCW, lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumResourceTypesA(
        hModule: HMODULE, lpEnumFunc: ENUMRESTYPEPROCA, lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumResourceTypesExA(
        hModule: HMODULE, lpEnumFunc: ENUMRESTYPEPROCA, lParam: LONG_PTR, dwFlags: DWORD,
        LangId: LANGID,
    ) -> BOOL;
    pub fn EnumResourceTypesExW(
        hModule: HMODULE, lpEnumFunc: ENUMRESTYPEPROCW, lParam: LONG_PTR, dwFlags: DWORD,
        LangId: LANGID,
    ) -> BOOL;
    pub fn EnumResourceTypesW(
        hModule: HMODULE, lpEnumFunc: ENUMRESTYPEPROCW, lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumSystemCodePagesA(lpCodePageEnumProc: CODEPAGE_ENUMPROCA, dwFlags: DWORD) -> BOOL;
    pub fn EnumSystemCodePagesW(lpCodePageEnumProc: CODEPAGE_ENUMPROCW, dwFlags: DWORD) -> BOOL;
    pub fn EnumSystemFirmwareTables(
        FirmwareTableProviderSignature: DWORD, pFirmwareTableEnumBuffer: PVOID, BufferSize: DWORD,
    ) -> UINT;
    pub fn EnumSystemGeoID(
        GeoClass: GEOCLASS, ParentGeoId: GEOID, lpGeoEnumProc: GEO_ENUMPROC,
    ) -> BOOL;
    pub fn EnumSystemLanguageGroupsA(
        lpLanguageGroupEnumProc: LANGUAGEGROUP_ENUMPROCA, dwFlags: DWORD, lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumSystemLanguageGroupsW(
        lpLanguageGroupEnumProc: LANGUAGEGROUP_ENUMPROCW, dwFlags: DWORD, lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumSystemLocalesA(lpLocaleEnumProc: LOCALE_ENUMPROCA, dwFlags: DWORD) -> BOOL;
    pub fn EnumSystemLocalesEx(
        lpLocaleEnumProcEx: LOCALE_ENUMPROCEX, dwFlags: DWORD, lParam: LPARAM, lpReserved: LPVOID,
    ) -> BOOL;
    pub fn EnumSystemLocalesW(lpLocaleEnumProc: LOCALE_ENUMPROCW, dwFlags: DWORD) -> BOOL;
    pub fn EnumTimeFormatsA(
        lpTimeFmtEnumProc: TIMEFMT_ENUMPROCA, Locale: LCID, dwFlags: DWORD,
    ) -> BOOL;
    pub fn EnumTimeFormatsEx(
        lpTimeFmtEnumProcEx: TIMEFMT_ENUMPROCEX, lpLocaleName: LPCWSTR, dwFlags: DWORD,
        lParam: LPARAM,
    ) -> BOOL;
    pub fn EnumTimeFormatsW(
        lpTimeFmtEnumProc: TIMEFMT_ENUMPROCW, Locale: LCID, dwFlags: DWORD,
    ) -> BOOL;
    pub fn EnumUILanguagesA(
        lpUILanguageEnumProc: UILANGUAGE_ENUMPROCA, dwFlags: DWORD, lParam: LONG_PTR,
    ) -> BOOL;
    pub fn EnumUILanguagesW(
        lpUILanguageEnumProc: UILANGUAGE_ENUMPROCW, dwFlags: DWORD, lParam: LONG_PTR,
    ) -> BOOL;
    // pub fn EnumerateLocalComputerNamesA();
    // pub fn EnumerateLocalComputerNamesW();
    pub fn EraseTape(hDevice: HANDLE, dwEraseType: DWORD, bImmediate: BOOL) -> DWORD;
    pub fn EscapeCommFunction(hFile: HANDLE, dwFunc: DWORD) -> BOOL;
    #[cfg(target_arch = "x86_64")]
    pub fn ExecuteUmsThread(UmsThread: PUMS_CONTEXT) -> BOOL;
    pub fn ExitProcess(uExitCode: UINT);
    pub fn ExitThread(dwExitCode: DWORD);
    pub fn ExpandEnvironmentStringsA(lpSrc: LPCSTR, lpDst: LPSTR, nSize: DWORD) -> DWORD;
    pub fn ExpandEnvironmentStringsW(lpSrc: LPCWSTR, lpDst: LPWSTR, nSize: DWORD) -> DWORD;
    pub fn FatalAppExitA(uAction: UINT, lpMessageText: LPCSTR);
    pub fn FatalAppExitW(uAction: UINT, lpMessageText: LPCWSTR);
    pub fn FatalExit(ExitCode: c_int);
    pub fn FileTimeToDosDateTime(
        lpFileTime: *const FILETIME, lpFatDate: LPWORD, lpFatTime: LPWORD,
    ) -> BOOL;
    pub fn FileTimeToLocalFileTime(
        lpFileTime: *const FILETIME, lpLocalFileTime: LPFILETIME,
    ) -> BOOL;
    pub fn FileTimeToSystemTime(
        lpFileTime: *const FILETIME, lpSystemTime: LPSYSTEMTIME,
    ) -> BOOL;
    pub fn FillConsoleOutputAttribute(
        hConsoleOutput: HANDLE, wAttribute: WORD, nLength: DWORD, dwWriteCoord: COORD,
        lpNumberOfAttrsWritten: LPDWORD,
    ) -> BOOL;
    pub fn FillConsoleOutputCharacterA(
        hConsoleOutput: HANDLE, cCharacter: CHAR, nLength: DWORD, dwWriteCoord: COORD,
        lpNumberOfCharsWritten: LPDWORD,
    ) -> BOOL;
    pub fn FillConsoleOutputCharacterW(
        hConsoleOutput: HANDLE, cCharacter: WCHAR, nLength: DWORD, dwWriteCoord: COORD,
        lpNumberOfCharsWritten: LPDWORD,
    ) -> BOOL;
    pub fn FindActCtxSectionGuid(
        dwFlags: DWORD, lpExtensionGuid: *const GUID, ulSectionId: ULONG, lpGuidToFind: *const GUID,
        ReturnedData: PACTCTX_SECTION_KEYED_DATA,
    ) -> BOOL;
    pub fn FindActCtxSectionStringA(
        dwFlags: DWORD, lpExtensionGuid: *const GUID, ulSectionId: ULONG, lpStringToFind: LPCSTR,
        ReturnedData: PACTCTX_SECTION_KEYED_DATA,
    ) -> BOOL;
    pub fn FindActCtxSectionStringW(
        dwFlags: DWORD, lpExtensionGuid: *const GUID, ulSectionId: ULONG, lpStringToFind: LPCWSTR,
        ReturnedData: PACTCTX_SECTION_KEYED_DATA,
    ) -> BOOL;
    pub fn FindAtomA(lpString: LPCSTR) -> ATOM;
    pub fn FindAtomW(lpString: LPCWSTR) -> ATOM;
    pub fn FindClose(hFindFile: HANDLE) -> BOOL;
    pub fn FindCloseChangeNotification(hChangeHandle: HANDLE) -> BOOL;
    pub fn FindFirstChangeNotificationA(
        lpPathName: LPCSTR, bWatchSubtree: BOOL, dwNotifyFilter: DWORD,
    ) -> HANDLE;
    pub fn FindFirstChangeNotificationW(
        lpPathName: LPCWSTR, bWatchSubtree: BOOL, dwNotifyFilter: DWORD,
    ) -> HANDLE;
    pub fn FindFirstFileA(lpFileName: LPCSTR, lpFindFileData: LPWIN32_FIND_DATAA) -> HANDLE;
    pub fn FindFirstFileExA(
        lpFileName: LPCSTR, fInfoLevelId: FINDEX_INFO_LEVELS, lpFindFileData: LPVOID,
        fSearchOp: FINDEX_SEARCH_OPS, lpSearchFilter: LPVOID, dwAdditionalFlags: DWORD,
    ) -> HANDLE;
    pub fn FindFirstFileExW(
        lpFileName: LPCWSTR, fInfoLevelId: FINDEX_INFO_LEVELS, lpFindFileData: LPVOID,
        fSearchOp: FINDEX_SEARCH_OPS, lpSearchFilter: LPVOID, dwAdditionalFlags: DWORD,
    ) -> HANDLE;
    pub fn FindFirstFileNameTransactedW(
        lpFileName: LPCWSTR, dwFlags: DWORD, StringLength: LPDWORD, LinkName: PWSTR,
        hTransaction: HANDLE,
    ) -> HANDLE;
    pub fn FindFirstFileNameW(
        lpFileName: LPCWSTR, dwFlags: DWORD, StringLength: LPDWORD, LinkName: PWSTR,
    ) -> HANDLE;
    pub fn FindFirstFileTransactedA(
        lpFileName: LPCSTR, fInfoLevelId: FINDEX_INFO_LEVELS, lpFindFileData: LPVOID,
        fSearchOp: FINDEX_SEARCH_OPS, lpSearchFilter: LPVOID, dwAdditionalFlags: DWORD,
        hTransaction: HANDLE,
    ) -> HANDLE;
    pub fn FindFirstFileTransactedW(
        lpFileName: LPCWSTR, fInfoLevelId: FINDEX_INFO_LEVELS, lpFindFileData: LPVOID,
        fSearchOp: FINDEX_SEARCH_OPS, lpSearchFilter: LPVOID, dwAdditionalFlags: DWORD,
        hTransaction: HANDLE,
    ) -> HANDLE;
    pub fn FindFirstFileW(lpFileName: LPCWSTR, lpFindFileData: LPWIN32_FIND_DATAW) -> HANDLE;
    pub fn FindFirstStreamTransactedW(
        lpFileName: LPCWSTR, InfoLevel: STREAM_INFO_LEVELS, lpFindStreamData: LPVOID,
        dwFlags: DWORD, hTransaction: HANDLE,
    ) -> HANDLE;
    pub fn FindFirstStreamW(
        lpFileName: LPCWSTR, InfoLevel: STREAM_INFO_LEVELS, lpFindStreamData: LPVOID,
        dwFlags: DWORD,
    ) -> HANDLE;
    pub fn FindFirstVolumeA(lpszVolumeName: LPSTR, cchBufferLength: DWORD) -> HANDLE;
    pub fn FindFirstVolumeMountPointA(
        lpszRootPathName: LPCSTR, lpszVolumeMountPoint: LPSTR, cchBufferLength: DWORD,
    ) -> HANDLE;
    pub fn FindFirstVolumeMountPointW(
        lpszRootPathName: LPCWSTR, lpszVolumeMountPoint: LPWSTR, cchBufferLength: DWORD,
    ) -> HANDLE;
    pub fn FindFirstVolumeW(lpszVolumeName: LPWSTR, cchBufferLength: DWORD) -> HANDLE;
    pub fn FindNLSString(
        Locale: LCID, dwFindNLSStringFlags: DWORD, lpStringSource: LPCWSTR, cchSource: c_int,
        lpStringValue: LPCWSTR, cchValue: c_int, pcchFound: LPINT,
    ) -> c_int;
    pub fn FindNLSStringEx(
        lpLocaleName: LPCWSTR, dwFindNLSStringFlags: DWORD, lpStringSource: LPCWSTR,
        cchSource: c_int, lpStringValue: LPCWSTR, cchValue: c_int, pcchFound: LPINT,
        lpVersionInformation: LPNLSVERSIONINFO, lpReserved: LPVOID, sortHandle: LPARAM,
    ) -> c_int;
    pub fn FindNextChangeNotification(hChangeHandle: HANDLE) -> BOOL;
    pub fn FindNextFileA(hFindFile: HANDLE, lpFindFileData: LPWIN32_FIND_DATAA) -> BOOL;
    pub fn FindNextFileNameW(hFindStream: HANDLE, StringLength: LPDWORD, LinkName: PWSTR) -> BOOL;
    pub fn FindNextFileW(hFindFile: HANDLE, lpFindFileData: LPWIN32_FIND_DATAW) -> BOOL;
    pub fn FindNextStreamW(hFindStream: HANDLE, lpFindStreamData: LPVOID) -> BOOL;
    pub fn FindNextVolumeA(
        hFindVolume: HANDLE, lpszVolumeName: LPSTR, cchBufferLength: DWORD,
    ) -> BOOL;
    pub fn FindNextVolumeMountPointA(
        hFindVolumeMountPoint: HANDLE, lpszVolumeMountPoint: LPSTR, cchBufferLength: DWORD,
    ) -> BOOL;
    pub fn FindNextVolumeMountPointW(
        hFindVolumeMountPoint: HANDLE, lpszVolumeMountPoint: LPWSTR, cchBufferLength: DWORD,
    ) -> BOOL;
    pub fn FindNextVolumeW(
        hFindVolume: HANDLE, lpszVolumeName: LPWSTR, cchBufferLength: DWORD,
    ) -> BOOL;
    // pub fn FindPackagesByPackageFamily();
    pub fn FindResourceA(hModule: HMODULE, lpName: LPCSTR, lpType: LPCSTR) -> HRSRC;
    pub fn FindResourceExA(
        hModule: HMODULE, lpName: LPCSTR, lpType: LPCSTR, wLanguage: WORD,
    ) -> HRSRC;
    pub fn FindResourceExW(
        hModule: HMODULE, lpName: LPCWSTR, lpType: LPCWSTR, wLanguage: WORD,
    ) -> HRSRC;
    pub fn FindResourceW(hModule: HMODULE, lpName: LPCWSTR, lpType: LPCWSTR) -> HRSRC;
    pub fn FindStringOrdinal(
        dwFindStringOrdinalFlags: DWORD, lpStringSource: LPCWSTR, cchSource: c_int,
        lpStringValue: LPCWSTR, cchValue: c_int, bIgnoreCase: BOOL,
    ) -> c_int;
    pub fn FindVolumeClose(hFindVolume: HANDLE) -> BOOL;
    pub fn FindVolumeMountPointClose(hFindVolumeMountPoint: HANDLE) -> BOOL;
    pub fn FlsAlloc(lpCallback: PFLS_CALLBACK_FUNCTION) -> DWORD;
    pub fn FlsFree(dwFlsIndex: DWORD) -> BOOL;
    pub fn FlsGetValue(dwFlsIndex: DWORD) -> PVOID;
    pub fn FlsSetValue(dwFlsIndex: DWORD, lpFlsData: PVOID) -> BOOL;
    pub fn FlushConsoleInputBuffer(hConsoleInput: HANDLE) -> BOOL;
    pub fn FlushFileBuffers(hFile: HANDLE) -> BOOL;
    pub fn FlushInstructionCache(hProcess: HANDLE, lpBaseAddress: LPCVOID, dwSize: SIZE_T) -> BOOL;
    pub fn FlushProcessWriteBuffers();
    pub fn FlushViewOfFile(lpBaseAddress: LPCVOID, dwNumberOfBytesToFlush: SIZE_T) -> BOOL;
    pub fn FoldStringA(
        dwMapFlags: DWORD, lpSrcStr: LPCSTR, cchSrc: c_int, lpDestStr: LPSTR, cchDest: c_int,
    ) -> c_int;
    pub fn FoldStringW(
        dwMapFlags: DWORD, lpSrcStr: LPCWCH, cchSrc: c_int, lpDestStr: LPWSTR, cchDest: c_int,
    ) -> c_int;
    // pub fn FormatApplicationUserModelId();
    pub fn FormatMessageA(
        dwFlags: DWORD, lpSource: LPCVOID, dwMessageId: DWORD, dwLanguageId: DWORD,
        lpBuffer: LPSTR, nSize: DWORD, Arguments: *mut va_list,
    ) -> DWORD;
    pub fn FormatMessageW(
        dwFlags: DWORD, lpSource: LPCVOID, dwMessageId: DWORD, dwLanguageId: DWORD,
        lpBuffer: LPWSTR, nSize: DWORD, Arguments: *mut va_list,
    ) -> DWORD;
    pub fn FreeConsole() -> BOOL;
    pub fn FreeEnvironmentStringsA(penv: LPCH) -> BOOL;
    pub fn FreeEnvironmentStringsW(penv: LPWCH) -> BOOL;
    pub fn FreeLibrary(hLibModule: HMODULE) -> BOOL;
    pub fn FreeLibraryAndExitThread(hLibModule: HMODULE, dwExitCode: DWORD);
    pub fn FreeLibraryWhenCallbackReturns(pci: PTP_CALLBACK_INSTANCE, module: HMODULE);
    pub fn FreeResource(hResData: HGLOBAL) -> BOOL;
    pub fn FreeUserPhysicalPages(
        hProcess: HANDLE, NumberOfPages: PULONG_PTR, PageArray: PULONG_PTR,
    ) -> BOOL;
    pub fn GenerateConsoleCtrlEvent(dwCtrlEvent: DWORD, dwProcessGroupId: DWORD) -> BOOL;
    pub fn GetACP() -> UINT;
    pub fn GetActiveProcessorCount(GroupNumber: WORD) -> DWORD;
    pub fn GetActiveProcessorGroupCount() -> WORD;
    pub fn GetAppContainerAce(
        Acl: PACL, StartingAceIndex: DWORD, AppContainerAce: *mut PVOID,
        AppContainerAceIndex: *mut DWORD,
    ) -> BOOL;
    pub fn GetAppContainerNamedObjectPath(
        Token: HANDLE, AppContainerSid: PSID, ObjectPathLength: ULONG, ObjectPath: LPWSTR,
        ReturnLength: PULONG,
    ) -> BOOL;
    pub fn GetApplicationRecoveryCallback(
        hProcess: HANDLE, pRecoveryCallback: *mut APPLICATION_RECOVERY_CALLBACK,
        ppvParameter: *mut PVOID, pdwPingInterval: PDWORD, pdwFlags: PDWORD,
    ) -> HRESULT;
    pub fn GetApplicationRestartSettings(
        hProcess: HANDLE, pwzCommandline: PWSTR, pcchSize: PDWORD, pdwFlags: PDWORD,
    ) -> HRESULT;
    // pub fn GetApplicationUserModelId();
    pub fn GetAtomNameA(nAtom: ATOM, lpBuffer: LPSTR, nSize: c_int) -> UINT;
    pub fn GetAtomNameW(nAtom: ATOM, lpBuffer: LPWSTR, nSize: c_int) -> UINT;
    pub fn GetBinaryTypeA(lpApplicationName: LPCSTR, lpBinaryType: LPDWORD) -> BOOL;
    pub fn GetBinaryTypeW(lpApplicationName: LPCWSTR, lpBinaryType: LPDWORD) -> BOOL;
    pub fn GetCPInfo(CodePage: UINT, lpCPInfo: LPCPINFO) -> BOOL;
    pub fn GetCPInfoExA(CodePage: UINT, dwFlags: DWORD, lpCPInfoEx: LPCPINFOEXA) -> BOOL;
    pub fn GetCPInfoExW(CodePage: UINT, dwFlags: DWORD, lpCPInfoEx: LPCPINFOEXW) -> BOOL;
    pub fn GetCachedSigningLevel(
        File: HANDLE, Flags: PULONG, SigningLevel: PULONG, Thumbprint: PUCHAR,
        ThumbprintSize: PULONG, ThumbprintAlgorithm: PULONG,
    ) -> BOOL;
    pub fn GetCalendarInfoA(
        Locale: LCID, Calendar: CALID, CalType: CALTYPE, lpCalData: LPSTR, cchData: c_int,
        lpValue: LPDWORD,
    ) -> c_int;
    pub fn GetCalendarInfoEx(
        lpLocaleName: LPCWSTR, Calendar: CALID, lpReserved: LPCWSTR, CalType: CALTYPE,
        lpCalData: LPWSTR, cchData: c_int, lpValue: LPDWORD,
    ) -> c_int;
    pub fn GetCalendarInfoW(
        Locale: LCID, Calendar: CALID, CalType: CALTYPE, lpCalData: LPWSTR, cchData: c_int,
        lpValue: LPDWORD,
    ) -> c_int;
    pub fn GetCommConfig(hCommDev: HANDLE, lpCC: LPCOMMCONFIG, lpdwSize: LPDWORD) -> BOOL;
    pub fn GetCommMask(hFile: HANDLE, lpEvtMask: LPDWORD) -> BOOL;
    pub fn GetCommModemStatus(hFile: HANDLE, lpModemStat: LPDWORD) -> BOOL;
    pub fn GetCommProperties(hFile: HANDLE, lpCommProp: LPCOMMPROP) -> BOOL;
    pub fn GetCommState(hFile: HANDLE, lpDCB: LPDCB) -> BOOL;
    pub fn GetCommTimeouts(hFile: HANDLE, lpCommTimeouts: LPCOMMTIMEOUTS) -> BOOL;
    pub fn GetCommandLineA() -> LPSTR;
    pub fn GetCommandLineW() -> LPWSTR;
    pub fn GetCompressedFileSizeA(lpFileName: LPCSTR, lpFileSizeHigh: LPDWORD) -> DWORD;
    pub fn GetCompressedFileSizeTransactedA(
        lpFileName: LPCSTR, lpFileSizeHigh: LPDWORD, hTransaction: HANDLE,
    ) -> DWORD;
    pub fn GetCompressedFileSizeTransactedW(
        lpFileName: LPCWSTR, lpFileSizeHigh: LPDWORD, hTransaction: HANDLE,
    );
    pub fn GetCompressedFileSizeW(lpFileName: LPCWSTR, lpFileSizeHigh: LPDWORD) -> DWORD;
    pub fn GetComputerNameA(lpBuffer: LPSTR, nSize: LPDWORD) -> BOOL;
    pub fn GetComputerNameExA(
        NameType: COMPUTER_NAME_FORMAT, lpBuffer: LPSTR, nSize: LPDWORD,
    ) -> BOOL;
    pub fn GetComputerNameExW(
        NameType: COMPUTER_NAME_FORMAT, lpBuffer: LPWSTR, nSize: LPDWORD,
    ) -> BOOL;
    pub fn GetComputerNameW(lpBuffer: LPWSTR, nSize: LPDWORD) -> BOOL;
    pub fn GetConsoleAliasA(
        Source: LPSTR, TargetBuffer: LPSTR, TargetBufferLength: DWORD, ExeName: LPSTR,
    ) -> DWORD;
    pub fn GetConsoleAliasExesA(ExeNameBuffer: LPSTR, ExeNameBufferLength: DWORD) -> DWORD;
    pub fn GetConsoleAliasExesLengthA() -> DWORD;
    pub fn GetConsoleAliasExesLengthW() -> DWORD;
    pub fn GetConsoleAliasExesW(ExeNameBuffer: LPWSTR, ExeNameBufferLength: DWORD) -> DWORD;
    pub fn GetConsoleAliasW(
        Source: LPWSTR, TargetBuffer: LPWSTR, TargetBufferLength: DWORD, ExeName: LPWSTR,
    ) -> DWORD;
    pub fn GetConsoleAliasesA(
        AliasBuffer: LPSTR, AliasBufferLength: DWORD, ExeName: LPSTR,
    ) -> DWORD;
    pub fn GetConsoleAliasesLengthA(ExeName: LPSTR) -> DWORD;
    pub fn GetConsoleAliasesLengthW(ExeName: LPWSTR) -> DWORD;
    pub fn GetConsoleAliasesW(
        AliasBuffer: LPWSTR, AliasBufferLength: DWORD, ExeName: LPWSTR,
    ) -> DWORD;
    pub fn GetConsoleCP() -> UINT;
    pub fn GetConsoleCursorInfo(
        hConsoleOutput: HANDLE, lpConsoleCursorInfo: PCONSOLE_CURSOR_INFO,
    ) -> BOOL;
    pub fn GetConsoleDisplayMode(lpModeFlags: LPDWORD) -> BOOL;
    pub fn GetConsoleFontSize(hConsoleOutput: HANDLE, nFont: DWORD) -> COORD;
    pub fn GetConsoleHistoryInfo(lpConsoleHistoryInfo: PCONSOLE_HISTORY_INFO) -> BOOL;
    pub fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: LPDWORD) -> BOOL;
    pub fn GetConsoleOriginalTitleA(lpConsoleTitle: LPSTR, nSize: DWORD) -> DWORD;
    pub fn GetConsoleOriginalTitleW(lpConsoleTitle: LPWSTR, nSize: DWORD) -> DWORD;
    pub fn GetConsoleOutputCP() -> UINT;
    pub fn GetConsoleProcessList(lpdwProcessList: LPDWORD, dwProcessCount: DWORD) -> DWORD;
    pub fn GetConsoleScreenBufferInfo(
        hConsoleOutput: HANDLE, lpConsoleScreenBufferInfo: PCONSOLE_SCREEN_BUFFER_INFO,
    ) -> BOOL;
    pub fn GetConsoleScreenBufferInfoEx(
        hConsoleOutput: HANDLE, lpConsoleScreenBufferInfoEx: PCONSOLE_SCREEN_BUFFER_INFOEX,
    ) -> BOOL;
    pub fn GetConsoleSelectionInfo(lpConsoleSelectionInfo: PCONSOLE_SELECTION_INFO) -> BOOL;
    pub fn GetConsoleTitleA(lpConsoleTitle: LPSTR, nSize: DWORD) -> DWORD;
    pub fn GetConsoleTitleW(lpConsoleTitle: LPWSTR, nSize: DWORD) -> DWORD;
    pub fn GetConsoleWindow() -> HWND;
    pub fn GetCurrencyFormatA(
        Locale: LCID, dwFlags: DWORD, lpValue: LPCSTR, lpFormat: *const CURRENCYFMTA,
        lpCurrencyStr: LPSTR, cchCurrency: c_int,
    ) -> c_int;
    pub fn GetCurrencyFormatEx(
        lpLocaleName: LPCWSTR, dwFlags: DWORD, lpValue: LPCWSTR, lpFormat: *const CURRENCYFMTW,
        lpCurrencyStr: LPWSTR, cchCurrency: c_int,
    ) -> c_int;
    pub fn GetCurrencyFormatW(
        Locale: LCID, dwFlags: DWORD, lpValue: LPCWSTR, lpFormat: *const CURRENCYFMTW,
        lpCurrencyStr: LPWSTR, cchCurrency: c_int,
    ) -> c_int;
    pub fn GetCurrentActCtx(lphActCtx: *mut HANDLE) -> BOOL;
    // pub fn GetCurrentApplicationUserModelId();
    pub fn GetCurrentConsoleFont(
        hConsoleOutput: HANDLE, bMaximumWindow: BOOL, lpConsoleCurrentFont: PCONSOLE_FONT_INFO,
    ) -> BOOL;
    pub fn GetCurrentConsoleFontEx(
        hConsoleOutput: HANDLE, bMaximumWindow: BOOL, lpConsoleCurrentFontEx: PCONSOLE_FONT_INFOEX,
    ) -> BOOL;
    pub fn GetCurrentDirectoryA(nBufferLength: DWORD, lpBuffer: LPSTR) -> DWORD;
    pub fn GetCurrentDirectoryW(nBufferLength: DWORD, lpBuffer: LPWSTR) -> DWORD;
    // pub fn GetCurrentPackageFamilyName();
    // pub fn GetCurrentPackageFullName();
    // pub fn GetCurrentPackageId();
    // pub fn GetCurrentPackageInfo();
    // pub fn GetCurrentPackagePath();
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn GetCurrentProcessId() -> DWORD;
    pub fn GetCurrentProcessorNumber() -> DWORD;
    pub fn GetCurrentProcessorNumberEx(ProcNumber: PPROCESSOR_NUMBER);
    pub fn GetCurrentThread() -> HANDLE;
    pub fn GetCurrentThreadId() -> DWORD;
    pub fn GetCurrentThreadStackLimits(LowLimit: PULONG_PTR, HighLimit: PULONG_PTR);
    #[cfg(target_arch = "x86_64")]
    pub fn GetCurrentUmsThread() -> PUMS_CONTEXT;
    pub fn GetDateFormatA(
        Locale: LCID, dwFlags: DWORD, lpDate: *const SYSTEMTIME, lpFormat: LPCSTR, lpDateStr: LPSTR,
        cchDate: c_int,
    ) -> c_int;
    pub fn GetDateFormatEx(
        lpLocaleName: LPCWSTR, dwFlags: DWORD, lpDate: *const SYSTEMTIME, lpFormat: LPCWSTR,
        lpDateStr: LPWSTR, cchDate: c_int, lpCalendar: LPCWSTR,
    ) -> c_int;
    pub fn GetDateFormatW(
        Locale: LCID, dwFlags: DWORD, lpDate: *const SYSTEMTIME, lpFormat: LPCWSTR,
        lpDateStr: LPWSTR, cchDate: c_int,
    ) -> c_int;
    pub fn GetDefaultCommConfigA(lpszName: LPCSTR, lpCC: LPCOMMCONFIG, lpdwSize: LPDWORD) -> BOOL;
    pub fn GetDefaultCommConfigW(lpszName: LPCWSTR, lpCC: LPCOMMCONFIG, lpdwSize: LPDWORD) -> BOOL;
    pub fn GetDevicePowerState(hDevice: HANDLE, pfOn: *mut BOOL) -> BOOL;
    pub fn GetDiskFreeSpaceA(
        lpRootPathName: LPCSTR, lpSectorsPerCluster: LPDWORD, lpBytesPerSector: LPDWORD,
        lpNumberOfFreeClusters: LPDWORD, lpTotalNumberOfClusters: LPDWORD,
    ) -> BOOL;
    pub fn GetDiskFreeSpaceExA(
        lpDirectoryName: LPCSTR, lpFreeBytesAvailableToCaller: PULARGE_INTEGER,
        lpTotalNumberOfBytes: PULARGE_INTEGER, lpTotalNumberOfFreeBytes: PULARGE_INTEGER,
    ) -> BOOL;
    pub fn GetDiskFreeSpaceExW(
        lpDirectoryName: LPCWSTR, lpFreeBytesAvailableToCaller: PULARGE_INTEGER,
        lpTotalNumberOfBytes: PULARGE_INTEGER, lpTotalNumberOfFreeBytes: PULARGE_INTEGER,
    ) -> BOOL;
    pub fn GetDiskFreeSpaceW(
        lpRootPathName: LPCWSTR, lpSectorsPerCluster: LPDWORD, lpBytesPerSector: LPDWORD,
        lpNumberOfFreeClusters: LPDWORD, lpTotalNumberOfClusters: LPDWORD,
    ) -> BOOL;
    pub fn GetDllDirectoryA(nBufferLength: DWORD, lpBuffer: LPSTR) -> DWORD;
    pub fn GetDllDirectoryW(nBufferLength: DWORD, lpBuffer: LPWSTR) -> DWORD;
    pub fn GetDriveTypeA(lpRootPathName: LPCSTR) -> UINT;
    pub fn GetDriveTypeW(lpRootPathName: LPCWSTR) -> UINT;
    pub fn GetDurationFormat(
        Locale: LCID, dwFlags: DWORD, lpDuration: *const SYSTEMTIME, ullDuration: ULONGLONG,
        lpFormat: LPCWSTR, lpDurationStr: LPWSTR, cchDuration: c_int,
    ) -> c_int;
    pub fn GetDurationFormatEx(
        lpLocaleName: LPCWSTR, dwFlags: DWORD, lpDuration: *const SYSTEMTIME,
        ullDuration: ULONGLONG, lpFormat: LPCWSTR, lpDurationStr: LPWSTR, cchDuration: c_int,
    ) -> c_int;
    pub fn GetDynamicTimeZoneInformation(
        pTimeZoneInformation: PDYNAMIC_TIME_ZONE_INFORMATION,
    ) -> DWORD;
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn GetEnabledXStateFeatures() -> DWORD64;
    pub fn GetEnvironmentStrings() -> LPCH;
    pub fn GetEnvironmentStringsW() -> LPWCH;
    pub fn GetEnvironmentVariableA(lpName: LPCSTR, lpBuffer: LPSTR, nSize: DWORD) -> DWORD;
    pub fn GetEnvironmentVariableW(lpName: LPCWSTR, lpBuffer: LPWSTR, nSize: DWORD) -> DWORD;
    // pub fn GetEraNameCountedString();
    pub fn GetErrorMode() -> UINT;
    pub fn GetExitCodeProcess(hProcess: HANDLE, lpExitCode: LPDWORD) -> BOOL;
    pub fn GetExitCodeThread(hThread: HANDLE, lpExitCode: LPDWORD) -> BOOL;
    pub fn GetFileAttributesA(lpFileName: LPCSTR) -> DWORD;
    pub fn GetFileAttributesExA(
        lpFileName: LPCSTR, fInfoLevelId: GET_FILEEX_INFO_LEVELS, lpFileInformation: LPVOID,
    ) -> BOOL;
    pub fn GetFileAttributesExW(
        lpFileName: LPCWSTR, fInfoLevelId: GET_FILEEX_INFO_LEVELS, lpFileInformation: LPVOID,
    ) -> BOOL;
    pub fn GetFileAttributesTransactedA(
        lpFileName: LPCSTR, fInfoLevelId: GET_FILEEX_INFO_LEVELS, lpFileInformation: LPVOID,
        hTransaction: HANDLE,
    ) -> BOOL;
    pub fn GetFileAttributesTransactedW(
        lpFileName: LPCWSTR, fInfoLevelId: GET_FILEEX_INFO_LEVELS, lpFileInformation: LPVOID,
        hTransaction: HANDLE,
    ) -> BOOL;
    pub fn GetFileAttributesW(lpFileName: LPCWSTR) -> DWORD;
    pub fn GetFileBandwidthReservation(
        hFile: HANDLE, lpPeriodMilliseconds: LPDWORD, lpBytesPerPeriod: LPDWORD,
        pDiscardable: LPBOOL, lpTransferSize: LPDWORD, lpNumOutstandingRequests: LPDWORD,
    ) -> BOOL;
    pub fn GetFileInformationByHandle(
        hFile: HANDLE, lpFileInformation: LPBY_HANDLE_FILE_INFORMATION,
    ) -> BOOL;
    pub fn GetFileInformationByHandleEx(
        hFile: HANDLE, FileInformationClass: FILE_INFO_BY_HANDLE_CLASS, lpFileInformation: LPVOID,
        dwBufferSize: DWORD,
    ) -> BOOL;
    pub fn GetFileMUIInfo(
        dwFlags: DWORD, pcwszFilePath: PCWSTR, pFileMUIInfo: PFILEMUIINFO,
        pcbFileMUIInfo: *mut DWORD,
    ) -> BOOL;
    pub fn GetFileMUIPath(
        dwFlags: DWORD, pcwszFilePath: PCWSTR, pwszLanguage: PWSTR, pcchLanguage: PULONG,
        pwszFileMUIPath: PWSTR, pcchFileMUIPath: PULONG, pululEnumerator: PULONGLONG,
    ) -> BOOL;
    pub fn GetFileSize(hFile: HANDLE, lpFileSizeHigh: LPDWORD) -> DWORD;
    pub fn GetFileSizeEx(hFile: HANDLE, lpFileSize: PLARGE_INTEGER) -> BOOL;
    pub fn GetFileTime(
        hFile: HANDLE, lpCreationTime: LPFILETIME, lpLastAccessTime: LPFILETIME,
        lpLastWriteTime: LPFILETIME,
    ) -> BOOL;
    pub fn GetFileType(hFile: HANDLE) -> DWORD;
    pub fn GetFinalPathNameByHandleA(
        hFile: HANDLE, lpszFilePath: LPSTR, cchFilePath: DWORD, dwFlags: DWORD,
    ) -> DWORD;
    pub fn GetFinalPathNameByHandleW(
        hFile: HANDLE, lpszFilePath: LPWSTR, cchFilePath: DWORD, dwFlags: DWORD,
    ) -> DWORD;
    pub fn GetFirmwareEnvironmentVariableA(
        lpName: LPCSTR, lpGuid: LPCSTR, pBuffer: PVOID, nSize: DWORD,
    ) -> DWORD;
    pub fn GetFirmwareEnvironmentVariableExA(
        lpName: LPCSTR, lpGuid: LPCSTR, pBuffer: PVOID, nSize: DWORD, pdwAttribubutes: PDWORD,
    ) -> DWORD;
    pub fn GetFirmwareEnvironmentVariableExW(
        lpName: LPCWSTR, lpGuid: LPCWSTR, pBuffer: PVOID, nSize: DWORD, pdwAttribubutes: PDWORD,
    ) -> DWORD;
    pub fn GetFirmwareEnvironmentVariableW(
        lpName: LPCWSTR, lpGuid: LPCWSTR, pBuffer: PVOID, nSize: DWORD,
    ) -> DWORD;
    pub fn GetFirmwareType(FirmwareType: PFIRMWARE_TYPE) -> BOOL;
    pub fn GetFullPathNameA(
        lpFileName: LPCSTR, nBufferLength: DWORD, lpBuffer: LPSTR, lpFilePart: *mut LPSTR,
    ) -> DWORD;
    pub fn GetFullPathNameTransactedA(
        lpFileName: LPCSTR, nBufferLength: DWORD, lpBuffer: LPSTR, lpFilePart: *mut LPSTR,
        hTransaction: HANDLE,
    ) -> DWORD;
    pub fn GetFullPathNameTransactedW(
        lpFileName: LPCWSTR, nBufferLength: DWORD, lpBuffer: LPWSTR, lpFilePart: *mut LPWSTR,
        hTransaction: HANDLE,
    );
    pub fn GetFullPathNameW(
        lpFileName: LPCWSTR, nBufferLength: DWORD, lpBuffer: LPWSTR, lpFilePart: *mut LPWSTR,
    ) -> DWORD;
    pub fn GetGeoInfoA(
        Location: GEOID, GeoType: GEOTYPE, lpGeoData: LPSTR, cchData: c_int, LangId: LANGID,
    ) -> c_int;
    pub fn GetGeoInfoW(
        Location: GEOID, GeoType: GEOTYPE, lpGeoData: LPWSTR, cchData: c_int, LangId: LANGID,
    ) -> c_int;
    pub fn GetHandleInformation(hObject: HANDLE, lpdwFlags: LPDWORD) -> BOOL;
    pub fn GetLargePageMinimum() -> SIZE_T;
    pub fn GetLargestConsoleWindowSize(hConsoleOutput: HANDLE) -> COORD;
    pub fn GetLastError() -> DWORD;
    pub fn GetLocalTime(lpSystemTime: LPSYSTEMTIME);
    pub fn GetLocaleInfoA(
        Locale: LCID, LCType: LCTYPE, lpLCData: LPSTR, cchData: c_int,
    ) -> c_int;
    pub fn GetLocaleInfoEx(
        lpLocaleName: LPCWSTR, LCType: LCTYPE, lpLCData: LPWSTR, cchData: c_int,
    ) -> c_int;
    pub fn GetLocaleInfoW(
        Locale: LCID, LCType: LCTYPE, lpLCData: LPWSTR, cchData: c_int,
    ) -> c_int;
    pub fn GetLogicalDriveStringsA(nBufferLength: DWORD, lpBuffer: LPSTR) -> DWORD;
    pub fn GetLogicalDriveStringsW(nBufferLength: DWORD, lpBuffer: LPWSTR) -> DWORD;
    pub fn GetLogicalDrives() -> DWORD;
    pub fn GetLogicalProcessorInformation(
        Buffer: PSYSTEM_LOGICAL_PROCESSOR_INFORMATION, ReturnedLength: PDWORD,
    ) -> BOOL;
    pub fn GetLogicalProcessorInformationEx(
        RelationshipType: LOGICAL_PROCESSOR_RELATIONSHIP,
        Buffer: PSYSTEM_LOGICAL_PROCESSOR_INFORMATION,
        ReturnedLength: PDWORD,
    ) -> BOOL;
    pub fn GetLongPathNameA(lpszShortPath: LPCSTR, lpszLongPath: LPSTR, cchBuffer: DWORD) -> DWORD;
    pub fn GetLongPathNameTransactedA(
        lpszShortPath: LPCSTR, lpszLongPath: LPSTR, cchBuffer: DWORD, hTransaction: HANDLE,
    ) -> DWORD;
    pub fn GetLongPathNameTransactedW(
        lpszShortPath: LPCWSTR, lpszLongPath: LPWSTR, cchBuffer: DWORD, hTransaction: HANDLE,
    ) -> DWORD;
    pub fn GetLongPathNameW(
        lpszShortPath: LPCWSTR, lpszLongPath: LPWSTR, cchBuffer: DWORD,
    ) -> DWORD;
    pub fn GetMailslotInfo(
        hMailslot: HANDLE, lpMaxMessageSize: LPDWORD, lpNextSize: LPDWORD, lpMessageCount: LPDWORD,
        lpReadTimeout: LPDWORD,
    ) -> BOOL;
    pub fn GetMaximumProcessorCount(GroupNumber: WORD) -> DWORD;
    pub fn GetMaximumProcessorGroupCount() -> WORD;
    pub fn GetMemoryErrorHandlingCapabilities(Capabilities: PULONG) -> BOOL;
    pub fn GetModuleFileNameA(
        hModule: HMODULE, lpFilename: LPSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn GetModuleFileNameW(
        hModule: HMODULE, lpFilename: LPWSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn GetModuleHandleA(lpModuleName: LPCSTR) -> HMODULE;
    pub fn GetModuleHandleExA(
        dwFlags: DWORD, lpModuleName: LPCSTR, phModule: *mut HMODULE,
    ) -> BOOL;
    pub fn GetModuleHandleExW(
        dwFlags: DWORD, lpModuleName: LPCWSTR, phModule: *mut HMODULE,
    ) -> BOOL;
    pub fn GetModuleHandleW(lpModuleName: LPCWSTR) -> HMODULE;
    pub fn GetNLSVersion(
        Function: NLS_FUNCTION, Locale: LCID, lpVersionInformation: LPNLSVERSIONINFO,
    ) -> BOOL;
    pub fn GetNLSVersionEx(
        function: NLS_FUNCTION, lpLocaleName: LPCWSTR, lpVersionInformation: LPNLSVERSIONINFOEX,
    ) -> BOOL;
    // pub fn GetNamedPipeAttribute();
    pub fn GetNamedPipeClientComputerNameA(
        Pipe: HANDLE, ClientComputerName: LPSTR, ClientComputerNameLength: ULONG,
    ) -> BOOL;
    pub fn GetNamedPipeClientComputerNameW(
        Pipe: HANDLE, ClientComputerName: LPWSTR, ClientComputerNameLength: ULONG,
    ) -> BOOL;
    pub fn GetNamedPipeClientProcessId(Pipe: HANDLE, ClientProcessId: PULONG) -> BOOL;
    pub fn GetNamedPipeClientSessionId(Pipe: HANDLE, ClientSessionId: PULONG) -> BOOL;
    pub fn GetNamedPipeHandleStateA(
        hNamedPipe: HANDLE, lpState: LPDWORD, lpCurInstances: LPDWORD,
        lpMaxCollectionCount: LPDWORD, lpCollectDataTimeout: LPDWORD, lpUserName: LPSTR,
        nMaxUserNameSize: DWORD,
    ) -> BOOL;
    pub fn GetNamedPipeHandleStateW(
        hNamedPipe: HANDLE, lpState: LPDWORD, lpCurInstances: LPDWORD,
        lpMaxCollectionCount: LPDWORD, lpCollectDataTimeout: LPDWORD, lpUserName: LPWSTR,
        nMaxUserNameSize: DWORD,
    ) -> BOOL;
    pub fn GetNamedPipeInfo(
        hNamedPipe: HANDLE, lpFlags: LPDWORD, lpOutBufferSize: LPDWORD, lpInBufferSize: LPDWORD,
        lpMaxInstances: LPDWORD,
    ) -> BOOL;
    pub fn GetNamedPipeServerProcessId(Pipe: HANDLE, ServerProcessId: PULONG) -> BOOL;
    pub fn GetNamedPipeServerSessionId(Pipe: HANDLE, ServerSessionId: PULONG) -> BOOL;
    pub fn GetNativeSystemInfo(lpSystemInfo: LPSYSTEM_INFO);
    #[cfg(target_arch = "x86_64")]
    pub fn GetNextUmsListItem(UmsContext: PUMS_CONTEXT) -> PUMS_CONTEXT;
    pub fn GetNumaAvailableMemoryNode(Node: UCHAR, AvailableBytes: PULONGLONG) -> BOOL;
    pub fn GetNumaAvailableMemoryNodeEx(Node: USHORT, AvailableBytes: PULONGLONG) -> BOOL;
    pub fn GetNumaHighestNodeNumber(HighestNodeNumber: PULONG) -> BOOL;
    pub fn GetNumaNodeNumberFromHandle(hFile: HANDLE, NodeNumber: PUSHORT) -> BOOL;
    pub fn GetNumaNodeProcessorMask(Node: UCHAR, ProcessorMask: PULONGLONG) -> BOOL;
    pub fn GetNumaNodeProcessorMaskEx(Node: USHORT, ProcessorMask: PGROUP_AFFINITY) -> BOOL;
    pub fn GetNumaProcessorNode(Processor: UCHAR, NodeNumber: PUCHAR) -> BOOL;
    pub fn GetNumaProcessorNodeEx(Processor: PPROCESSOR_NUMBER, NodeNumber: PUSHORT) -> BOOL;
    pub fn GetNumaProximityNode(ProximityId: ULONG, NodeNumber: PUCHAR) -> BOOL;
    pub fn GetNumaProximityNodeEx(ProximityId: ULONG, NodeNumber: PUSHORT) -> BOOL;
    pub fn GetNumberFormatA(
        Locale: LCID, dwFlags: DWORD, lpValue: LPCSTR, lpFormat: *const NUMBERFMTA,
        lpNumberStr: LPSTR, cchNumber: c_int,
    ) -> c_int;
    pub fn GetNumberFormatEx(
        lpLocaleName: LPCWSTR, dwFlags: DWORD, lpValue: LPCWSTR, lpFormat: *const NUMBERFMTW,
        lpNumberStr: LPWSTR, cchNumber: c_int,
    ) -> c_int;
    pub fn GetNumberFormatW(
        Locale: LCID, dwFlags: DWORD, lpValue: LPCWSTR, lpFormat: *const NUMBERFMTW,
        lpNumberStr: LPWSTR, cchNumber: c_int,
    ) -> c_int;
    pub fn GetNumberOfConsoleInputEvents(hConsoleInput: HANDLE, lpNumberOfEvents: LPDWORD) -> BOOL;
    pub fn GetNumberOfConsoleMouseButtons(lpNumberOfMouseButtons: LPDWORD) -> BOOL;
    pub fn GetOEMCP() -> UINT;
    pub fn GetOverlappedResult(
        hFile: HANDLE, lpOverlapped: LPOVERLAPPED, lpNumberOfBytesTransferred: LPDWORD, bWait: BOOL,
    ) -> BOOL;
    pub fn GetOverlappedResultEx(
        hFile: HANDLE, lpOverlapped: LPOVERLAPPED, lpNumberOfBytesTransferred: LPDWORD,
        dwMilliseconds: DWORD, bAlertable: BOOL,
    ) -> BOOL;
    // pub fn GetPackageApplicationIds();
    // pub fn GetPackageFamilyName();
    // pub fn GetPackageFullName();
    // pub fn GetPackageId();
    // pub fn GetPackageInfo();
    // pub fn GetPackagePath();
    // pub fn GetPackagePathByFullName();
    // pub fn GetPackagesByPackageFamily();
    pub fn GetPhysicallyInstalledSystemMemory(TotalMemoryInKilobytes: PULONGLONG) -> BOOL;
    pub fn GetPriorityClass(hProcess: HANDLE) -> DWORD;
    pub fn GetPrivateProfileIntA(
        lpAppName: LPCSTR, lpKeyName: LPCSTR, nDefault: INT, lpFileName: LPCSTR,
    ) -> UINT;
    pub fn GetPrivateProfileIntW(
        lpAppName: LPCWSTR, lpKeyName: LPCWSTR, nDefault: INT, lpFileName: LPCWSTR,
    ) -> UINT;
    pub fn GetPrivateProfileSectionA(
        lpAppName: LPCSTR, lpReturnedString: LPSTR, nSize: DWORD, lpFileName: LPCSTR,
    ) -> DWORD;
    pub fn GetPrivateProfileSectionNamesA(
        lpszReturnBuffer: LPSTR, nSize: DWORD, lpFileName: LPCSTR,
    ) -> DWORD;
    pub fn GetPrivateProfileSectionNamesW(
        lpszReturnBuffer: LPWSTR, nSize: DWORD, lpFileName: LPCWSTR,
    ) -> DWORD;
    pub fn GetPrivateProfileSectionW(
        lpAppName: LPCWSTR, lpReturnedString: LPWSTR, nSize: DWORD, lpFileName: LPCWSTR,
    ) -> DWORD;
    pub fn GetPrivateProfileStringA(
        lpAppName: LPCSTR, lpKeyName: LPCSTR, lpDefault: LPCSTR, lpReturnedString: LPSTR,
        nSize: DWORD, lpFileName: LPCSTR,
    ) -> DWORD;
    pub fn GetPrivateProfileStringW(
        lpAppName: LPCWSTR, lpKeyName: LPCWSTR, lpDefault: LPCWSTR, lpReturnedString: LPWSTR,
        nSize: DWORD, lpFileName: LPCWSTR,
    ) -> DWORD;
    pub fn GetPrivateProfileStructA(
        lpszSection: LPCSTR, lpszKey: LPCSTR, lpStruct: LPVOID, uSizeStruct: UINT, szFile: LPCSTR,
    ) -> BOOL;
    pub fn GetPrivateProfileStructW(
        lpszSection: LPCWSTR, lpszKey: LPCWSTR, lpStruct: LPVOID, uSizeStruct: UINT,
        szFile: LPCWSTR,
    ) -> BOOL;
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: LPCSTR) -> FARPROC;
    pub fn GetProcessAffinityMask(
        hProcess: HANDLE, lpProcessAffinityMask: PDWORD_PTR, lpSystemAffinityMask: PDWORD_PTR,
    ) -> BOOL;
    pub fn GetProcessDEPPolicy(hProcess: HANDLE, lpFlags: LPDWORD, lpPermanent: PBOOL) -> BOOL;
    pub fn GetProcessGroupAffinity(
        hProcess: HANDLE, GroupCount: PUSHORT, GroupArray: PUSHORT,
    ) -> BOOL;
    pub fn GetProcessHandleCount(hProcess: HANDLE, pdwHandleCount: PDWORD) -> BOOL;
    pub fn GetProcessHeap() -> HANDLE;
    pub fn GetProcessHeaps(NumberOfHeaps: DWORD, ProcessHeaps: PHANDLE) -> DWORD;
    pub fn GetProcessId(Process: HANDLE) -> DWORD;
    pub fn GetProcessIdOfThread(Thread: HANDLE) -> DWORD;
    pub fn GetProcessInformation(
        hProcess: HANDLE, ProcessInformationClass: PROCESS_INFORMATION_CLASS,
        ProcessInformation: LPVOID, ProcessInformationSize: DWORD,
    ) -> BOOL;
    pub fn GetProcessIoCounters(hProcess: HANDLE, lpIoCounters: PIO_COUNTERS) -> BOOL;
    pub fn GetProcessMitigationPolicy(
        hProcess: HANDLE, MitigationPolicy: PROCESS_MITIGATION_POLICY, lpBuffer: PVOID,
        dwLength: SIZE_T,
    ) -> BOOL;
    pub fn GetProcessPreferredUILanguages(
        dwFlags: DWORD, pulNumLanguages: PULONG, pwszLanguagesBuffer: PZZWSTR,
        pcchLanguagesBuffer: PULONG,
    ) -> BOOL;
    pub fn GetProcessPriorityBoost(hProcess: HANDLE, pDisablePriorityBoost: PBOOL) -> BOOL;
    pub fn GetProcessShutdownParameters(lpdwLevel: LPDWORD, lpdwFlags: LPDWORD) -> BOOL;
    pub fn GetProcessTimes(
        hProcess: HANDLE, lpCreationTime: LPFILETIME, lpExitTime: LPFILETIME,
        lpKernelTime: LPFILETIME, lpUserTime: LPFILETIME,
    ) -> BOOL;
    pub fn GetProcessVersion(ProcessId: DWORD) -> DWORD;
    pub fn GetProcessWorkingSetSize(
        hProcess: HANDLE, lpMinimumWorkingSetSize: PSIZE_T, lpMaximumWorkingSetSize: PSIZE_T,
    ) -> BOOL;
    pub fn GetProcessWorkingSetSizeEx(
        hProcess: HANDLE, lpMinimumWorkingSetSize: PSIZE_T, lpMaximumWorkingSetSize: PSIZE_T,
        Flags: PDWORD,
    ) -> BOOL;
    pub fn GetProcessorSystemCycleTime(
        Group: USHORT, Buffer: PSYSTEM_PROCESSOR_CYCLE_TIME_INFORMATION, ReturnedLength: PDWORD,
    ) -> BOOL;
    pub fn GetProductInfo(
        dwOSMajorVersion: DWORD, dwOSMinorVersion: DWORD, dwSpMajorVersion: DWORD,
        dwSpMinorVersion: DWORD, pdwReturnedProductType: PDWORD,
    ) -> BOOL;
    pub fn GetProfileIntA(lpAppName: LPCSTR, lpKeyName: LPCSTR, nDefault: INT) -> UINT;
    pub fn GetProfileIntW(lpAppName: LPCWSTR, lpKeyName: LPCWSTR, nDefault: INT) -> UINT;
    pub fn GetProfileSectionA(lpAppName: LPCSTR, lpReturnedString: LPSTR, nSize: DWORD) -> DWORD;
    pub fn GetProfileSectionW(lpAppName: LPCWSTR, lpReturnedString: LPWSTR, nSize: DWORD) -> DWORD;
    pub fn GetProfileStringA(
        lpAppName: LPCSTR, lpKeyName: LPCSTR, lpDefault: LPCSTR, lpReturnedString: LPSTR,
        nSize: DWORD,
    ) -> DWORD;
    pub fn GetProfileStringW(
        lpAppName: LPCWSTR, lpKeyName: LPCWSTR, lpDefault: LPCWSTR, lpReturnedString: LPWSTR,
        nSize: DWORD,
    ) -> DWORD;
    pub fn GetQueuedCompletionStatus(
        CompletionPort: HANDLE, lpNumberOfBytesTransferred: LPDWORD, lpCompletionKey: PULONG_PTR,
        lpOverlapped: *mut LPOVERLAPPED, dwMilliseconds: DWORD,
    ) -> BOOL;
    pub fn GetQueuedCompletionStatusEx(
        CompletionPort: HANDLE, lpCompletionPortEntries: LPOVERLAPPED_ENTRY, ulCount: ULONG,
        ulNumEntriesRemoved: PULONG, dwMilliseconds: DWORD, fAlertable: BOOL,
    ) -> BOOL;
    pub fn GetShortPathNameA(
        lpszLongPath: LPCSTR, lpszShortPath: LPSTR, cchBuffer: DWORD,
    ) -> DWORD;
    pub fn GetShortPathNameW(
        lpszLongPath: LPCWSTR, lpszShortPath: LPWSTR, cchBuffer: DWORD,
    ) -> DWORD;
    // pub fn GetStagedPackagePathByFullName();
    pub fn GetStartupInfoA(lpStartupInfo: LPSTARTUPINFOA);
    pub fn GetStartupInfoW(lpStartupInfo: LPSTARTUPINFOW);
    // pub fn GetStateFolder();
    pub fn GetStdHandle(nStdHandle: DWORD) -> HANDLE;
    pub fn GetStringScripts(
        dwFlags: DWORD, lpString: LPCWSTR, cchString: c_int, lpScripts: LPWSTR, cchScripts: c_int,
    ) -> c_int;
    pub fn GetStringTypeA(
        Locale: LCID, dwInfoType: DWORD, lpSrcStr: LPCSTR, cchSrc: c_int, lpCharType: LPWORD,
    ) -> BOOL;
    pub fn GetStringTypeExA(
        Locale: LCID, dwInfoType: DWORD, lpSrcStr: LPCSTR, cchSrc: c_int, lpCharType: LPWORD,
    ) -> BOOL;
    pub fn GetStringTypeExW(
        Locale: LCID, dwInfoType: DWORD, lpSrcStr: LPCWCH, cchSrc: c_int, lpCharType: LPWORD,
    ) -> BOOL;
    pub fn GetStringTypeW(
        dwInfoType: DWORD, lpSrcStr: LPCWCH, cchSrc: c_int, lpCharType: LPWORD,
    ) -> BOOL;
    // pub fn GetSystemAppDataKey();
    pub fn GetSystemDEPPolicy() -> DEP_SYSTEM_POLICY_TYPE;
    pub fn GetSystemDefaultLCID() -> LCID;
    pub fn GetSystemDefaultLangID() -> LANGID;
    pub fn GetSystemDefaultLocaleName(lpLocaleName: LPWSTR, cchLocaleName: c_int) -> c_int;
    pub fn GetSystemDefaultUILanguage() -> LANGID;
    pub fn GetSystemDirectoryA(lpBuffer: LPSTR, uSize: UINT) -> UINT;
    pub fn GetSystemDirectoryW(lpBuffer: LPWSTR, uSize: UINT) -> UINT;
    pub fn GetSystemFileCacheSize(
        lpMinimumFileCacheSize: PSIZE_T, lpMaximumFileCacheSize: PSIZE_T, lpFlags: PDWORD,
    ) -> BOOL;
    pub fn GetSystemFirmwareTable(
        FirmwareTableProviderSignature: DWORD, FirmwareTableID: DWORD, pFirmwareTableBuffer: PVOID,
        BufferSize: DWORD,
    ) -> UINT;
    pub fn GetSystemInfo(lpSystemInfo: LPSYSTEM_INFO);
    pub fn GetSystemPowerStatus(lpSystemPowerStatus: LPSYSTEM_POWER_STATUS) -> BOOL;
    pub fn GetSystemPreferredUILanguages(
        dwFlags: DWORD, pulNumLanguages: PULONG, pwszLanguagesBuffer: PZZWSTR,
        pcchLanguagesBuffer: PULONG,
    ) -> BOOL;
    pub fn GetSystemRegistryQuota(pdwQuotaAllowed: PDWORD, pdwQuotaUsed: PDWORD) -> BOOL;
    pub fn GetSystemTime(lpSystemTime: LPSYSTEMTIME);
    pub fn GetSystemTimeAdjustment(
        lpTimeAdjustment: PDWORD, lpTimeIncrement: PDWORD, lpTimeAdjustmentDisabled: PBOOL,
    ) -> BOOL;
    pub fn GetSystemTimeAsFileTime(lpSystemTimeAsFileTime: LPFILETIME);
    pub fn GetSystemTimePreciseAsFileTime(lpSystemTimeAsFileTime: LPFILETIME);
    pub fn GetSystemTimes(
        lpIdleTime: LPFILETIME, lpKernelTime: LPFILETIME, lpUserTime: LPFILETIME,
    ) -> BOOL;
    pub fn GetSystemWindowsDirectoryA(lpBuffer: LPSTR, uSize: UINT) -> UINT;
    pub fn GetSystemWindowsDirectoryW(lpBuffer: LPWSTR, uSize: UINT) -> UINT;
    pub fn GetSystemWow64DirectoryA(lpBuffer: LPSTR, uSize: UINT) -> UINT;
    pub fn GetSystemWow64DirectoryW(lpBuffer: LPWSTR, uSize: UINT) -> UINT;
    pub fn GetTapeParameters(
        hDevice: HANDLE, dwOperation: DWORD, lpdwSize: LPDWORD, lpTapeInformation: LPVOID
    ) -> DWORD;
    pub fn GetTapePosition(
        hDevice: HANDLE, dwPositionType: DWORD, lpdwPartition: LPDWORD,
        lpdwOffsetLow: LPDWORD, lpdwOffsetHigh: LPDWORD
    ) -> DWORD;
    pub fn GetTapeStatus(hDevice: HANDLE) -> DWORD;
    pub fn GetTempFileNameA(
        lpPathName: LPCSTR, lpPrefixString: LPCSTR, uUnique: UINT, lpTempFileName: LPSTR,
    ) -> UINT;
    pub fn GetTempFileNameW(
        lpPathName: LPCWSTR, lpPrefixString: LPCWSTR, uUnique: UINT, lpTempFileName: LPWSTR,
    ) -> UINT;
    pub fn GetTempPathA(nBufferLength: DWORD, lpBuffer: LPSTR) -> DWORD;
    pub fn GetTempPathW(nBufferLength: DWORD, lpBuffer: LPWSTR) -> DWORD;
    pub fn GetThreadContext(hThread: HANDLE, lpContext: LPCONTEXT) -> BOOL;
    pub fn GetThreadErrorMode() -> DWORD;
    pub fn GetThreadGroupAffinity(hThread: HANDLE, GroupAffinity: PGROUP_AFFINITY) -> BOOL;
    pub fn GetThreadIOPendingFlag(hThread: HANDLE, lpIOIsPending: PBOOL) -> BOOL;
    pub fn GetThreadId(Thread: HANDLE) -> DWORD;
    pub fn GetThreadIdealProcessorEx(hThread: HANDLE, lpIdealProcessor: PPROCESSOR_NUMBER) -> BOOL;
    pub fn GetThreadInformation(
        hThread: HANDLE, ThreadInformationClass: THREAD_INFORMATION_CLASS,
        ThreadInformation: LPVOID, ThreadInformationSize: DWORD,
    ) -> BOOL;
    pub fn GetThreadLocale() -> LCID;
    pub fn GetThreadPreferredUILanguages(
        dwFlags: DWORD, pulNumLanguages: PULONG, pwszLanguagesBuffer: PZZWSTR,
        pcchLanguagesBuffer: PULONG,
    ) -> BOOL;
    pub fn GetThreadPriority(hThread: HANDLE) -> c_int;
    pub fn GetThreadPriorityBoost(hThread: HANDLE, pDisablePriorityBoost: PBOOL) -> BOOL;
    pub fn GetThreadSelectorEntry(
        hThread: HANDLE, dwSelector: DWORD, lpSelectorEntry: LPLDT_ENTRY,
    ) -> BOOL;
    pub fn GetThreadTimes(
        hThread: HANDLE, lpCreationTime: LPFILETIME, lpExitTime: LPFILETIME,
        lpKernelTime: LPFILETIME, lpUserTime: LPFILETIME,
    ) -> BOOL;
    pub fn GetThreadUILanguage() -> LANGID;
    pub fn GetTickCount() -> DWORD;
    pub fn GetTickCount64() -> ULONGLONG;
    pub fn GetTimeFormatA(
        Locale: LCID, dwFlags: DWORD, lpTime: *const SYSTEMTIME, lpFormat: LPCSTR,
        lpTimeStr: LPSTR, cchTime: c_int,
    ) -> c_int;
    pub fn GetTimeFormatEx(
        lpLocaleName: LPCWSTR, dwFlags: DWORD, lpTime: *const SYSTEMTIME, lpFormat: LPCWSTR,
        lpTimeStr: LPWSTR, cchTime: c_int,
    ) -> c_int;
    pub fn GetTimeFormatW(
        Locale: LCID, dwFlags: DWORD, lpTime: *const SYSTEMTIME, lpFormat: LPCWSTR,
        lpTimeStr: LPWSTR, cchTime: c_int,
    ) -> c_int;
    pub fn GetTimeZoneInformation(lpTimeZoneInformation: LPTIME_ZONE_INFORMATION) -> DWORD;
    pub fn GetTimeZoneInformationForYear(
        wYear: USHORT, pdtzi: PDYNAMIC_TIME_ZONE_INFORMATION, ptzi: LPTIME_ZONE_INFORMATION,
    ) -> BOOL;
    pub fn GetUILanguageInfo(
        dwFlags: DWORD, pwmszLanguage: PCZZWSTR, pwszFallbackLanguages: PZZWSTR,
        pcchFallbackLanguages: PDWORD, pAttributes: PDWORD,
    ) -> BOOL;
    #[cfg(target_arch = "x86_64")]
    pub fn GetUmsCompletionListEvent(
        UmsCompletionList: PUMS_COMPLETION_LIST, UmsCompletionEvent: PHANDLE,
    ) -> BOOL;
    #[cfg(target_arch = "x86_64")]
    pub fn GetUmsSystemThreadInformation(
        ThreadHandle: HANDLE, SystemThreadInfo: PUMS_SYSTEM_THREAD_INFORMATION,
    ) -> BOOL;
    pub fn GetUserDefaultLCID() -> LCID;
    pub fn GetUserDefaultLangID() -> LANGID;
    pub fn GetUserDefaultLocaleName(lpLocaleName: LPWSTR, cchLocaleName: c_int) -> c_int;
    pub fn GetUserDefaultUILanguage() -> LANGID;
    pub fn GetUserGeoID(GeoClass: GEOCLASS) -> GEOID;
    pub fn GetUserPreferredUILanguages(
        dwFlags: DWORD, pulNumLanguages: PULONG, pwszLanguagesBuffer: PZZWSTR,
        pcchLanguagesBuffer: PULONG,
    ) -> BOOL;
    pub fn GetVersion() -> DWORD;
    pub fn GetVersionExA(lpVersionInformation: LPOSVERSIONINFOA) -> BOOL;
    pub fn GetVersionExW(lpVersionInformation: LPOSVERSIONINFOW) -> BOOL;
    pub fn GetVolumeInformationA(
        lpRootPathName: LPCSTR, lpVolumeNameBuffer: LPSTR, nVolumeNameSize: DWORD,
        lpVolumeSerialNumber: LPDWORD, lpMaximumComponentLength: LPDWORD,
        lpFileSystemFlags: LPDWORD, lpFileSystemNameBuffer: LPSTR, nFileSystemNameSize: DWORD,
    ) -> BOOL;
    pub fn GetVolumeInformationByHandleW(
        hFile: HANDLE, lpVolumeNameBuffer: LPWSTR, nVolumeNameSize: DWORD,
        lpVolumeSerialNumber: LPDWORD, lpMaximumComponentLength: LPDWORD,
        lpFileSystemFlags: LPDWORD, lpFileSystemNameBuffer: LPWSTR, nFileSystemNameSize: DWORD,
    ) -> BOOL;
    pub fn GetVolumeInformationW(
        lpRootPathName: LPCWSTR, lpVolumeNameBuffer: LPWSTR, nVolumeNameSize: DWORD,
        lpVolumeSerialNumber: LPDWORD, lpMaximumComponentLength: LPDWORD,
        lpFileSystemFlags: LPDWORD, lpFileSystemNameBuffer: LPWSTR, nFileSystemNameSize: DWORD,
    ) -> BOOL;
    pub fn GetVolumeNameForVolumeMountPointA(
        lpszVolumeMountPoint: LPCSTR, lpszVolumeName: LPSTR, cchBufferLength: DWORD,
    ) -> BOOL;
    pub fn GetVolumeNameForVolumeMountPointW(
        lpszVolumeMountPoint: LPCWSTR, lpszVolumeName: LPWSTR, cchBufferLength: DWORD,
    ) -> BOOL;
    pub fn GetVolumePathNameA(
        lpszFileName: LPCSTR, lpszVolumePathName: LPSTR, cchBufferLength: DWORD,
    ) -> BOOL;
    pub fn GetVolumePathNameW(
        lpszFileName: LPCWSTR, lpszVolumePathName: LPWSTR, cchBufferLength: DWORD,
    ) -> BOOL;
    pub fn GetVolumePathNamesForVolumeNameA(
        lpszVolumeName: LPCSTR, lpszVolumePathNames: LPCH, cchBufferLength: DWORD,
        lpcchReturnLength: PDWORD,
    ) -> BOOL;
    pub fn GetVolumePathNamesForVolumeNameW(
        lpszVolumeName: LPCWSTR, lpszVolumePathNames: LPWCH, cchBufferLength: DWORD,
        lpcchReturnLength: PDWORD,
    ) -> BOOL;
    pub fn GetWindowsDirectoryA(lpBuffer: LPSTR, uSize: UINT) -> UINT;
    pub fn GetWindowsDirectoryW(lpBuffer: LPWSTR, uSize: UINT) -> UINT;
    pub fn GetWriteWatch(
        dwFlags: DWORD, lpBaseAddress: PVOID, dwRegionSize: SIZE_T, lpAddresses: *mut PVOID,
        lpdwCount: *mut ULONG_PTR, lpdwGranularity: LPDWORD,
    ) -> UINT;
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn GetXStateFeaturesMask(Context: PCONTEXT, FeatureMask: PDWORD64) -> BOOL;
    pub fn GlobalAddAtomA(lpString: LPCSTR) -> ATOM;
    pub fn GlobalAddAtomExA(lpString: LPCSTR, Flags: DWORD) -> ATOM;
    pub fn GlobalAddAtomExW(lpString: LPCWSTR, Flags: DWORD) -> ATOM;
    pub fn GlobalAddAtomW(lpString: LPCWSTR) -> ATOM;
    pub fn GlobalAlloc(uFlags: UINT, dwBytes: SIZE_T) -> HGLOBAL;
    pub fn GlobalCompact(dwMinFree: DWORD) -> SIZE_T;
    pub fn GlobalDeleteAtom(nAtom: ATOM) -> ATOM;
    pub fn GlobalFindAtomA(lpString: LPCSTR) -> ATOM;
    pub fn GlobalFindAtomW(lpString: LPCWSTR) -> ATOM;
    pub fn GlobalFix(hMem: HGLOBAL);
    pub fn GlobalFlags(hMem: HGLOBAL) -> UINT;
    pub fn GlobalFree(hMem: HGLOBAL) -> HGLOBAL;
    pub fn GlobalGetAtomNameA(nAtom: ATOM, lpBuffer: LPSTR, nSize: c_int) -> UINT;
    pub fn GlobalGetAtomNameW(nAtom: ATOM, lpBuffer: LPWSTR, nSize: c_int) -> UINT;
    pub fn GlobalHandle(pMem: LPCVOID) -> HGLOBAL;
    pub fn GlobalLock(hMem: HGLOBAL) -> LPVOID;
    pub fn GlobalMemoryStatus(lpBuffer: LPMEMORYSTATUS);
    pub fn GlobalMemoryStatusEx(lpBuffer: LPMEMORYSTATUSEX) -> BOOL;
    pub fn GlobalReAlloc(hMem: HGLOBAL, dwBytes: SIZE_T, uFlags: UINT) -> HGLOBAL;
    pub fn GlobalSize(hMem: HGLOBAL) -> SIZE_T;
    pub fn GlobalUnWire(hMem: HGLOBAL) -> BOOL;
    pub fn GlobalUnfix(hMem: HGLOBAL);
    pub fn GlobalUnlock(hMem: HGLOBAL) -> BOOL;
    pub fn GlobalWire(hMem: HGLOBAL) -> LPVOID;
    pub fn Heap32First(lphe: LPHEAPENTRY32, th32ProcessID: DWORD, th32HeapID: ULONG_PTR) -> BOOL;
    pub fn Heap32ListFirst(hSnapshot: HANDLE, lphl: LPHEAPLIST32) -> BOOL;
    pub fn Heap32ListNext(hSnapshot: HANDLE, lphl: LPHEAPLIST32) -> BOOL;
    pub fn Heap32Next(lphe: LPHEAPENTRY32) -> BOOL;
    pub fn HeapAlloc(hHeap: HANDLE, dwFlags: DWORD, dwBytes: SIZE_T) -> LPVOID;
    pub fn HeapCompact(hHeap: HANDLE, dwFlags: DWORD) -> SIZE_T;
    pub fn HeapCreate(flOptions: DWORD, dwInitialSize: SIZE_T, dwMaximumSize: SIZE_T) -> HANDLE;
    pub fn HeapDestroy(hHeap: HANDLE) -> BOOL;
    pub fn HeapFree(hHeap: HANDLE, dwFlags: DWORD, lpMem: LPVOID) -> BOOL;
    pub fn HeapLock(hHeap: HANDLE) -> BOOL;
    pub fn HeapQueryInformation(
        HeapHandle: HANDLE, HeapInformationClass: HEAP_INFORMATION_CLASS, HeapInformation: PVOID,
        HeapInformationLength: SIZE_T, ReturnLength: PSIZE_T,
    ) -> BOOL;
    pub fn HeapReAlloc(hHeap: HANDLE, dwFlags: DWORD, lpMem: LPVOID, dwBytes: SIZE_T) -> LPVOID;
    pub fn HeapSetInformation(
        HeapHandle: HANDLE, HeapInformationClass: HEAP_INFORMATION_CLASS, HeapInformation: PVOID,
        HeapInformationLength: SIZE_T,
    ) -> BOOL;
    pub fn HeapSize(hHeap: HANDLE, dwFlags: DWORD, lpMem: LPCVOID) -> SIZE_T;
    pub fn HeapSummary(hHeap: HANDLE, dwFlags: DWORD, lpSummary: LPHEAP_SUMMARY) -> BOOL;
    pub fn HeapUnlock(hHeap: HANDLE) -> BOOL;
    pub fn HeapValidate(hHeap: HANDLE, dwFlags: DWORD, lpMem: LPCVOID) -> BOOL;
    pub fn HeapWalk(hHeap: HANDLE, lpEntry: LPPROCESS_HEAP_ENTRY) -> BOOL;
    pub fn InitAtomTable(nSize: DWORD) -> BOOL;
    pub fn InitOnceBeginInitialize(
        lpInitOnce: LPINIT_ONCE, dwFlags: DWORD, fPending: PBOOL, lpContext: *mut LPVOID,
    ) -> BOOL;
    pub fn InitOnceComplete(
        lpInitOnce: LPINIT_ONCE, dwFlags: DWORD, lpContext: LPVOID,
    ) -> BOOL;
    pub fn InitOnceExecuteOnce(
        InitOnce: PINIT_ONCE, InitFn: PINIT_ONCE_FN, Parameter: PVOID, Context: *mut LPVOID,
    ) -> BOOL;
    pub fn InitOnceInitialize(InitOnce: PINIT_ONCE);
    pub fn InitializeConditionVariable(ConditionVariable: PCONDITION_VARIABLE);
    pub fn InitializeContext(
        Buffer: PVOID, ContextFlags: DWORD, Context: *mut PCONTEXT, ContextLength: PDWORD,
    ) -> BOOL;
    pub fn InitializeCriticalSection(lpCriticalSection: LPCRITICAL_SECTION);
    pub fn InitializeCriticalSectionAndSpinCount(
        lpCriticalSection: LPCRITICAL_SECTION, dwSpinCount: DWORD,
    ) -> BOOL;
    pub fn InitializeCriticalSectionEx(
        lpCriticalSection: LPCRITICAL_SECTION, dwSpinCount: DWORD, Flags: DWORD,
    ) -> BOOL;
    pub fn InitializeProcThreadAttributeList(
        lpAttributeList: LPPROC_THREAD_ATTRIBUTE_LIST, dwAttributeCount: DWORD, dwFlags: DWORD,
        lpSize: PSIZE_T,
    ) -> BOOL;
    pub fn InitializeSListHead(ListHead: PSLIST_HEADER);
    pub fn InitializeSRWLock(SRWLock: PSRWLOCK);
    pub fn InitializeSynchronizationBarrier(
        lpBarrier: LPSYNCHRONIZATION_BARRIER, lTotalThreads: LONG, lSpinCount: LONG,
    ) -> BOOL;
    pub fn InstallELAMCertificateInfo(ELAMFile: HANDLE) -> BOOL;
    #[cfg(target_arch = "x86")]
    pub fn InterlockedCompareExchange(
        Destination: *mut LONG, ExChange: LONG, Comperand: LONG,
    ) -> LONG;
    #[cfg(target_arch = "x86")]
    pub fn InterlockedCompareExchange64(
        Destination: *mut LONG64, ExChange: LONG64, Comperand: LONG64,
    ) -> LONG64;
    #[cfg(target_arch = "x86")]
    pub fn InterlockedDecrement(Addend: *mut LONG) -> LONG;
    #[cfg(target_arch = "x86")]
    pub fn InterlockedExchange(Target: *mut LONG, Value: LONG) -> LONG;
    #[cfg(target_arch = "x86")]
    pub fn InterlockedExchangeAdd(Addend: *mut LONG, Value: LONG) -> LONG;
    pub fn InterlockedFlushSList(ListHead: PSLIST_HEADER) -> PSLIST_ENTRY;
    #[cfg(target_arch = "x86")]
    pub fn InterlockedIncrement(Addend: *mut LONG) -> LONG;
    pub fn InterlockedPopEntrySList(ListHead: PSLIST_HEADER) -> PSLIST_ENTRY;
    pub fn InterlockedPushEntrySList(
        ListHead: PSLIST_HEADER, ListEntry: PSLIST_ENTRY,
    ) -> PSLIST_ENTRY;
    pub fn InterlockedPushListSListEx(
        ListHead: PSLIST_HEADER, List: PSLIST_ENTRY, ListEnd: PSLIST_ENTRY, Count: ULONG,
    ) -> PSLIST_ENTRY;
    pub fn IsBadCodePtr(lpfn: FARPROC) -> BOOL;
    pub fn IsBadHugeReadPtr(lp: *const VOID, ucb: UINT_PTR) -> BOOL;
    pub fn IsBadHugeWritePtr(lp: LPVOID, ucb: UINT_PTR) -> BOOL;
    pub fn IsBadReadPtr(lp: *const VOID, ucb: UINT_PTR) -> BOOL;
    pub fn IsBadStringPtrA(lpsz: LPCSTR, ucchMax: UINT_PTR) -> BOOL;
    pub fn IsBadStringPtrW(lpsz: LPCWSTR, ucchMax: UINT_PTR) -> BOOL;
    pub fn IsBadWritePtr(lp: LPVOID, ucb: UINT_PTR) -> BOOL;
    pub fn IsDBCSLeadByte(TestChar: BYTE) -> BOOL;
    pub fn IsDBCSLeadByteEx(CodePage: UINT, TestChar: BYTE) -> BOOL;
    pub fn IsDebuggerPresent() -> BOOL;
    pub fn IsNLSDefinedString(
        Function: NLS_FUNCTION, dwFlags: DWORD, lpVersionInformation: LPNLSVERSIONINFO,
        lpString: LPCWSTR, cchStr: INT,
    ) -> BOOL;
    pub fn IsNativeVhdBoot(NativeVhdBoot: PBOOL) -> BOOL;
    pub fn IsNormalizedString(NormForm: NORM_FORM, lpString: LPCWSTR, cwLength: c_int) -> BOOL;
    pub fn IsProcessCritical(hProcess: HANDLE, Critical: PBOOL) -> BOOL;
    pub fn IsProcessInJob(ProcessHandle: HANDLE, JobHandle: HANDLE, Result: PBOOL) -> BOOL;
    pub fn IsProcessorFeaturePresent(ProcessorFeature: DWORD) -> BOOL;
    pub fn IsSystemResumeAutomatic() -> BOOL;
    pub fn IsThreadAFiber() -> BOOL;
    pub fn IsThreadpoolTimerSet(pti: PTP_TIMER) -> BOOL;
    pub fn IsValidCodePage(CodePage: UINT) -> BOOL;
    pub fn IsValidLanguageGroup(LanguageGroup: LGRPID, dwFlags: DWORD) -> BOOL;
    pub fn IsValidLocale(Locale: LCID, dwFlags: DWORD) -> BOOL;
    pub fn IsValidLocaleName(lpLocaleName: LPCWSTR) -> BOOL;
    pub fn IsValidNLSVersion(
        function: NLS_FUNCTION, lpLocaleName: LPCWSTR, lpVersionInformation: LPNLSVERSIONINFOEX,
    ) -> BOOL;
    pub fn IsWow64Process(hProcess: HANDLE, Wow64Process: PBOOL) -> BOOL;
    pub fn K32EmptyWorkingSet(hProcess: HANDLE) -> BOOL;
    pub fn K32EnumDeviceDrivers(lpImageBase: *mut LPVOID, cb: DWORD, lpcbNeeded: LPDWORD) -> BOOL;
    pub fn K32EnumPageFilesA(
        pCallBackRoutine: PENUM_PAGE_FILE_CALLBACKA, pContext: LPVOID,
    ) -> BOOL;
    pub fn K32EnumPageFilesW(
        pCallBackRoutine: PENUM_PAGE_FILE_CALLBACKW, pContext: LPVOID,
    ) -> BOOL;
    pub fn K32EnumProcessModules(
        hProcess: HANDLE, lphModule: *mut HMODULE, cb: DWORD, lpcbNeeded: LPDWORD,
    ) -> BOOL;
    pub fn K32EnumProcessModulesEx(
        hProcess: HANDLE, lphModule: *mut HMODULE, cb: DWORD, lpcbNeeded: LPDWORD,
        dwFilterFlag: DWORD,
    ) -> BOOL;
    pub fn K32EnumProcesses(
        lpidProcess: *mut DWORD, cb: DWORD, lpcbNeeded: LPDWORD,
    ) -> BOOL;
    pub fn K32GetDeviceDriverBaseNameA(ImageBase: LPVOID, lpFilename: LPSTR, nSize: DWORD) -> DWORD;
    pub fn K32GetDeviceDriverBaseNameW(
        ImageBase: LPVOID, lpFilename: LPWSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn K32GetDeviceDriverFileNameA(ImageBase: LPVOID, lpFilename: LPSTR, nSize: DWORD) -> DWORD;
    pub fn K32GetDeviceDriverFileNameW(
        ImageBase: LPVOID, lpFilename: LPWSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn K32GetMappedFileNameA(
        hProcess: HANDLE, lpv: LPVOID, lpFilename: LPSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn K32GetMappedFileNameW(
        hProcess: HANDLE, lpv: LPVOID, lpFilename: LPWSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn K32GetModuleBaseNameA(
        hProcess: HANDLE, hModule: HMODULE, lpBaseName: LPSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn K32GetModuleBaseNameW(
        hProcess: HANDLE, hModule: HMODULE, lpBaseName: LPWSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn K32GetModuleFileNameExA(
        hProcess: HANDLE, hModule: HMODULE, lpFilename: LPSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn K32GetModuleFileNameExW(
        hProcess: HANDLE, hModule: HMODULE, lpFilename: LPWSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn K32GetModuleInformation(
        hProcess: HANDLE, hModule: HMODULE, lpmodinfo: LPMODULEINFO, cb: DWORD,
    ) -> BOOL;
    pub fn K32GetPerformanceInfo(
        pPerformanceInformation: PPERFORMANCE_INFORMATION, cb: DWORD,
    ) -> BOOL;
    pub fn K32GetProcessImageFileNameA(
        hProcess: HANDLE, lpImageFileName: LPSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn K32GetProcessImageFileNameW(
        hProcess: HANDLE, lpImageFileName: LPWSTR, nSize: DWORD,
    ) -> DWORD;
    pub fn K32GetProcessMemoryInfo(
        Process: HANDLE, ppsmemCounters: PPROCESS_MEMORY_COUNTERS, cb: DWORD,
    ) -> BOOL;
    pub fn K32GetWsChanges(
        hProcess: HANDLE, lpWatchInfo: PPSAPI_WS_WATCH_INFORMATION, cb: DWORD,
    ) -> BOOL;
    pub fn K32GetWsChangesEx(
        hProcess: HANDLE, lpWatchInfoEx: PPSAPI_WS_WATCH_INFORMATION_EX, cb: PDWORD,
    ) -> BOOL;
    pub fn K32InitializeProcessForWsWatch(hProcess: HANDLE) -> BOOL;
    pub fn K32QueryWorkingSet(hProcess: HANDLE, pv: PVOID, cb: DWORD) -> BOOL;
    pub fn K32QueryWorkingSetEx(hProcess: HANDLE, pv: PVOID, cb: DWORD) -> BOOL;
    pub fn LCIDToLocaleName(Locale: LCID, lpName: LPWSTR, cchName: c_int, dwFlags: DWORD) -> c_int;
    pub fn LCMapStringA(
        Locale: LCID, dwMapFlags: DWORD, lpSrcStr: LPCSTR, cchSrc: c_int, lpDestStr: LPSTR,
        cchDest: c_int,
    ) -> c_int;
    pub fn LCMapStringEx(
        lpLocaleName: LPCWSTR, dwMapFlags: DWORD, lpSrcStr: LPCWSTR, cchSrc: c_int,
        lpDestStr: LPWSTR, cchDest: c_int, lpVersionInformation: LPNLSVERSIONINFO,
        lpReserved: LPVOID, sortHandle: LPARAM,
    ) -> c_int;
    pub fn LCMapStringW(
        Locale: LCID, dwMapFlags: DWORD, lpSrcStr: LPCWSTR, cchSrc: c_int, lpDestStr: LPWSTR,
        cchDest: c_int,
    ) -> c_int;
    pub fn LeaveCriticalSection(lpCriticalSection: LPCRITICAL_SECTION);
    pub fn LeaveCriticalSectionWhenCallbackReturns(
        pci: PTP_CALLBACK_INSTANCE, pcs: PCRITICAL_SECTION,
    );
    // pub fn LoadAppInitDlls();
    pub fn LoadLibraryA(lpFileName: LPCSTR) -> HMODULE;
    pub fn LoadLibraryExA(lpLibFileName: LPCSTR, hFile: HANDLE, dwFlags: DWORD) -> HMODULE;
    pub fn LoadLibraryExW(lpLibFileName: LPCWSTR, hFile: HANDLE, dwFlags: DWORD) -> HMODULE;
    pub fn LoadLibraryW(lpFileName: LPCWSTR) -> HMODULE;
    pub fn LoadModule(lpModuleName: LPCSTR, lpParameterBlock: LPVOID) -> DWORD;
    pub fn LoadPackagedLibrary(lpwLibFileName: LPCWSTR, Reserved: DWORD) -> HMODULE;
    pub fn LoadResource(hModule: HMODULE, hResInfo: HRSRC) -> HGLOBAL;
    // pub fn LoadStringBaseExW();
    // pub fn LoadStringBaseW();
    pub fn LocalAlloc(uFlags: UINT, uBytes: SIZE_T) -> HLOCAL;
    pub fn LocalCompact(uMinFree: UINT) -> SIZE_T;
    pub fn LocalFileTimeToFileTime(
        lpLocalFileTime: *const FILETIME, lpFileTime: LPFILETIME,
    ) -> BOOL;
    pub fn LocalFlags(hMem: HLOCAL) -> UINT;
    pub fn LocalFree(hMem: HLOCAL) -> HLOCAL;
    pub fn LocalHandle(pMem: LPCVOID) -> HLOCAL;
    pub fn LocalLock(hMem: HLOCAL) -> LPVOID;
    pub fn LocalReAlloc(hMem: HLOCAL, uBytes: SIZE_T, uFlags: UINT) -> HLOCAL;
    pub fn LocalShrink(hMem: HLOCAL, cbNewSize: UINT) -> SIZE_T;
    pub fn LocalSize(hMem: HLOCAL) -> SIZE_T;
    pub fn LocalUnlock(hMem: HLOCAL) -> BOOL;
    pub fn LocaleNameToLCID(lpName: LPCWSTR, dwFlags: DWORD) -> LCID;
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn LocateXStateFeature(Context: PCONTEXT, FeatureId: DWORD, Length: PDWORD) -> PVOID;
    pub fn LockFile(
        hFile: HANDLE, dwFileOffsetLow: DWORD, dwFileOffsetHigh: DWORD,
        nNumberOfBytesToLockLow: DWORD, nNumberOfBytesToLockHigh: DWORD,
    ) -> BOOL;
    pub fn LockFileEx(
        hFile: HANDLE, dwFlags: DWORD, dwReserved: DWORD, nNumberOfBytesToLockLow: DWORD,
        nNumberOfBytesToLockHigh: DWORD, lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
    pub fn LockResource(hResData: HGLOBAL) -> LPVOID;
    pub fn MapUserPhysicalPages(
        VirtualAddress: PVOID, NumberOfPages: ULONG_PTR, PageArray: PULONG_PTR,
    ) -> BOOL;
    pub fn MapUserPhysicalPagesScatter(
        VirtualAddresses: *mut PVOID, NumberOfPages: ULONG_PTR, PageArray: PULONG_PTR,
    ) -> BOOL;
    pub fn MapViewOfFile(
        hFileMappingObject: HANDLE, dwDesiredAccess: DWORD, dwFileOffsetHigh: DWORD,
        dwFileOffsetLow: DWORD, dwNumberOfBytesToMap: SIZE_T,
    ) -> LPVOID;
    pub fn MapViewOfFileEx(
        hFileMappingObject: HANDLE, dwDesiredAccess: DWORD, dwFileOffsetHigh: DWORD,
        dwFileOffsetLow: DWORD, dwNumberOfBytesToMap: SIZE_T, lpBaseAddress: LPVOID,
    ) -> LPVOID;
    pub fn MapViewOfFileExNuma(
        hFileMappingObject: HANDLE, dwDesiredAccess: DWORD, dwFileOffsetHigh: DWORD,
        dwFileOffsetLow: DWORD, dwNumberOfBytesToMap: SIZE_T, lpBaseAddress: LPVOID,
        nndPreferred: DWORD,
    ) -> LPVOID;
    pub fn MapViewOfFileFromApp(
        hFileMappingObject: HANDLE, DesiredAccess: ULONG, FileOffset: ULONG64,
        NumberOfBytesToMap: SIZE_T,
    ) -> PVOID;
    pub fn Module32First(hSnapshot: HANDLE, lpme: LPMODULEENTRY32) -> BOOL;
    pub fn Module32FirstW(hSnapshot: HANDLE, lpme: LPMODULEENTRY32W) -> BOOL;
    pub fn Module32Next(hSnapshot: HANDLE, lpme: LPMODULEENTRY32) -> BOOL;
    pub fn Module32NextW(hSnapshot: HANDLE, lpme: LPMODULEENTRY32W) -> BOOL;
    pub fn MoveFileA(lpExistingFileName: LPCSTR, lpNewFileName: LPCSTR) -> BOOL;
    pub fn MoveFileExA(lpExistingFileName: LPCSTR, lpNewFileName: LPCSTR, dwFlags: DWORD) -> BOOL;
    pub fn MoveFileExW(lpExistingFileName: LPCWSTR, lpNewFileName: LPCWSTR, dwFlags: DWORD) -> BOOL;
    pub fn MoveFileTransactedA(
        lpExistingFileName: LPCSTR, lpNewFileName: LPCSTR, lpProgressRoutine: LPPROGRESS_ROUTINE,
        lpData: LPVOID, dwFlags: DWORD, hTransaction: HANDLE,
    ) -> BOOL;
    pub fn MoveFileTransactedW(
        lpExistingFileName: LPCWSTR, lpNewFileName: LPCWSTR, lpProgressRoutine: LPPROGRESS_ROUTINE,
        lpData: LPVOID, dwFlags: DWORD, hTransaction: HANDLE,
    ) -> BOOL;
    pub fn MoveFileW(lpExistingFileName: LPCWSTR, lpNewFileName: LPCWSTR) -> BOOL;
    pub fn MoveFileWithProgressA(
        lpExistingFileName: LPCSTR, lpNewFileName: LPCSTR, lpProgressRoutine: LPPROGRESS_ROUTINE,
        lpData: LPVOID, dwFlags: DWORD,
    ) -> BOOL;
    pub fn MoveFileWithProgressW(
        lpExistingFileName: LPCWSTR, lpNewFileName: LPCWSTR, lpProgressRoutine: LPPROGRESS_ROUTINE,
        lpData: LPVOID, dwFlags: DWORD,
    ) -> BOOL;
    pub fn MulDiv(nNumber: c_int, nNumerator: c_int, nDenominator: c_int) -> c_int;
    pub fn MultiByteToWideChar(
        CodePage: UINT, dwFlags: DWORD, lpMultiByteStr: LPCSTR, cbMultiByte: c_int,
        lpWideCharStr: LPWSTR, cchWideChar: c_int,
    ) -> c_int;
    pub fn NeedCurrentDirectoryForExePathA(ExeName: LPCSTR) -> BOOL;
    pub fn NeedCurrentDirectoryForExePathW(ExeName: LPCWSTR) -> BOOL;
    pub fn NormalizeString(
        NormForm: NORM_FORM, lpSrcString: LPCWSTR, cwSrcLength: c_int, lpDstString: LPWSTR,
        cwDstLength: c_int,
    ) -> c_int;
    // pub fn NotifyMountMgr();
    pub fn NotifyUILanguageChange(
        dwFlags: DWORD, pcwstrNewLanguage: PCWSTR, pcwstrPreviousLanguage: PCWSTR,
        dwReserved: DWORD, pdwStatusRtrn: PDWORD,
    ) -> BOOL;
    // pub fn OOBEComplete();
    pub fn OpenEventA(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: LPCSTR) -> HANDLE;
    pub fn OpenEventW(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: LPCWSTR) -> HANDLE;
    pub fn OpenFile(lpFileName: LPCSTR, lpReOpenBuff: LPOFSTRUCT, uStyle: UINT) -> HFILE;
    pub fn OpenFileById(
        hVolumeHint: HANDLE, lpFileId: LPFILE_ID_DESCRIPTOR, dwDesiredAccess: DWORD,
        dwShareMode: DWORD, lpSecurityAttributes: LPSECURITY_ATTRIBUTES,
        dwFlagsAndAttributes: DWORD,
    ) -> HANDLE;
    pub fn OpenFileMappingA(
        dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: LPCSTR,
    ) -> HANDLE;
    pub fn OpenFileMappingW(
        dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: LPCWSTR,
    ) -> HANDLE;
    pub fn OpenJobObjectA(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: LPCSTR) -> HANDLE;
    pub fn OpenJobObjectW(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: LPCWSTR) -> HANDLE;
    pub fn OpenMutexA(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: LPCSTR) -> HANDLE;
    pub fn OpenMutexW(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: LPCWSTR) -> HANDLE;
    // pub fn OpenPackageInfoByFullName();
    pub fn OpenPrivateNamespaceA(lpBoundaryDescriptor: LPVOID, lpAliasPrefix: LPCSTR) -> HANDLE;
    pub fn OpenPrivateNamespaceW(lpBoundaryDescriptor: LPVOID, lpAliasPrefix: LPCWSTR) -> HANDLE;
    pub fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
    pub fn OpenSemaphoreA(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: LPCSTR) -> HANDLE;
    pub fn OpenSemaphoreW(dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpName: LPCWSTR) -> HANDLE;
    // pub fn OpenState();
    // pub fn OpenStateExplicit();
    pub fn OpenThread(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwThreadId: DWORD) -> HANDLE;
    pub fn OpenWaitableTimerA(
        dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpTimerName: LPCSTR,
    ) -> HANDLE;
    pub fn OpenWaitableTimerW(
        dwDesiredAccess: DWORD, bInheritHandle: BOOL, lpTimerName: LPCWSTR,
    ) -> HANDLE;
    pub fn OutputDebugStringA(lpOutputString: LPCSTR);
    pub fn OutputDebugStringW(lpOutputString: LPCWSTR);
    // pub fn PackageFamilyNameFromFullName();
    // pub fn PackageFamilyNameFromId();
    // pub fn PackageFullNameFromId();
    // pub fn PackageIdFromFullName();
    // pub fn PackageNameAndPublisherIdFromFamilyName();
    // pub fn ParseApplicationUserModelId();
    pub fn PeekConsoleInputA(
        hConsoleInput: HANDLE, lpBuffer: PINPUT_RECORD, nLength: DWORD,
        lpNumberOfEventsRead: LPDWORD,
    ) -> BOOL;
    pub fn PeekConsoleInputW(
        hConsoleInput: HANDLE, lpBuffer: PINPUT_RECORD, nLength: DWORD,
        lpNumberOfEventsRead: LPDWORD,
    ) -> BOOL;
    pub fn PeekNamedPipe(
        hNamedPipe: HANDLE, lpBuffer: LPVOID, nBufferSize: DWORD, lpBytesRead: LPDWORD,
        lpTotalBytesAvail: LPDWORD, lpBytesLeftThisMessage: LPDWORD,
    ) -> BOOL;
    pub fn PostQueuedCompletionStatus(
        CompletionPort: HANDLE, dwNumberOfBytesTransferred: DWORD, dwCompletionKey: ULONG_PTR,
        lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
    pub fn PowerClearRequest(PowerRequest: HANDLE, RequestType: POWER_REQUEST_TYPE) -> BOOL;
    pub fn PowerCreateRequest(Context: PREASON_CONTEXT) -> HANDLE;
    pub fn PowerSetRequest(PowerRequest: HANDLE, RequestType: POWER_REQUEST_TYPE) -> BOOL;
    pub fn PrefetchVirtualMemory(
        hProcess: HANDLE, NumberOfEntries: ULONG_PTR, VirtualAddresses: PWIN32_MEMORY_RANGE_ENTRY,
        Flags: ULONG,
    ) -> BOOL;
    pub fn PrepareTape(hDevice: HANDLE, dwOperation: DWORD, bImmediate: BOOL) -> DWORD;
    pub fn Process32First(hSnapshot: HANDLE, lppe: LPPROCESSENTRY32) -> BOOL;
    pub fn Process32FirstW(hSnapshot: HANDLE, lppe: LPPROCESSENTRY32W) -> BOOL;
    pub fn Process32Next(hSnapshot: HANDLE, lppe: LPPROCESSENTRY32) -> BOOL;
    pub fn Process32NextW(hSnapshot: HANDLE, lppe: LPPROCESSENTRY32W) -> BOOL;
    pub fn ProcessIdToSessionId(dwProcessId: DWORD, pSessionId: *mut DWORD) -> BOOL;
    pub fn PssCaptureSnapshot(
        ProcessHandle: HANDLE, CaptureFlags: PSS_CAPTURE_FLAGS, ThreadContextFlags: DWORD,
        SnapshotHandle: *mut HPSS,
    ) -> DWORD;
    pub fn PssDuplicateSnapshot(
        SourceProcessHandle: HANDLE, SnapshotHandle: HPSS, TargetProcessHandle: HANDLE,
        TargetSnapshotHandle: *mut HPSS, Flags: PSS_DUPLICATE_FLAGS,
    ) -> DWORD;
    pub fn PssFreeSnapshot(ProcessHandle: HANDLE, SnapshotHandle: HPSS) -> DWORD;
    pub fn PssQuerySnapshot(
        SnapshotHandle: HPSS, InformationClass: PSS_QUERY_INFORMATION_CLASS, Buffer: *mut c_void,
        BufferLength: DWORD,
    ) -> DWORD;
    pub fn PssWalkMarkerCreate(
        Allocator: *const PSS_ALLOCATOR, WalkMarkerHandle: *mut HPSSWALK,
    ) -> DWORD;
    pub fn PssWalkMarkerFree(WalkMarkerHandle: HPSSWALK) -> DWORD;
    pub fn PssWalkMarkerGetPosition(WalkMarkerHandle: HPSSWALK, Position: *mut ULONG_PTR) -> DWORD;
    // pub fn PssWalkMarkerRewind();
    // pub fn PssWalkMarkerSeek();
    pub fn PssWalkMarkerSeekToBeginning(WalkMarkerHandle: HPSS) -> DWORD;
    pub fn PssWalkMarkerSetPosition(WalkMarkerHandle: HPSSWALK, Position: ULONG_PTR) -> DWORD;
    // pub fn PssWalkMarkerTell();
    pub fn PssWalkSnapshot(
        SnapshotHandle: HPSS, InformationClass: PSS_WALK_INFORMATION_CLASS,
        WalkMarkerHandle: HPSSWALK, Buffer: *mut c_void, BufferLength: DWORD,
    ) -> DWORD;
    pub fn PulseEvent(hEvent: HANDLE) -> BOOL;
    pub fn PurgeComm(hFile: HANDLE, dwFlags: DWORD) -> BOOL;
    pub fn QueryActCtxSettingsW(
        dwFlags: DWORD, hActCtx: HANDLE, settingsNameSpace: PCWSTR, settingName: PCWSTR,
        pvBuffer: PWSTR, dwBuffer: SIZE_T, pdwWrittenOrRequired: *mut SIZE_T,
    ) -> BOOL;
    pub fn QueryActCtxW(
        dwFlags: DWORD, hActCtx: HANDLE, pvSubInstance: PVOID, ulInfoClass: ULONG, pvBuffer: PVOID,
        cbBuffer: SIZE_T, pcbWrittenOrRequired: *mut SIZE_T,
    ) -> BOOL;
    pub fn QueryDepthSList(ListHead: PSLIST_HEADER) -> USHORT;
    pub fn QueryDosDeviceA(lpDeviceName: LPCSTR, lpTargetPath: LPSTR, ucchMax: DWORD) -> DWORD;
    pub fn QueryDosDeviceW(lpDeviceName: LPCWSTR, lpTargetPath: LPWSTR, ucchMax: DWORD) -> DWORD;
    pub fn QueryFullProcessImageNameA(
        hProcess: HANDLE, dwFlags: DWORD, lpExeName: LPSTR, lpdwSize: PDWORD,
    ) -> BOOL;
    pub fn QueryFullProcessImageNameW(
        hProcess: HANDLE, dwFlags: DWORD, lpExeName: LPWSTR, lpdwSize: PDWORD,
    ) -> BOOL;
    pub fn QueryIdleProcessorCycleTime(
        BufferLength: PULONG, ProcessorIdleCycleTime: PULONG64,
    ) -> BOOL;
    pub fn QueryIdleProcessorCycleTimeEx(
        Group: USHORT, BufferLength: PULONG, ProcessorIdleCycleTime: PULONG64,
    ) -> BOOL;
    pub fn QueryInformationJobObject(
        hJob: HANDLE, JobObjectInformationClass: JOBOBJECTINFOCLASS,
        lpJobObjectInformation: LPVOID, cbJobObjectInformationLength: DWORD,
        lpReturnLength: LPDWORD,
    ) -> BOOL;
    pub fn QueryMemoryResourceNotification(
        ResourceNotificationHandle: HANDLE, ResourceState: PBOOL,
    ) -> BOOL;
    pub fn QueryPerformanceCounter(lpPerformanceCount: *mut LARGE_INTEGER) -> BOOL;
    pub fn QueryPerformanceFrequency(lpFrequency: *mut LARGE_INTEGER) -> BOOL;
    pub fn QueryProcessAffinityUpdateMode(hProcess: HANDLE, lpdwFlags: LPDWORD) -> BOOL;
    pub fn QueryProcessCycleTime(ProcessHandle: HANDLE, CycleTime: PULONG64) -> BOOL;
    pub fn QueryProtectedPolicy(PolicyGuid: LPCGUID, PolicyValue: PULONG_PTR) -> BOOL;
    pub fn QueryThreadCycleTime(ThreadHandle: HANDLE, CycleTime: PULONG64) -> BOOL;
    pub fn QueryThreadProfiling(ThreadHandle: HANDLE, Enabled: PBOOLEAN) -> DWORD;
    pub fn QueryThreadpoolStackInformation(
        ptpp: PTP_POOL, ptpsi: PTP_POOL_STACK_INFORMATION,
    ) -> BOOL;
    #[cfg(target_arch = "x86_64")]
    pub fn QueryUmsThreadInformation(
        UmsThread: PUMS_CONTEXT, UmsThreadInfoClass: UMS_THREAD_INFO_CLASS,
        UmsThreadInformation: PVOID, UmsThreadInformationLength: ULONG, ReturnLength: PULONG,
    ) -> BOOL;
    pub fn QueryUnbiasedInterruptTime(UnbiasedTime: PULONGLONG) -> BOOL;
    pub fn QueueUserAPC(pfnAPC: PAPCFUNC, hThread: HANDLE, dwData: ULONG_PTR) -> DWORD;
    pub fn QueueUserWorkItem(
        Function: LPTHREAD_START_ROUTINE, Context: PVOID, Flags: ULONG,
    ) -> BOOL;
    pub fn RaiseException(
        dwExceptionCode: DWORD, dwExceptionFlags: DWORD, nNumberOfArguments: DWORD,
        lpArguments: *const ULONG_PTR,
    );
    pub fn RaiseFailFastException(
        pExceptionRecord: PEXCEPTION_RECORD, pContextRecord: PCONTEXT, dwFlags: DWORD,
    );
    pub fn ReOpenFile(
        hOriginalFile: HANDLE, dwDesiredAccess: DWORD, dwShareMode: DWORD, dwFlags: DWORD,
    ) -> HANDLE;
    pub fn ReadConsoleA(
        hConsoleInput: HANDLE, lpBuffer: LPVOID, nNumberOfCharsToRead: DWORD,
        lpNumberOfCharsRead: LPDWORD, pInputControl: PCONSOLE_READCONSOLE_CONTROL,
    ) -> BOOL;
    pub fn ReadConsoleInputA(
        hConsoleInput: HANDLE, lpBuffer: PINPUT_RECORD, nLength: DWORD,
        lpNumberOfEventsRead: LPDWORD,
    ) -> BOOL;
    pub fn ReadConsoleInputW(
        hConsoleInput: HANDLE, lpBuffer: PINPUT_RECORD, nLength: DWORD,
        lpNumberOfEventsRead: LPDWORD,
    ) -> BOOL;
    pub fn ReadConsoleOutputA(
        hConsoleOutput: HANDLE, lpBuffer: PCHAR_INFO, dwBufferSize: COORD, dwBufferCoord: COORD,
        lpReadRegion: PSMALL_RECT,
    ) -> BOOL;
    pub fn ReadConsoleOutputAttribute(
        hConsoleOutput: HANDLE, lpAttribute: LPWORD, nLength: DWORD, dwReadCoord: COORD,
        lpNumberOfAttrsRead: LPDWORD,
    ) -> BOOL;
    pub fn ReadConsoleOutputCharacterA(
        hConsoleOutput: HANDLE, lpCharacter: LPSTR, nLength: DWORD, dwReadCoord: COORD,
        lpNumberOfCharsRead: LPDWORD,
    ) -> BOOL;
    pub fn ReadConsoleOutputCharacterW(
        hConsoleOutput: HANDLE, lpCharacter: LPWSTR, nLength: DWORD, dwReadCoord: COORD,
        lpNumberOfCharsRead: LPDWORD,
    ) -> BOOL;
    pub fn ReadConsoleOutputW(
        hConsoleOutput: HANDLE, lpBuffer: PCHAR_INFO, dwBufferSize: COORD, dwBufferCoord: COORD,
        lpReadRegion: PSMALL_RECT,
    ) -> BOOL;
    pub fn ReadConsoleW(
        hConsoleInput: HANDLE, lpBuffer: LPVOID, nNumberOfCharsToRead: DWORD,
        lpNumberOfCharsRead: LPDWORD, pInputControl: PCONSOLE_READCONSOLE_CONTROL,
    ) -> BOOL;
    pub fn ReadDirectoryChangesW(
        hDirectory: HANDLE, lpBuffer: LPVOID, nBufferLength: DWORD, bWatchSubtree: BOOL,
        dwNotifyFilter: DWORD, lpBytesReturned: LPDWORD, lpOverlapped: LPOVERLAPPED,
        lpCompletionRoutine: LPOVERLAPPED_COMPLETION_ROUTINE,
    ) -> BOOL;
    pub fn ReadFile(
        hFile: HANDLE, lpBuffer: LPVOID, nNumberOfBytesToRead: DWORD, lpNumberOfBytesRead: LPDWORD,
        lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
    pub fn ReadFileEx(
        hFile: HANDLE, lpBuffer: LPVOID, nNumberOfBytesToRead: DWORD, lpOverlapped: LPOVERLAPPED,
        lpCompletionRoutine: LPOVERLAPPED_COMPLETION_ROUTINE,
    ) -> BOOL;
    pub fn ReadFileScatter(
        hFile: HANDLE, aSegmentArray: *mut FILE_SEGMENT_ELEMENT, nNumberOfBytesToRead: DWORD,
        lpReserved: LPDWORD, lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
    pub fn ReadProcessMemory(
        hProcess: HANDLE, lpBaseAddress: LPCVOID, lpBuffer: LPVOID, nSize: SIZE_T,
        lpNumberOfBytesRead: *mut SIZE_T,
    ) -> BOOL;
    pub fn ReadThreadProfilingData(
        PerformanceDataHandle: HANDLE, Flags: DWORD, PerformanceData: PPERFORMANCE_DATA,
    ) -> DWORD;
    pub fn RegisterApplicationRecoveryCallback(
        pRecoveyCallback: APPLICATION_RECOVERY_CALLBACK, pvParameter: PVOID, dwPingInterval: DWORD,
        dwFlags: DWORD,
    ) -> HRESULT;
    pub fn RegisterApplicationRestart(pwzCommandline: PCWSTR, dwFlags: DWORD) -> HRESULT;
    pub fn RegisterBadMemoryNotification(Callback: PBAD_MEMORY_CALLBACK_ROUTINE) -> PVOID;
    // pub fn RegisterWaitForInputIdle();
    pub fn RegisterWaitForSingleObject(
        phNewWaitObject: PHANDLE, hObject: HANDLE, Callback: WAITORTIMERCALLBACK, Context: PVOID,
        dwMilliseconds: ULONG, dwFlags: ULONG,
    ) -> BOOL;
    pub fn RegisterWaitForSingleObjectEx(
        hObject: HANDLE, Callback: WAITORTIMERCALLBACK, Context: PVOID, dwMilliseconds: ULONG,
        dwFlags: ULONG,
    ) -> HANDLE;
    // pub fn RegisterWaitUntilOOBECompleted();
    pub fn ReleaseActCtx(hActCtx: HANDLE);
    pub fn ReleaseMutex(hMutex: HANDLE) -> BOOL;
    pub fn ReleaseMutexWhenCallbackReturns(pci: PTP_CALLBACK_INSTANCE, mutex: HANDLE);
    pub fn ReleaseSRWLockExclusive(SRWLock: PSRWLOCK);
    pub fn ReleaseSRWLockShared(SRWLock: PSRWLOCK);
    pub fn ReleaseSemaphore(
        hSemaphore: HANDLE, lReleaseCount: LONG, lpPreviousCount: LPLONG,
    ) -> BOOL;
    pub fn ReleaseSemaphoreWhenCallbackReturns(
        pci: PTP_CALLBACK_INSTANCE, sem: HANDLE, crel: DWORD,
    );
    pub fn RemoveDirectoryA(lpPathName: LPCSTR) -> BOOL;
    pub fn RemoveDirectoryTransactedA(lpPathName: LPCSTR, hTransaction: HANDLE) -> BOOL;
    pub fn RemoveDirectoryTransactedW(lpPathName: LPCWSTR, hTransaction: HANDLE) -> BOOL;
    pub fn RemoveDirectoryW(lpPathName: LPCWSTR) -> BOOL;
    pub fn RemoveDllDirectory(Cookie: DLL_DIRECTORY_COOKIE) -> BOOL;
    // pub fn RemoveLocalAlternateComputerNameA();
    // pub fn RemoveLocalAlternateComputerNameW();
    pub fn RemoveSecureMemoryCacheCallback(pfnCallBack: PSECURE_MEMORY_CACHE_CALLBACK) -> BOOL;
    pub fn RemoveVectoredContinueHandler(Handle: PVOID) -> ULONG;
    pub fn RemoveVectoredExceptionHandler(Handle: PVOID) -> ULONG;
    pub fn ReplaceFileA(
        lpReplacedFileName: LPCSTR, lpReplacementFileName: LPCSTR, lpBackupFileName: LPCSTR,
        dwReplaceFlags: DWORD, lpExclude: LPVOID, lpReserved: LPVOID,
    );
    pub fn ReplaceFileW(
        lpReplacedFileName: LPCWSTR, lpReplacementFileName: LPCWSTR, lpBackupFileName: LPCWSTR,
        dwReplaceFlags: DWORD, lpExclude: LPVOID, lpReserved: LPVOID,
    );
    pub fn ReplacePartitionUnit(
        TargetPartition: PWSTR, SparePartition: PWSTR, Flags: ULONG,
    ) -> BOOL;
    pub fn RequestDeviceWakeup(hDevice: HANDLE) -> BOOL;
    pub fn RequestWakeupLatency(latency: LATENCY_TIME) -> BOOL;
    pub fn ResetEvent(hEvent: HANDLE) -> BOOL;
    pub fn ResetWriteWatch(lpBaseAddress: LPVOID, dwRegionSize: SIZE_T) -> UINT;
    // pub fn ResolveDelayLoadedAPI();
    // pub fn ResolveDelayLoadsFromDll();
    pub fn ResolveLocaleName(
        lpNameToResolve: LPCWSTR, lpLocaleName: LPWSTR, cchLocaleName: c_int,
    ) -> c_int;
    pub fn RestoreLastError(dwErrCode: DWORD);
    pub fn ResumeThread(hThread: HANDLE) -> DWORD;
    #[cfg(target_arch = "arm")]
    pub fn RtlAddFunctionTable(
        FunctionTable: PRUNTIME_FUNCTION, EntryCount: DWORD, BaseAddress: DWORD,
    ) -> BOOLEAN;
    #[cfg(target_arch = "x86_64")]
    pub fn RtlAddFunctionTable(
        FunctionTable: PRUNTIME_FUNCTION, EntryCount: DWORD, BaseAddress: DWORD64,
    ) -> BOOLEAN;
    pub fn RtlCaptureContext(ContextRecord: PCONTEXT);
    pub fn RtlCaptureStackBackTrace(
        FramesToSkip: DWORD, FramesToCapture: DWORD, BackTrace: *mut PVOID, BackTraceHash: PDWORD,
    ) -> WORD;
    // #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn RtlCompareMemory(Source1: *const VOID, Source2: *const VOID, Length: SIZE_T) -> SIZE_T;
    // #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn RtlCopyMemory(Destination: PVOID, Source: *const VOID, Length: SIZE_T);
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn RtlDeleteFunctionTable(FunctionTable: PRUNTIME_FUNCTION) -> BOOLEAN;
    // pub fn RtlFillMemory();
    #[cfg(target_arch = "arm")]
    pub fn RtlInstallFunctionTableCallback(
        TableIdentifier: DWORD, BaseAddress: DWORD, Length: DWORD,
        Callback: PGET_RUNTIME_FUNCTION_CALLBACK, Context: PVOID, OutOfProcessCallbackDll: PCWSTR,
    ) -> BOOLEAN;
    #[cfg(target_arch = "x86_64")]
    pub fn RtlInstallFunctionTableCallback(
        TableIdentifier: DWORD64, BaseAddress: DWORD64, Length: DWORD,
        Callback: PGET_RUNTIME_FUNCTION_CALLBACK, Context: PVOID, OutOfProcessCallbackDll: PCWSTR,
    ) -> BOOLEAN;
    #[cfg(target_arch = "arm")]
    pub fn RtlLookupFunctionEntry(
        ControlPc: ULONG_PTR, ImageBase: PDWORD, HistoryTable: PUNWIND_HISTORY_TABLE,
    ) -> PRUNTIME_FUNCTION;
    #[cfg(target_arch = "x86_64")]
    pub fn RtlLookupFunctionEntry(
        ControlPc: DWORD64, ImageBase: PDWORD64, HistoryTable: PUNWIND_HISTORY_TABLE,
    ) -> PRUNTIME_FUNCTION;
    // pub fn RtlMoveMemory();
    // #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn RtlPcToFileHeader(PcValue: PVOID, BaseOfImage: *mut PVOID) -> PVOID;
    // #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    // pub fn RtlRaiseException();
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn RtlRestoreContext(ContextRecord: PCONTEXT, ExceptionRecord: *mut EXCEPTION_RECORD);
    pub fn RtlUnwind(
        TargetFrame: PVOID, TargetIp: PVOID, ExceptionRecord: PEXCEPTION_RECORD, ReturnValue: PVOID,
    );
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn RtlUnwindEx(
        TargetFrame: PVOID, TargetIp: PVOID, ExceptionRecord: PEXCEPTION_RECORD, ReturnValue: PVOID,
        ContextRecord: PCONTEXT, HistoryTable: PUNWIND_HISTORY_TABLE,
    );
    #[cfg(target_arch = "arm")]
    pub fn RtlVirtualUnwind(
        HandlerType: DWORD, ImageBase: DWORD, ControlPc: DWORD, FunctionEntry: PRUNTIME_FUNCTION,
        ContextRecord: PCONTEXT, HandlerData: *mut PVOID, EstablisherFrame: PDWORD,
        ContextPointers: PKNONVOLATILE_CONTEXT_POINTERS,
    ) -> PEXCEPTION_ROUTINE;
    #[cfg(target_arch = "x86_64")]
    pub fn RtlVirtualUnwind(
        HandlerType: DWORD, ImageBase: DWORD64, ControlPc: DWORD64,
        FunctionEntry: PRUNTIME_FUNCTION, ContextRecord: PCONTEXT, HandlerData: *mut PVOID,
        EstablisherFrame: PDWORD64, ContextPointers: PKNONVOLATILE_CONTEXT_POINTERS,
    ) -> PEXCEPTION_ROUTINE;
    // pub fn RtlZeroMemory();
    pub fn ScrollConsoleScreenBufferA(
        hConsoleOutput: HANDLE, lpScrollRectangle: *const SMALL_RECT,
        lpClipRectangle: *const SMALL_RECT, dwDestinationOrigin: COORD, lpFill: *const CHAR_INFO,
    ) -> BOOL;
    pub fn ScrollConsoleScreenBufferW(
        hConsoleOutput: HANDLE, lpScrollRectangle: *const SMALL_RECT,
        lpClipRectangle: *const SMALL_RECT, dwDestinationOrigin: COORD, lpFill: *const CHAR_INFO,
    ) -> BOOL;
    pub fn SearchPathA(
        lpPath: LPCSTR, lpFileName: LPCSTR, lpExtension: LPCSTR, nBufferLength: DWORD,
        lpBuffer: LPSTR, lpFilePart: *mut LPSTR,
    ) -> DWORD;
    pub fn SearchPathW(
        lpPath: LPCWSTR, lpFileName: LPCWSTR, lpExtension: LPCWSTR, nBufferLength: DWORD,
        lpBuffer: LPWSTR, lpFilePart: *mut LPWSTR,
    ) -> DWORD;
    pub fn SetCachedSigningLevel(
        SourceFiles: PHANDLE, SourceFileCount: ULONG, Flags: ULONG, TargetFile: HANDLE,
    ) -> BOOL;
    pub fn SetCalendarInfoA(
        Locale: LCID, Calendar: CALID, CalType: CALTYPE, lpCalData: LPCSTR,
    ) -> BOOL;
    pub fn SetCalendarInfoW(
        Locale: LCID, Calendar: CALID, CalType: CALTYPE, lpCalData: LPCWSTR,
    ) -> BOOL;
    pub fn SetCommBreak(hFile: HANDLE) -> BOOL;
    pub fn SetCommConfig(hCommDev: HANDLE, lpCC: LPCOMMCONFIG, dwSize: DWORD) -> BOOL;
    pub fn SetCommMask(hFile: HANDLE, dwEvtMask: DWORD) -> BOOL;
    pub fn SetCommState(hFile: HANDLE, lpDCB: LPDCB) -> BOOL;
    pub fn SetCommTimeouts(hFile: HANDLE, lpCommTimeouts: LPCOMMTIMEOUTS) -> BOOL;
    pub fn SetComputerNameA(lpComputerName: LPCSTR) -> BOOL;
    pub fn SetComputerNameEx2W(
        NameType: COMPUTER_NAME_FORMAT, Flags: DWORD, lpBuffer: LPCWSTR,
    ) -> BOOL;
    pub fn SetComputerNameExA(NameType: COMPUTER_NAME_FORMAT, lpBuffer: LPCSTR) -> BOOL;
    pub fn SetComputerNameExW(NameType: COMPUTER_NAME_FORMAT, lpBuffer: LPCWSTR) -> BOOL;
    pub fn SetComputerNameW(lpComputerName: LPCWSTR) -> BOOL;
    pub fn SetConsoleActiveScreenBuffer(hConsoleOutput: HANDLE) -> BOOL;
    pub fn SetConsoleCP(wCodePageID: UINT) -> BOOL;
    pub fn SetConsoleCtrlHandler(HandlerRoutine: PHANDLER_ROUTINE, Add: BOOL) -> BOOL;
    // pub fn SetConsoleCursor();
    pub fn SetConsoleCursorInfo(
        hConsoleOutput: HANDLE, lpConsoleCursorInfo: *const CONSOLE_CURSOR_INFO,
    ) -> BOOL;
    pub fn SetConsoleCursorPosition(hConsoleOutput: HANDLE, dwCursorPosition: COORD) -> BOOL;
    pub fn SetConsoleDisplayMode(
        hConsoleOutput: HANDLE, dwFlags: DWORD, lpNewScreenBufferDimensions: PCOORD,
    ) -> BOOL;
    pub fn SetConsoleHistoryInfo(lpConsoleHistoryInfo: PCONSOLE_HISTORY_INFO) -> BOOL;
    pub fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: DWORD) -> BOOL;
    pub fn SetConsoleOutputCP(wCodePageID: UINT) -> BOOL;
    pub fn SetConsoleScreenBufferInfoEx(
        hConsoleOutput: HANDLE, lpConsoleScreenBufferInfoEx: PCONSOLE_SCREEN_BUFFER_INFOEX,
    ) -> BOOL;
    pub fn SetConsoleScreenBufferSize(hConsoleOutput: HANDLE, dwSize: COORD) -> BOOL;
    pub fn SetConsoleTextAttribute(hConsoleOutput: HANDLE, wAttributes: WORD) -> BOOL;
    pub fn SetConsoleTitleA(lpConsoleTitle: LPCSTR) -> BOOL;
    pub fn SetConsoleTitleW(lpConsoleTitle: LPCWSTR) -> BOOL;
    pub fn SetConsoleWindowInfo(
        hConsoleOutput: HANDLE, bAbsolute: BOOL, lpConsoleWindow: *const SMALL_RECT,
    ) -> BOOL;
    pub fn SetCriticalSectionSpinCount(
        lpCriticalSection: LPCRITICAL_SECTION, dwSpinCount: DWORD,
    ) -> DWORD;
    pub fn SetCurrentConsoleFontEx(
        hConsoleOutput: HANDLE, bMaximumWindow: BOOL, lpConsoleCurrentFontEx: PCONSOLE_FONT_INFOEX,
    ) -> BOOL;
    pub fn SetCurrentDirectoryA(lpPathName: LPCSTR) -> BOOL;
    pub fn SetCurrentDirectoryW(lpPathName: LPCWSTR) -> BOOL;
    pub fn SetDefaultCommConfigA(lpszName: LPCSTR, lpCC: LPCOMMCONFIG, dwSize: DWORD) -> BOOL;
    pub fn SetDefaultCommConfigW(lpszName: LPCWSTR, lpCC: LPCOMMCONFIG, dwSize: DWORD) -> BOOL;
    pub fn SetDefaultDllDirectories(DirectoryFlags: DWORD) -> BOOL;
    pub fn SetDllDirectoryA(lpPathName: LPCSTR) -> BOOL;
    pub fn SetDllDirectoryW(lpPathName: LPCWSTR) -> BOOL;
    pub fn SetDynamicTimeZoneInformation(
        lpTimeZoneInformation: *const DYNAMIC_TIME_ZONE_INFORMATION,
    ) -> BOOL;
    pub fn SetEndOfFile(hFile: HANDLE) -> BOOL;
    pub fn SetEnvironmentStringsA(NewEnvironment: LPCH) -> BOOL;
    pub fn SetEnvironmentStringsW(NewEnvironment: LPWCH) -> BOOL;
    pub fn SetEnvironmentVariableA(lpName: LPCSTR, lpValue: LPCSTR) -> BOOL;
    pub fn SetEnvironmentVariableW(lpName: LPCWSTR, lpValue: LPCWSTR) -> BOOL;
    pub fn SetErrorMode(uMode: UINT) -> UINT;
    pub fn SetEvent(hEvent: HANDLE) -> BOOL;
    pub fn SetEventWhenCallbackReturns(pci: PTP_CALLBACK_INSTANCE, evt: HANDLE);
    pub fn SetFileApisToANSI();
    pub fn SetFileApisToOEM();
    pub fn SetFileAttributesA(lpFileName: LPCSTR, dwFileAttributes: DWORD) -> BOOL;
    pub fn SetFileAttributesTransactedA(
        lpFileName: LPCSTR, dwFileAttributes: DWORD, hTransaction: HANDLE,
    ) -> BOOL;
    pub fn SetFileAttributesTransactedW(
        lpFileName: LPCWSTR, dwFileAttributes: DWORD, hTransaction: HANDLE,
    ) -> BOOL;
    pub fn SetFileAttributesW(lpFileName: LPCWSTR, dwFileAttributes: DWORD) -> BOOL;
    pub fn SetFileBandwidthReservation(
        hFile: HANDLE, nPeriodMilliseconds: DWORD, nBytesPerPeriod: DWORD, bDiscardable: BOOL,
        lpTransferSize: LPDWORD, lpNumOutstandingRequests: LPDWORD,
    ) -> BOOL;
    pub fn SetFileCompletionNotificationModes(FileHandle: HANDLE, Flags: UCHAR) -> BOOL;
    pub fn SetFileInformationByHandle(
        hFile: HANDLE, FileInformationClass: FILE_INFO_BY_HANDLE_CLASS, lpFileInformation: LPVOID,
        dwBufferSize: DWORD,
    ) -> BOOL;
    pub fn SetFileIoOverlappedRange(
        FileHandle: HANDLE, OverlappedRangeStart: PUCHAR, Length: ULONG,
    ) -> BOOL;
    pub fn SetFilePointer(
        hFile: HANDLE, lDistanceToMove: LONG, lpDistanceToMoveHigh: PLONG, dwMoveMethod: DWORD,
    ) -> DWORD;
    pub fn SetFilePointerEx(
        hFile: HANDLE, liDistanceToMove: LARGE_INTEGER, lpNewFilePointer: PLARGE_INTEGER,
        dwMoveMethod: DWORD,
    ) -> BOOL;
    pub fn SetFileShortNameA(hFile: HANDLE, lpShortName: LPCSTR) -> BOOL;
    pub fn SetFileShortNameW(hFile: HANDLE, lpShortName: LPCWSTR) -> BOOL;
    pub fn SetFileTime(
        hFile: HANDLE, lpCreationTime: *const FILETIME, lpLastAccessTime: *const FILETIME,
        lpLastWriteTime: *const FILETIME,
    ) -> BOOL;
    pub fn SetFileValidData(hFile: HANDLE, ValidDataLength: LONGLONG) -> BOOL;
    pub fn SetFirmwareEnvironmentVariableA(
        lpName: LPCSTR, lpGuid: LPCSTR, pValue: PVOID, nSize: DWORD,
    ) -> BOOL;
    pub fn SetFirmwareEnvironmentVariableExA(
        lpName: LPCSTR, lpGuid: LPCSTR, pValue: PVOID, nSize: DWORD, dwAttributes: DWORD,
    ) -> BOOL;
    pub fn SetFirmwareEnvironmentVariableExW(
        lpName: LPCWSTR, lpGuid: LPCWSTR, pValue: PVOID, nSize: DWORD, dwAttributes: DWORD,
    ) -> BOOL;
    pub fn SetFirmwareEnvironmentVariableW(
        lpName: LPCWSTR, lpGuid: LPCWSTR, pValue: PVOID, nSize: DWORD,
    ) -> BOOL;
    pub fn SetHandleCount(uNumber: UINT) -> UINT;
    pub fn SetHandleInformation(hObject: HANDLE, dwMask: DWORD, dwFlags: DWORD) -> BOOL;
    pub fn SetInformationJobObject(
        hJob: HANDLE, JobObjectInformationClass: JOBOBJECTINFOCLASS,
        lpJobObjectInformation: LPVOID, cbJobObjectInformationLength: DWORD,
    ) -> BOOL;
    pub fn SetLastError(dwErrCode: DWORD);
    // pub fn SetLocalPrimaryComputerNameA();
    // pub fn SetLocalPrimaryComputerNameW();
    pub fn SetLocalTime(lpSystemTime: *const SYSTEMTIME) -> BOOL;
    pub fn SetLocaleInfoA(Locale: LCID, LCType: LCTYPE, lpLCData: LPCSTR) -> BOOL;
    pub fn SetLocaleInfoW(Locale: LCID, LCType: LCTYPE, lpLCData: LPCWSTR) -> BOOL;
    pub fn SetMailslotInfo(hMailslot: HANDLE, lReadTimeout: DWORD) -> BOOL;
    pub fn SetMessageWaitingIndicator(hMsgIndicator: HANDLE, ulMsgCount: ULONG) -> BOOL;
    pub fn SetNamedPipeAttribute(
        Pipe: HANDLE, AttributeType: PIPE_ATTRIBUTE_TYPE, AttributeName: PSTR,
        AttributeValue: PVOID, AttributeValueLength: SIZE_T,
    ) -> BOOL;
    pub fn SetNamedPipeHandleState(
        hNamedPipe: HANDLE, lpMode: LPDWORD, lpMaxCollectionCount: LPDWORD,
        lpCollectDataTimeout: LPDWORD,
    ) -> BOOL;
    pub fn SetPriorityClass(hProcess: HANDLE, dwPriorityClass: DWORD) -> BOOL;
    pub fn SetProcessAffinityMask(hProcess: HANDLE, dwProcessAffinityMask: DWORD) -> BOOL;
    pub fn SetProcessAffinityUpdateMode(hProcess: HANDLE, dwFlags: DWORD) -> BOOL;
    pub fn SetProcessDEPPolicy(dwFlags: DWORD) -> BOOL;
    pub fn SetProcessInformation(
        hProcess: HANDLE, ProcessInformationClass: PROCESS_INFORMATION_CLASS,
        ProcessInformation: LPVOID, ProcessInformationSize: DWORD,
    ) -> BOOL;
    pub fn SetProcessMitigationPolicy(
        MitigationPolicy: PROCESS_MITIGATION_POLICY, lpBuffer: PVOID, dwLength: SIZE_T,
    ) -> BOOL;
    pub fn SetProcessPreferredUILanguages(
        dwFlags: DWORD, pwszLanguagesBuffer: PCZZWSTR, pulNumLanguages: PULONG,
    ) -> BOOL;
    pub fn SetProcessPriorityBoost(hProcess: HANDLE, bDisablePriorityBoost: BOOL) -> BOOL;
    pub fn SetProcessShutdownParameters(dwLevel: DWORD, dwFlags: DWORD) -> BOOL;
    pub fn SetProcessWorkingSetSize(
        hProcess: HANDLE, dwMinimumWorkingSetSize: SIZE_T, dwMaximumWorkingSetSize: SIZE_T,
    ) -> BOOL;
    pub fn SetProcessWorkingSetSizeEx(
        hProcess: HANDLE, dwMinimumWorkingSetSize: SIZE_T, dwMaximumWorkingSetSize: SIZE_T,
        Flags: DWORD,
    ) -> BOOL;
    pub fn SetProtectedPolicy(
        PolicyGuid: LPCGUID, PolicyValue: ULONG_PTR, OldPolicyValue: PULONG_PTR,
    ) -> BOOL;
    pub fn SetSearchPathMode(Flags: DWORD) -> BOOL;
    pub fn SetStdHandle(nStdHandle: DWORD, hHandle: HANDLE) -> BOOL;
    pub fn SetStdHandleEx(nStdHandle: DWORD, hHandle: HANDLE, phPrevValue: PHANDLE) -> BOOL;
    pub fn SetSystemFileCacheSize(
        MinimumFileCacheSize: SIZE_T, MaximumFileCacheSize: SIZE_T, Flags: DWORD,
    ) -> BOOL;
    pub fn SetSystemPowerState(fSuspend: BOOL, fForce: BOOL) -> BOOL;
    pub fn SetSystemTime(lpSystemTime: *const SYSTEMTIME) -> BOOL;
    pub fn SetSystemTimeAdjustment(dwTimeAdjustment: DWORD, bTimeAdjustmentDisabled: BOOL) -> BOOL;
    pub fn SetTapeParameters(
        hDevice: HANDLE, dwOperation: DWORD, lpTapeInformation: LPVOID,
    ) -> DWORD;
    pub fn SetTapePosition(
        hDevice: HANDLE, dwPositionMethod: DWORD, dwPartition: DWORD,
        dwOffsetLow: DWORD, dwOffsetHigh: DWORD, bImmediate: BOOL
    ) -> DWORD;
    pub fn SetThreadAffinityMask(hThread: HANDLE, dwThreadAffinityMask: DWORD_PTR) -> DWORD_PTR;
    pub fn SetThreadContext(hThread: HANDLE, lpContext: *const CONTEXT) -> BOOL;
    pub fn SetThreadErrorMode(dwNewMode: DWORD, lpOldMode: LPDWORD) -> BOOL;
    pub fn SetThreadExecutionState(esFlags: EXECUTION_STATE) -> EXECUTION_STATE;
    pub fn SetThreadGroupAffinity(
        hThread: HANDLE, GroupAffinity: *const GROUP_AFFINITY,
        PreviousGroupAffinity: PGROUP_AFFINITY,
    ) -> BOOL;
    pub fn SetThreadIdealProcessor(hThread: HANDLE, dwIdealProcessor: DWORD) -> DWORD;
    pub fn SetThreadIdealProcessorEx(
        hThread: HANDLE, lpIdealProcessor: PPROCESSOR_NUMBER,
        lpPreviousIdealProcessor: PPROCESSOR_NUMBER,
    ) -> BOOL;
    pub fn SetThreadInformation(
        hThread: HANDLE, ThreadInformationClass: THREAD_INFORMATION_CLASS,
        ThreadInformation: LPVOID, ThreadInformationSize: DWORD,
    );
    pub fn SetThreadLocale(Locale: LCID) -> BOOL;
    pub fn SetThreadPreferredUILanguages(
        dwFlags: DWORD, pwszLanguagesBuffer: PCZZWSTR, pulNumLanguages: PULONG,
    ) -> BOOL;
    pub fn SetThreadPriority(hThread: HANDLE, nPriority: c_int) -> BOOL;
    pub fn SetThreadPriorityBoost(hThread: HANDLE, bDisablePriorityBoost: BOOL) -> BOOL;
    pub fn SetThreadStackGuarantee(StackSizeInBytes: PULONG) -> BOOL;
    pub fn SetThreadUILanguage(LangId: LANGID) -> LANGID;
    pub fn SetThreadpoolStackInformation(
        ptpp: PTP_POOL, ptpsi: PTP_POOL_STACK_INFORMATION,
    ) -> BOOL;
    pub fn SetThreadpoolThreadMaximum(ptpp: PTP_POOL, cthrdMost: DWORD);
    pub fn SetThreadpoolThreadMinimum(ptpp: PTP_POOL, cthrdMic: DWORD) -> BOOL;
    pub fn SetThreadpoolTimer(
        pti: PTP_TIMER, pftDueTime: PFILETIME, msPeriod: DWORD, msWindowLength: DWORD,
    );
    pub fn SetThreadpoolTimerEx(
        pti: PTP_TIMER, pftDueTime: PFILETIME, msPeriod: DWORD, msWindowLength: DWORD,
    ) -> BOOL;
    pub fn SetThreadpoolWait(pwa: PTP_WAIT, h: HANDLE, pftTimeout: PFILETIME);
    pub fn SetThreadpoolWaitEx(
        pwa: PTP_WAIT, h: HANDLE, pftTimeout: PFILETIME, Reserved: PVOID,
    ) -> BOOL;
    pub fn SetTimeZoneInformation(lpTimeZoneInformation: *const TIME_ZONE_INFORMATION) -> BOOL;
    pub fn SetTimerQueueTimer(
        TimerQueue: HANDLE, Callback: WAITORTIMERCALLBACK, Parameter: PVOID, DueTime: DWORD,
        Period: DWORD, PreferIo: BOOL,
    ) -> HANDLE;
    #[cfg(target_arch = "x86_64")]
    pub fn SetUmsThreadInformation(
        UmsThread: PUMS_CONTEXT, UmsThreadInfoClass: UMS_THREAD_INFO_CLASS,
        UmsThreadInformation: PVOID, UmsThreadInformationLength: ULONG,
    ) -> BOOL;
    pub fn SetUnhandledExceptionFilter(
        lpTopLevelExceptionFilter: LPTOP_LEVEL_EXCEPTION_FILTER,
    ) -> LPTOP_LEVEL_EXCEPTION_FILTER;
    pub fn SetUserGeoID(GeoId: GEOID) -> BOOL;
    pub fn SetVolumeLabelA(lpRootPathName: LPCSTR, lpVolumeName: LPCSTR) -> BOOL;
    pub fn SetVolumeLabelW(lpRootPathName: LPCWSTR, lpVolumeName: LPCWSTR) -> BOOL;
    pub fn SetVolumeMountPointA(lpszVolumeMountPoint: LPCSTR, lpszVolumeName: LPCSTR) -> BOOL;
    pub fn SetVolumeMountPointW(lpszVolumeMountPoint: LPCWSTR, lpszVolumeName: LPCWSTR) -> BOOL;
    pub fn SetWaitableTimer(
        hTimer: HANDLE, lpDueTime: *const LARGE_INTEGER, lPeriod: LONG,
        pfnCompletionRoutine: PTIMERAPCROUTINE, lpArgToCompletionRoutine: LPVOID, fResume: BOOL,
    ) -> BOOL;
    pub fn SetWaitableTimerEx(
        hTimer: HANDLE, lpDueTime: *const LARGE_INTEGER, lPeriod: LONG,
        pfnCompletionRoutine: PTIMERAPCROUTINE, lpArgToCompletionRoutine: LPVOID,
        WakeContext: PREASON_CONTEXT, TolerableDelay: ULONG,
    ) -> BOOL;
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub fn SetXStateFeaturesMask(Context: PCONTEXT, FeatureMask: DWORD64) -> BOOL;
    pub fn SetupComm(hFile: HANDLE, dwInQueue: DWORD, dwOutQueue: DWORD) -> BOOL;
    pub fn SignalObjectAndWait(
        hObjectToSignal: HANDLE, hObjectToWaitOn: HANDLE, dwMilliseconds: DWORD, bAlertable: BOOL,
    ) -> DWORD;
    pub fn SizeofResource(hModule: HMODULE, hResInfo: HRSRC) -> DWORD;
    pub fn Sleep(dwMilliseconds: DWORD);
    pub fn SleepConditionVariableCS(
        ConditionVariable: PCONDITION_VARIABLE, CriticalSection: PCRITICAL_SECTION,
        dwMilliseconds: DWORD,
    ) -> BOOL;
    pub fn SleepConditionVariableSRW(
        ConditionVariable: PCONDITION_VARIABLE, SRWLock: PSRWLOCK, dwMilliseconds: DWORD,
        Flags: ULONG,
    ) -> BOOL;
    pub fn SleepEx(dwMilliseconds: DWORD, bAlertable: BOOL) -> DWORD;
    pub fn StartThreadpoolIo(pio: PTP_IO);
    pub fn SubmitThreadpoolWork(pwk: PTP_WORK);
    pub fn SuspendThread(hThread: HANDLE) -> DWORD;
    pub fn SwitchToFiber(lpFiber: LPVOID);
    pub fn SwitchToThread() -> BOOL;
    pub fn SystemTimeToFileTime(lpSystemTime: *const SYSTEMTIME, lpFileTime: LPFILETIME) -> BOOL;
    pub fn SystemTimeToTzSpecificLocalTime(
        lpTimeZoneInformation: *const TIME_ZONE_INFORMATION, lpUniversalTime: *const SYSTEMTIME,
        lpLocalTime: LPSYSTEMTIME,
    ) -> BOOL;
    pub fn SystemTimeToTzSpecificLocalTimeEx(
        lpTimeZoneInformation: *const DYNAMIC_TIME_ZONE_INFORMATION,
        lpUniversalTime: *const SYSTEMTIME, lpLocalTime: LPSYSTEMTIME,
    ) -> BOOL;
    pub fn TerminateJobObject(hJob: HANDLE, uExitCode: UINT) -> BOOL;
    pub fn TerminateProcess(hProcess: HANDLE, uExitCode: UINT) -> BOOL;
    pub fn TerminateThread(hThread: HANDLE, dwExitCode: DWORD) -> BOOL;
    pub fn Thread32First(hSnapshot: HANDLE, lpte: LPTHREADENTRY32) -> BOOL;
    pub fn Thread32Next(hSnapshot: HANDLE, lpte: LPTHREADENTRY32) -> BOOL;
    pub fn TlsAlloc() -> DWORD;
    pub fn TlsFree(dwTlsIndex: DWORD) -> BOOL;
    pub fn TlsGetValue(dwTlsIndex: DWORD) -> LPVOID;
    pub fn TlsSetValue(dwTlsIndex: DWORD, lpTlsValue: LPVOID) -> BOOL;
    pub fn Toolhelp32ReadProcessMemory(th32ProcessID: DWORD, lpBaseAddress: LPCVOID,
        lpBuffer: LPVOID, cbRead: SIZE_T, lpNumberOfBytesRead: *mut SIZE_T
    ) -> BOOL;
    pub fn TransactNamedPipe(
        hNamedPipe: HANDLE, lpInBuffer: LPVOID, nInBufferSize: DWORD, lpOutBuffer: LPVOID,
        nOutBufferSize: DWORD, lpBytesRead: LPDWORD, lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
    pub fn TransmitCommChar(hFile: HANDLE, cChar: c_char) -> BOOL;
    pub fn TryAcquireSRWLockExclusive(SRWLock: PSRWLOCK) -> BOOLEAN;
    pub fn TryAcquireSRWLockShared(SRWLock: PSRWLOCK) -> BOOLEAN;
    pub fn TryEnterCriticalSection(lpCriticalSection: LPCRITICAL_SECTION) -> BOOL;
    pub fn TrySubmitThreadpoolCallback(
        pfns: PTP_SIMPLE_CALLBACK, pv: PVOID, pcbe: PTP_CALLBACK_ENVIRON,
    ) -> BOOL;
    pub fn TzSpecificLocalTimeToSystemTime(
        lpTimeZoneInformation: *const TIME_ZONE_INFORMATION, lpLocalTime: *const SYSTEMTIME,
        lpUniversalTime: LPSYSTEMTIME,
    ) -> BOOL;
    pub fn TzSpecificLocalTimeToSystemTimeEx(
        lpTimeZoneInformation: *const DYNAMIC_TIME_ZONE_INFORMATION,
        lpLocalTime: *const SYSTEMTIME, lpUniversalTime: LPSYSTEMTIME,
    ) -> BOOL;
    #[cfg(target_arch = "x86_64")]
    pub fn UmsThreadYield(SchedulerParam: PVOID) -> BOOL;
    pub fn UnhandledExceptionFilter(ExceptionInfo: *mut EXCEPTION_POINTERS) -> LONG;
    pub fn UnlockFile(
        hFile: HANDLE, dwFileOffsetLow: DWORD, dwFileOffsetHigh: DWORD,
        nNumberOfBytesToUnlockLow: DWORD, nNumberOfBytesToUnlockHigh: DWORD,
    ) -> BOOL;
    pub fn UnlockFileEx(
        hFile: HANDLE, dwReserved: DWORD, nNumberOfBytesToUnlockLow: DWORD,
        nNumberOfBytesToUnlockHigh: DWORD, lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
    pub fn UnmapViewOfFile(lpBaseAddress: LPCVOID) -> BOOL;
    pub fn UnregisterApplicationRecoveryCallback() -> HRESULT;
    pub fn UnregisterApplicationRestart() -> HRESULT;
    pub fn UnregisterBadMemoryNotification(RegistrationHandle: PVOID) -> BOOL;
    pub fn UnregisterWait(WaitHandle: HANDLE) -> BOOL;
    pub fn UnregisterWaitEx(WaitHandle: HANDLE, CompletionEvent: HANDLE) -> BOOL;
    // pub fn UnregisterWaitUntilOOBECompleted();
    pub fn UpdateProcThreadAttribute(
        lpAttributeList: LPPROC_THREAD_ATTRIBUTE_LIST, dwFlags: DWORD, Attribute: DWORD_PTR,
        lpValue: PVOID, cbSize: SIZE_T, lpPreviousValue: PVOID, lpReturnSize: PSIZE_T,
    ) -> BOOL;
    pub fn UpdateResourceA(
        hUpdate: HANDLE, lpType: LPCSTR, lpName: LPCSTR, wLanguage: WORD, lpData: LPVOID, cb: DWORD,
    ) -> BOOL;
    pub fn UpdateResourceW(
        hUpdate: HANDLE, lpType: LPCWSTR, lpName: LPCWSTR, wLanguage: WORD, lpData: LPVOID,
        cb: DWORD,
    ) -> BOOL;
    pub fn VerLanguageNameA(wLang: DWORD, szLang: LPSTR, cchLang: DWORD) -> DWORD;
    pub fn VerLanguageNameW(wLang: DWORD, szLang: LPWSTR, cchLang: DWORD) -> DWORD;
    pub fn VerSetConditionMask(
        ConditionMask: ULONGLONG, TypeMask: DWORD, Condition: BYTE,
    ) -> ULONGLONG;
    pub fn VerifyScripts(
        dwFlags: DWORD, lpLocaleScripts: LPCWSTR, cchLocaleScripts: c_int, lpTestScripts: LPCWSTR,
        cchTestScripts: c_int,
    ) -> BOOL;
    pub fn VerifyVersionInfoA(
        lpVersionInformation: LPOSVERSIONINFOEXA, dwTypeMask: DWORD, dwlConditionMask: DWORDLONG,
    ) -> BOOL;
    pub fn VerifyVersionInfoW(
        lpVersionInformation: LPOSVERSIONINFOEXW, dwTypeMask: DWORD, dwlConditionMask: DWORDLONG,
    ) -> BOOL;
    pub fn VirtualAlloc(
        lpAddress: LPVOID, dwSize: SIZE_T, flAllocationType: DWORD, flProtect: DWORD,
    ) -> LPVOID;
    pub fn VirtualAllocEx(
        hProcess: HANDLE, lpAddress: LPVOID, dwSize: SIZE_T, flAllocationType: DWORD,
        flProtect: DWORD,
    ) -> LPVOID;
    pub fn VirtualAllocExNuma(
        hProcess: HANDLE, lpAddress: LPVOID, dwSize: SIZE_T, flAllocationType: DWORD,
        flProtect: DWORD, nndPreferred: DWORD,
    ) -> LPVOID;
    pub fn VirtualFree(lpAddress: LPVOID, dwSize: SIZE_T, dwFreeType: DWORD) -> BOOL;
    pub fn VirtualFreeEx(
        hProcess: HANDLE, lpAddress: LPVOID, dwSize: SIZE_T, dwFreeType: DWORD,
    ) -> BOOL;
    pub fn VirtualLock(lpAddress: LPVOID, dwSize: SIZE_T) -> BOOL;
    pub fn VirtualProtect(
        lpAddress: LPVOID, dwSize: SIZE_T, flNewProtect: DWORD, lpflOldProtect: PDWORD,
    ) -> BOOL;
    pub fn VirtualProtectEx(
        hProcess: HANDLE, lpAddress: LPVOID, dwSize: SIZE_T, flNewProtect: DWORD,
        lpflOldProtect: DWORD,
    ) -> BOOL;
    pub fn VirtualQuery(
        lpAddress: LPCVOID, lpBuffer: PMEMORY_BASIC_INFORMATION, dwLength: SIZE_T,
    ) -> SIZE_T;
    pub fn VirtualQueryEx(
        hProcess: HANDLE, lpAddress: LPCVOID, lpBuffer: PMEMORY_BASIC_INFORMATION, dwLength: SIZE_T,
    ) -> SIZE_T;
    pub fn VirtualUnlock(lpAddress: LPVOID, dwSize: SIZE_T) -> BOOL;
    pub fn WTSGetActiveConsoleSessionId() -> DWORD;
    pub fn WaitCommEvent(hFile: HANDLE, lpEvtMask: LPDWORD, lpOverlapped: LPOVERLAPPED) -> BOOL;
    pub fn WaitForDebugEvent(lpDebugEvent: LPDEBUG_EVENT, dwMilliseconds: DWORD) -> BOOL;
    pub fn WaitForMultipleObjects(
        nCount: DWORD, lpHandles: *const HANDLE, bWaitAll: BOOL, dwMilliseconds: DWORD,
    ) -> DWORD;
    pub fn WaitForMultipleObjectsEx(
        nCount: DWORD, lpHandles: *const HANDLE, bWaitAll: BOOL, dwMilliseconds: DWORD,
        bAlertable: BOOL,
    ) -> DWORD;
    pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
    pub fn WaitForSingleObjectEx(
        hHandle: HANDLE, dwMilliseconds: DWORD, bAlertable: BOOL,
    ) -> DWORD;
    pub fn WaitForThreadpoolIoCallbacks(pio: PTP_IO, fCancelPendingCallbacks: BOOL);
    pub fn WaitForThreadpoolTimerCallbacks(pti: PTP_TIMER, fCancelPendingCallbacks: BOOL);
    pub fn WaitForThreadpoolWaitCallbacks(pwa: PTP_WAIT, fCancelPendingCallbacks: BOOL);
    pub fn WaitForThreadpoolWorkCallbacks(pwk: PTP_WORK, fCancelPendingCallbacks: BOOL);
    pub fn WaitNamedPipeA(lpNamedPipeName: LPCSTR, nTimeOut: DWORD) -> BOOL;
    pub fn WaitNamedPipeW(lpNamedPipeName: LPCWSTR, nTimeOut: DWORD) -> BOOL;
    pub fn WakeAllConditionVariable(ConditionVariable: PCONDITION_VARIABLE);
    pub fn WakeConditionVariable(ConditionVariable: PCONDITION_VARIABLE);
    pub fn WerGetFlags(hProcess: HANDLE, pdwFlags: PDWORD) -> HRESULT;
    pub fn WerRegisterFile(
        pwzFile: PCWSTR, regFileType: WER_REGISTER_FILE_TYPE, dwFlags: DWORD,
    ) -> HRESULT;
    pub fn WerRegisterMemoryBlock(pvAddress: PVOID, dwSize: DWORD) -> HRESULT;
    pub fn WerRegisterRuntimeExceptionModule(
        pwszOutOfProcessCallbackDll: PCWSTR, pContext: PVOID,
    ) -> HRESULT;
    pub fn WerSetFlags(dwFlags: DWORD) -> HRESULT;
    pub fn WerUnregisterFile(pwzFilePath: PCWSTR) -> HRESULT;
    pub fn WerUnregisterMemoryBlock(pvAddress: PVOID) -> HRESULT;
    pub fn WerUnregisterRuntimeExceptionModule(
        pwszOutOfProcessCallbackDll: PCWSTR, pContext: PVOID,
    ) -> HRESULT;
    // pub fn WerpInitiateRemoteRecovery();
    pub fn WideCharToMultiByte(
      CodePage: UINT, dwFlags: DWORD, lpWideCharStr: LPCWSTR, cchWideChar: c_int,
      lpMultiByteStr: LPSTR, cbMultiByte: c_int, lpDefaultChar: LPCSTR, lpUsedDefaultChar: LPBOOL,
    ) -> c_int;
    pub fn WinExec(lpCmdLine: LPCSTR, uCmdShow: UINT) -> UINT;
    pub fn Wow64DisableWow64FsRedirection(OldValue: *mut PVOID) -> BOOL;
    pub fn Wow64EnableWow64FsRedirection(Wow64FsEnableRedirection: BOOLEAN) -> BOOLEAN;
    pub fn Wow64GetThreadContext(hThread: HANDLE, lpContext: PWOW64_CONTEXT) -> BOOL;
    pub fn Wow64GetThreadSelectorEntry(
        hThread: HANDLE, dwSelector: DWORD, lpSelectorEntry: PWOW64_LDT_ENTRY,
    ) -> BOOL;
    pub fn Wow64RevertWow64FsRedirection(OlValue: PVOID) -> BOOL;
    pub fn Wow64SetThreadContext(hThread: HANDLE, lpContext: *const WOW64_CONTEXT) -> BOOL;
    pub fn Wow64SuspendThread(hThread: HANDLE) -> DWORD;
    pub fn WriteConsoleA(
        hConsoleOutput: HANDLE, lpBuffer: *const VOID, nNumberOfCharsToWrite: DWORD,
        lpNumberOfCharsWritten: LPDWORD, lpReserved: LPVOID,
    ) -> BOOL;
    pub fn WriteConsoleInputA(
        hConsoleInput: HANDLE, lpBuffer: *const INPUT_RECORD, nLength: DWORD,
        lpNumberOfEventsWritten: LPDWORD,
    ) -> BOOL;
    pub fn WriteConsoleInputW(
        hConsoleInput: HANDLE, lpBuffer: *const INPUT_RECORD, nLength: DWORD,
        lpNumberOfEventsWritten: LPDWORD,
    ) -> BOOL;
    pub fn WriteConsoleOutputA(
        hConsoleOutput: HANDLE, lpBuffer: *const CHAR_INFO, dwBufferSize: COORD,
        dwBufferCoord: COORD, lpWriteRegion: PSMALL_RECT,
    ) -> BOOL;
    pub fn WriteConsoleOutputAttribute(
        hConsoleOutput: HANDLE, lpAttribute: *const WORD, nLength: DWORD, dwWriteCoord: COORD,
        lpNumberOfAttrsWritten: LPDWORD,
    ) -> BOOL;
    pub fn WriteConsoleOutputCharacterA(
        hConsoleOutput: HANDLE, lpCharacter: LPCSTR, nLength: DWORD, dwWriteCoord: COORD,
        lpNumberOfCharsWritten: LPDWORD,
    ) -> BOOL;
    pub fn WriteConsoleOutputCharacterW(
        hConsoleOutput: HANDLE, lpCharacter: LPCWSTR, nLength: DWORD, dwWriteCoord: COORD,
        lpNumberOfCharsWritten: LPDWORD,
    ) -> BOOL;
    pub fn WriteConsoleOutputW(
        hConsoleOutput: HANDLE, lpBuffer: *const CHAR_INFO, dwBufferSize: COORD,
        dwBufferCoord: COORD, lpWriteRegion: PSMALL_RECT,
    ) -> BOOL;
    pub fn WriteConsoleW(
        hConsoleOutput: HANDLE, lpBuffer: *const VOID, nNumberOfCharsToWrite: DWORD,
        lpNumberOfCharsWritten: LPDWORD, lpReserved: LPVOID,
    ) -> BOOL;
    pub fn WriteFile(
        hFile: HANDLE, lpBuffer: LPCVOID, nNumberOfBytesToWrite: DWORD,
        lpNumberOfBytesWritten: LPDWORD, lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
    pub fn WriteFileEx(
        hFile: HANDLE, lpBuffer: LPCVOID, nNumberOfBytesToWrite: DWORD, lpOverlapped: LPOVERLAPPED,
        lpCompletionRoutine: LPOVERLAPPED_COMPLETION_ROUTINE,
    ) -> BOOL;
    pub fn WriteFileGather(
        hFile: HANDLE, aSegmentArray: *mut FILE_SEGMENT_ELEMENT, nNumberOfBytesToWrite: DWORD,
        lpReserved: LPDWORD, lpOverlapped: LPOVERLAPPED,
    ) -> BOOL;
    pub fn WritePrivateProfileSectionA(
        lpAppName: LPCSTR, lpString: LPCSTR, lpFileName: LPCSTR,
    ) -> BOOL;
    pub fn WritePrivateProfileSectionW(
        lpAppName: LPCWSTR, lpString: LPCWSTR, lpFileName: LPCWSTR,
    ) -> BOOL;
    pub fn WritePrivateProfileStringA(
        lpAppName: LPCSTR, lpKeyName: LPCSTR, lpString: LPCSTR, lpFileName: LPCSTR,
    ) -> BOOL;
    pub fn WritePrivateProfileStringW(
        lpAppName: LPCWSTR, lpKeyName: LPCWSTR, lpString: LPCWSTR, lpFileName: LPCWSTR,
    ) -> BOOL;
    pub fn WritePrivateProfileStructA(
        lpszSection: LPCSTR, lpszKey: LPCSTR, lpStruct: LPVOID, uSizeStruct: UINT, szFile: LPCSTR,
    ) -> BOOL;
    pub fn WritePrivateProfileStructW(
        lpszSection: LPCWSTR, lpszKey: LPCWSTR, lpStruct: LPVOID, uSizeStruct: UINT,
        szFile: LPCWSTR,
    ) -> BOOL;
    pub fn WriteProcessMemory(
        hProcess: HANDLE, lpBaseAddress: LPVOID, lpBuffer: LPCVOID, nSize: SIZE_T,
        lpNumberOfBytesWritten: *mut SIZE_T,
    ) -> BOOL;
    pub fn WriteProfileSectionA(lpAppName: LPCSTR, lpString: LPCSTR) -> BOOL;
    pub fn WriteProfileSectionW(lpAppName: LPCWSTR, lpString: LPCWSTR) -> BOOL;
    pub fn WriteProfileStringA(lpAppName: LPCSTR, lpKeyName: LPCSTR, lpString: LPCSTR) -> BOOL;
    pub fn WriteProfileStringW(lpAppName: LPCWSTR, lpKeyName: LPCWSTR, lpString: LPCWSTR) -> BOOL;
    pub fn WriteTapemark(
        hDevice: HANDLE, dwTapemarkType: DWORD, dwTapemarkCount: DWORD, bImmediate: BOOL,
    ) -> DWORD;
    pub fn ZombifyActCtx(hActCtx: HANDLE) -> BOOL;
    pub fn _hread(hFile: HFILE, lpBuffer: LPVOID, lBytes: c_long) -> c_long;
    pub fn _hwrite(hFile: HFILE, lpBuffer: LPCCH, lBytes: c_long) -> c_long;
    pub fn _lclose(hFile: HFILE) -> HFILE;
    pub fn _lcreat(lpPathName: LPCSTR, iAttrubute: c_int) -> HFILE;
    pub fn _llseek(hFile: HFILE, lOffset: LONG, iOrigin: c_int) -> LONG;
    pub fn _lopen(lpPathName: LPCSTR, iReadWrite: c_int) -> HFILE;
    pub fn _lread(hFile: HFILE, lpBuffer: LPVOID, uBytes: UINT) -> UINT;
    pub fn _lwrite(hFile: HFILE, lpBuffer: LPCCH, uBytes: UINT) -> UINT;
    pub fn lstrcat(lpString1: LPSTR, lpString2: LPCSTR) -> LPSTR;
    pub fn lstrcatA(lpString1: LPSTR, lpString2: LPCSTR) -> LPSTR;
    pub fn lstrcatW(lpString1: LPWSTR, lpString2: LPCWSTR) -> LPSTR;
    pub fn lstrcmp(lpString1: LPCSTR, lpString2: LPCSTR) -> c_int;
    pub fn lstrcmpA(lpString1: LPCSTR, lpString2: LPCSTR) -> c_int;
    pub fn lstrcmpW(lpString1: LPCWSTR, lpString2: LPCWSTR) -> c_int;
    pub fn lstrcmpi(lpString1: LPCSTR, lpString2: LPCSTR) -> c_int;
    pub fn lstrcmpiA(lpString1: LPCSTR, lpString2: LPCSTR) -> c_int;
    pub fn lstrcmpiW(lpString1: LPCWSTR, lpString2: LPCWSTR) -> c_int;
    pub fn lstrcpy(lpString1: LPSTR, lpString2: LPCSTR) -> LPSTR;
    pub fn lstrcpyA(lpString1: LPSTR, lpString2: LPCSTR) -> LPSTR;
    pub fn lstrcpyW(lpString1: LPWSTR, lpString2: LPCWSTR) -> LPSTR;
    pub fn lstrcpyn(lpString1: LPSTR, lpString2: LPCSTR, iMaxLength: c_int) -> LPSTR;
    pub fn lstrcpynA(lpString1: LPSTR, lpString2: LPCSTR, iMaxLength: c_int) -> LPSTR;
    pub fn lstrcpynW(lpString1: LPWSTR, lpString2: LPCWSTR, iMaxLength: c_int) -> LPSTR;
    pub fn lstrlen(lpString: LPCSTR) -> c_int;
    pub fn lstrlenA(lpString: LPCSTR) -> c_int;
    pub fn lstrlenW(lpString: LPCWSTR) -> c_int;
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn uaw_lstrcmpW(String1: PCUWSTR, String2: PCUWSTR) -> c_int;
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn uaw_lstrcmpiW(String1: PCUWSTR, String2: PCUWSTR) -> c_int;
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn uaw_lstrlenW(String: LPCUWSTR) -> c_int;
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn uaw_wcschr(String: PCUWSTR, Character: WCHAR) -> PUWSTR;
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn uaw_wcscpy(Destination: PUWSTR, Source: PCUWSTR) -> PUWSTR;
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn uaw_wcsicmp(String1: PCUWSTR, String2: PCUWSTR) -> c_int;
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn uaw_wcslen(String: PCUWSTR) -> size_t;
    #[cfg(any(target_arch = "arm", target_arch = "x86_64"))]
    pub fn uaw_wcsrchr(String: PCUWSTR, Character: WCHAR) -> PUWSTR;
}
