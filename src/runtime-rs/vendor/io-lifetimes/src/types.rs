use std::fmt;
use std::marker::PhantomData;
use std::mem::forget;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
#[cfg(target_os = "wasi")]
use std::os::wasi::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
#[cfg(windows)]
use std::{
    convert::TryFrom,
    os::windows::io::{
        AsRawHandle, AsRawSocket, FromRawHandle, FromRawSocket, IntoRawHandle, IntoRawSocket,
        RawHandle, RawSocket,
    },
};
#[cfg(all(windows, feature = "close"))]
use winapi::{
    shared::minwindef::{BOOL, DWORD},
    shared::ntdef::HANDLE,
    um::handleapi::DuplicateHandle,
    um::handleapi::SetHandleInformation,
    um::handleapi::INVALID_HANDLE_VALUE,
    um::processthreadsapi::GetCurrentProcess,
    um::processthreadsapi::GetCurrentProcessId,
    um::winbase::HANDLE_FLAG_INHERIT,
    um::winnt::DUPLICATE_SAME_ACCESS,
    um::winsock2::WSADuplicateSocketW,
    um::winsock2::WSAGetLastError,
    um::winsock2::WSASocketW,
    um::winsock2::INVALID_SOCKET,
    um::winsock2::SOCKET_ERROR,
    um::winsock2::WSAEINVAL,
    um::winsock2::WSAEPROTOTYPE,
    um::winsock2::WSAPROTOCOL_INFOW,
    um::winsock2::WSA_FLAG_NO_HANDLE_INHERIT,
    um::winsock2::WSA_FLAG_OVERLAPPED,
};

#[cfg(all(windows, not(feature = "winapi")))]
const INVALID_HANDLE_VALUE: *mut core::ffi::c_void = !0 as _;
#[cfg(all(windows, not(feature = "winapi")))]
const INVALID_SOCKET: usize = !0 as _;

/// A borrowed file descriptor.
///
/// This has a lifetime parameter to tie it to the lifetime of something that
/// owns the file descriptor.
///
/// This uses `repr(transparent)` and has the representation of a host file
/// descriptor, so it can be used in FFI in places where a file descriptor is
/// passed as an argument, it is not captured or consumed, and it never has the
/// value `-1`.
///
/// This type's `.to_owned()` implementation returns another `BorrowedFd`
/// rather than an `OwnedFd`. It just makes a trivial copy of the raw file
/// descriptor, which is then borrowed under the same lifetime.
#[cfg(any(unix, target_os = "wasi"))]
#[derive(Copy, Clone)]
#[repr(transparent)]
#[cfg_attr(rustc_attrs, rustc_nonnull_optimization_guaranteed)]
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_start(0))]
// libstd/os/raw/mod.rs assures me that every libstd-supported platform has a
// 32-bit c_int. Below is -2, in two's complement, but that only works out
// because c_int is 32 bits.
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FE))]
pub struct BorrowedFd<'fd> {
    fd: RawFd,
    _phantom: PhantomData<&'fd OwnedFd>,
}

/// A borrowed handle.
///
/// This has a lifetime parameter to tie it to the lifetime of something that
/// owns the handle.
///
/// This uses `repr(transparent)` and has the representation of a host handle,
/// so it can be used in FFI in places where a handle is passed as an argument,
/// it is not captured or consumed, and it is never null.
///
/// Note that it *may* have the value [`INVALID_HANDLE_VALUE`]. See [here] for
/// the full story.
///
/// This type's `.to_owned()` implementation returns another `BorrowedHandle`
/// rather than an `OwnedHandle`. It just makes a trivial copy of the raw
/// handle, which is then borrowed under the same lifetime.
///
/// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
#[cfg(windows)]
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct BorrowedHandle<'handle> {
    handle: RawHandle,
    _phantom: PhantomData<&'handle OwnedHandle>,
}

/// A borrowed socket.
///
/// This has a lifetime parameter to tie it to the lifetime of something that
/// owns the socket.
///
/// This uses `repr(transparent)` and has the representation of a host socket,
/// so it can be used in FFI in places where a socket is passed as an argument,
/// it is not captured or consumed, and it never has the value
/// [`INVALID_SOCKET`].
///
/// This type's `.to_owned()` implementation returns another `BorrowedSocket`
/// rather than an `OwnedSocket`. It just makes a trivial copy of the raw
/// socket, which is then borrowed under the same lifetime.
#[cfg(windows)]
#[derive(Copy, Clone)]
#[repr(transparent)]
#[cfg_attr(rustc_attrs, rustc_nonnull_optimization_guaranteed)]
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_start(0))]
// This is -2, in two's complement. -1 is `INVALID_SOCKET`.
#[cfg_attr(
    all(rustc_attrs, target_pointer_width = "32"),
    rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FE)
)]
#[cfg_attr(
    all(rustc_attrs, target_pointer_width = "64"),
    rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FF_FF_FF_FF_FE)
)]
pub struct BorrowedSocket<'socket> {
    socket: RawSocket,
    _phantom: PhantomData<&'socket OwnedSocket>,
}

/// An owned file descriptor.
///
/// This closes the file descriptor on drop.
///
/// This uses `repr(transparent)` and has the representation of a host file
/// descriptor, so it can be used in FFI in places where a file descriptor is
/// passed as a consumed argument or returned as an owned value, and it never
/// has the value `-1`.
#[cfg(any(unix, target_os = "wasi"))]
#[repr(transparent)]
#[cfg_attr(rustc_attrs, rustc_nonnull_optimization_guaranteed)]
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_start(0))]
// libstd/os/raw/mod.rs assures me that every libstd-supported platform has a
// 32-bit c_int. Below is -2, in two's complement, but that only works out
// because c_int is 32 bits.
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FE))]
pub struct OwnedFd {
    fd: RawFd,
}

#[cfg(any(unix, target_os = "wasi"))]
impl OwnedFd {
    /// Creates a new `OwnedFd` instance that shares the same underlying file handle
    /// as the existing `OwnedFd` instance.
    pub fn try_clone(&self) -> std::io::Result<Self> {
        #[cfg(feature = "close")]
        {
            #[cfg(unix)]
            {
                // We want to atomically duplicate this file descriptor and set the
                // CLOEXEC flag, and currently that's done via F_DUPFD_CLOEXEC. This
                // is a POSIX flag that was added to Linux in 2.6.24.
                #[cfg(not(target_os = "espidf"))]
                let cmd = libc::F_DUPFD_CLOEXEC;

                // For ESP-IDF, F_DUPFD is used instead, because the CLOEXEC semantics
                // will never be supported, as this is a bare metal framework with
                // no capabilities for multi-process execution.  While F_DUPFD is also
                // not supported yet, it might be (currently it returns ENOSYS).
                #[cfg(target_os = "espidf")]
                let cmd = libc::F_DUPFD;

                let fd = match unsafe { libc::fcntl(self.as_raw_fd(), cmd, 0) } {
                    -1 => return Err(std::io::Error::last_os_error()),
                    fd => fd,
                };

                Ok(unsafe { Self::from_raw_fd(fd) })
            }

            #[cfg(target_os = "wasi")]
            {
                unreachable!("try_clone is not yet suppported on wasi");
            }
        }

        // If the `close` feature is disabled, we expect users to avoid cloning
        // `OwnedFd` instances, so that we don't have to call `fcntl`.
        #[cfg(not(feature = "close"))]
        {
            unreachable!("try_clone called without the \"close\" feature in io-lifetimes");
        }
    }
}

/// An owned handle.
///
/// This closes the handle on drop.
///
/// Note that it *may* have the value `INVALID_HANDLE_VALUE` (-1), which is
/// sometimes a valid handle value. See [here] for the full story.
///
/// And, it *may* have the value `NULL` (0), which can occur when consoles are
/// detached from processes, or when `windows_subsystem` is used.
///
/// `OwnedHandle` uses [`CloseHandle`] to close its handle on drop. As such,
/// it must not be used with handles to open registry keys which need to be
/// closed with [`RegCloseKey`] instead.
///
/// [`CloseHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-closehandle
/// [`RegCloseKey`]: https://docs.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regclosekey
///
/// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
#[cfg(windows)]
#[repr(transparent)]
pub struct OwnedHandle {
    handle: RawHandle,
}

#[cfg(windows)]
impl OwnedHandle {
    /// Creates a new `OwnedHandle` instance that shares the same underlying file handle
    /// as the existing `OwnedHandle` instance.
    pub fn try_clone(&self) -> std::io::Result<OwnedHandle> {
        #[cfg(feature = "close")]
        {
            self.duplicate(0, false, DUPLICATE_SAME_ACCESS)
        }

        // If the `close` feature is disabled, we expect users to avoid cloning
        // `OwnedHandle` instances, so that we don't have to call `fcntl`.
        #[cfg(not(feature = "close"))]
        {
            unreachable!("try_clone called without the \"close\" feature in io-lifetimes");
        }
    }

    #[cfg(feature = "close")]
    pub(crate) fn duplicate(
        &self,
        access: DWORD,
        inherit: bool,
        options: DWORD,
    ) -> std::io::Result<Self> {
        let mut ret = 0 as HANDLE;
        match unsafe {
            let cur_proc = GetCurrentProcess();
            DuplicateHandle(
                cur_proc,
                self.as_raw_handle(),
                cur_proc,
                &mut ret,
                access,
                inherit as BOOL,
                options,
            )
        } {
            0 => return Err(std::io::Error::last_os_error()),
            _ => (),
        }
        unsafe { Ok(Self::from_raw_handle(ret)) }
    }
}

/// An owned socket.
///
/// This closes the socket on drop.
///
/// This uses `repr(transparent)` and has the representation of a host socket,
/// so it can be used in FFI in places where a socket is passed as a consumed
/// argument or returned as an owned value, and it never has the value
/// [`INVALID_SOCKET`].
#[cfg(windows)]
#[repr(transparent)]
#[cfg_attr(rustc_attrs, rustc_nonnull_optimization_guaranteed)]
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_start(0))]
// This is -2, in two's complement. -1 is `INVALID_SOCKET`.
#[cfg_attr(
    all(rustc_attrs, target_pointer_width = "32"),
    rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FE)
)]
#[cfg_attr(
    all(rustc_attrs, target_pointer_width = "64"),
    rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FF_FF_FF_FF_FE)
)]
pub struct OwnedSocket {
    socket: RawSocket,
}

#[cfg(windows)]
impl OwnedSocket {
    /// Creates a new `OwnedSocket` instance that shares the same underlying socket
    /// as the existing `OwnedSocket` instance.
    pub fn try_clone(&self) -> std::io::Result<Self> {
        #[cfg(feature = "close")]
        {
            let mut info = unsafe { std::mem::zeroed::<WSAPROTOCOL_INFOW>() };
            let result = unsafe {
                WSADuplicateSocketW(self.as_raw_socket() as _, GetCurrentProcessId(), &mut info)
            };
            match result {
                SOCKET_ERROR => return Err(std::io::Error::last_os_error()),
                0 => (),
                _ => panic!(),
            }
            let socket = unsafe {
                WSASocketW(
                    info.iAddressFamily,
                    info.iSocketType,
                    info.iProtocol,
                    &mut info,
                    0,
                    WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT,
                )
            };

            if socket != INVALID_SOCKET {
                unsafe { Ok(OwnedSocket::from_raw_socket(socket as _)) }
            } else {
                let error = unsafe { WSAGetLastError() };

                if error != WSAEPROTOTYPE && error != WSAEINVAL {
                    return Err(std::io::Error::from_raw_os_error(error));
                }

                let socket = unsafe {
                    WSASocketW(
                        info.iAddressFamily,
                        info.iSocketType,
                        info.iProtocol,
                        &mut info,
                        0,
                        WSA_FLAG_OVERLAPPED,
                    )
                };

                if socket == INVALID_SOCKET {
                    return Err(std::io::Error::last_os_error());
                }

                unsafe {
                    let socket = OwnedSocket::from_raw_socket(socket as _);
                    socket.set_no_inherit()?;
                    Ok(socket)
                }
            }
        }

        // If the `close` feature is disabled, we expect users to avoid cloning
        // `OwnedSocket` instances, so that we don't have to call `fcntl`.
        #[cfg(not(feature = "close"))]
        {
            unreachable!("try_clone called without the \"close\" feature in io-lifetimes");
        }
    }

    #[cfg(feature = "close")]
    #[cfg(not(target_vendor = "uwp"))]
    fn set_no_inherit(&self) -> std::io::Result<()> {
        match unsafe {
            SetHandleInformation(self.as_raw_socket() as HANDLE, HANDLE_FLAG_INHERIT, 0)
        } {
            0 => return Err(std::io::Error::last_os_error()),
            _ => Ok(()),
        }
    }

    #[cfg(feature = "close")]
    #[cfg(target_vendor = "uwp")]
    fn set_no_inherit(&self) -> std::io::Result<()> {
        Err(io::Error::new_const(
            std::io::ErrorKind::Unsupported,
            &"Unavailable on UWP",
        ))
    }
}

/// FFI type for handles in return values or out parameters, where `INVALID_HANDLE_VALUE` is used
/// as a sentry value to indicate errors, such as in the return value of `CreateFileW`. This uses
/// `repr(transparent)` and has the representation of a host handle, so that it can be used in such
/// FFI declarations.
///
/// The only thing you can usefully do with a `HandleOrInvalid` is to convert it into an
/// `OwnedHandle` using its [`TryFrom`] implementation; this conversion takes care of the check for
/// `INVALID_HANDLE_VALUE`. This ensures that such FFI calls cannot start using the handle without
/// checking for `INVALID_HANDLE_VALUE` first.
///
/// This type concerns any value other than `INVALID_HANDLE_VALUE` to be valid, including `NULL`.
/// This is because APIs that use `INVALID_HANDLE_VALUE` as their sentry value may return `NULL`
/// under `windows_subsystem = "windows"` or other situations where I/O devices are detached.
///
/// If this holds a valid handle, it will close the handle on drop.
#[cfg(windows)]
#[repr(transparent)]
#[derive(Debug)]
pub struct HandleOrInvalid(RawHandle);

/// FFI type for handles in return values or out parameters, where `NULL` is used
/// as a sentry value to indicate errors, such as in the return value of `CreateThread`. This uses
/// `repr(transparent)` and has the representation of a host handle, so that it can be used in such
/// FFI declarations.
///
/// The only thing you can usefully do with a `HandleOrNull` is to convert it into an
/// `OwnedHandle` using its [`TryFrom`] implementation; this conversion takes care of the check for
/// `NULL`. This ensures that such FFI calls cannot start using the handle without
/// checking for `NULL` first.
///
/// This type concerns any value other than `NULL` to be valid, including `INVALID_HANDLE_VALUE`.
/// This is because APIs that use `NULL` as their sentry value don't treat `INVALID_HANDLE_VALUE`
/// as special.
///
/// If this holds a valid handle, it will close the handle on drop.
#[cfg(windows)]
#[repr(transparent)]
#[derive(Debug)]
pub struct HandleOrNull(RawHandle);

// The Windows [`HANDLE`] type may be transferred across and shared between
// thread boundaries (despite containing a `*mut void`, which in general isn't
// `Send` or `Sync`).
//
// [`HANDLE`]: std::os::windows::raw::HANDLE
#[cfg(windows)]
unsafe impl Send for OwnedHandle {}
#[cfg(windows)]
unsafe impl Send for HandleOrInvalid {}
#[cfg(windows)]
unsafe impl Send for HandleOrNull {}
#[cfg(windows)]
unsafe impl Send for BorrowedHandle<'_> {}
#[cfg(windows)]
unsafe impl Sync for OwnedHandle {}
#[cfg(windows)]
unsafe impl Sync for HandleOrInvalid {}
#[cfg(windows)]
unsafe impl Sync for HandleOrNull {}
#[cfg(windows)]
unsafe impl Sync for BorrowedHandle<'_> {}

#[cfg(any(unix, target_os = "wasi"))]
impl BorrowedFd<'_> {
    /// Return a `BorrowedFd` holding the given raw file descriptor.
    ///
    /// # Safety
    ///
    /// The resource pointed to by `raw` must remain open for the duration of
    /// the returned `BorrowedFd`, and it must not have the value `-1`.
    #[inline]
    pub unsafe fn borrow_raw(fd: RawFd) -> Self {
        debug_assert_ne!(fd, -1_i32 as RawFd);
        Self {
            fd,
            _phantom: PhantomData,
        }
    }
}

#[cfg(windows)]
impl BorrowedHandle<'_> {
    /// Return a `BorrowedHandle` holding the given raw handle.
    ///
    /// # Safety
    ///
    /// The resource pointed to by `raw` must remain open for the duration of
    /// the returned `BorrowedHandle`, and it must not be null.
    #[inline]
    pub unsafe fn borrow_raw(handle: RawHandle) -> Self {
        debug_assert!(!handle.is_null());
        Self {
            handle,
            _phantom: PhantomData,
        }
    }
}

#[cfg(windows)]
impl BorrowedSocket<'_> {
    /// Return a `BorrowedSocket` holding the given raw socket.
    ///
    /// # Safety
    ///
    /// The resource pointed to by `raw` must remain open for the duration of
    /// the returned `BorrowedSocket`, and it must not have the value
    /// [`INVALID_SOCKET`].
    #[inline]
    pub unsafe fn borrow_raw(socket: RawSocket) -> Self {
        debug_assert_ne!(socket, INVALID_SOCKET as RawSocket);
        Self {
            socket,
            _phantom: PhantomData,
        }
    }
}

#[cfg(windows)]
impl TryFrom<HandleOrInvalid> for OwnedHandle {
    type Error = ();

    #[inline]
    fn try_from(handle_or_invalid: HandleOrInvalid) -> Result<Self, ()> {
        let raw = handle_or_invalid.0;
        if raw == INVALID_HANDLE_VALUE {
            // Don't call `CloseHandle`; it'd be harmless, except that it could
            // overwrite the `GetLastError` error.
            forget(handle_or_invalid);

            Err(())
        } else {
            Ok(OwnedHandle { handle: raw })
        }
    }
}

#[cfg(windows)]
impl TryFrom<HandleOrNull> for OwnedHandle {
    type Error = ();

    #[inline]
    fn try_from(handle_or_null: HandleOrNull) -> Result<Self, ()> {
        let raw = handle_or_null.0;
        if raw.is_null() {
            // Don't call `CloseHandle`; it'd be harmless, except that it could
            // overwrite the `GetLastError` error.
            forget(handle_or_null);

            Err(())
        } else {
            Ok(OwnedHandle { handle: raw })
        }
    }
}

#[cfg(any(unix, target_os = "wasi"))]
impl AsRawFd for BorrowedFd<'_> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

#[cfg(windows)]
impl AsRawHandle for BorrowedHandle<'_> {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.handle
    }
}

#[cfg(windows)]
impl AsRawSocket for BorrowedSocket<'_> {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.socket
    }
}

#[cfg(any(unix, target_os = "wasi"))]
impl AsRawFd for OwnedFd {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

#[cfg(windows)]
impl AsRawHandle for OwnedHandle {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.handle
    }
}

#[cfg(windows)]
impl AsRawSocket for OwnedSocket {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.socket
    }
}

#[cfg(any(unix, target_os = "wasi"))]
impl IntoRawFd for OwnedFd {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        let fd = self.fd;
        forget(self);
        fd
    }
}

#[cfg(windows)]
impl IntoRawHandle for OwnedHandle {
    #[inline]
    fn into_raw_handle(self) -> RawHandle {
        let handle = self.handle;
        forget(self);
        handle
    }
}

#[cfg(windows)]
impl IntoRawSocket for OwnedSocket {
    #[inline]
    fn into_raw_socket(self) -> RawSocket {
        let socket = self.socket;
        forget(self);
        socket
    }
}

#[cfg(any(unix, target_os = "wasi"))]
impl FromRawFd for OwnedFd {
    /// Constructs a new instance of `Self` from the given raw file descriptor.
    ///
    /// # Safety
    ///
    /// The resource pointed to by `raw` must be open and suitable for assuming
    /// ownership.
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        debug_assert_ne!(fd, -1_i32 as RawFd);
        Self { fd }
    }
}

#[cfg(windows)]
impl FromRawHandle for OwnedHandle {
    #[inline]
    unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        Self { handle }
    }
}

#[cfg(windows)]
impl FromRawSocket for OwnedSocket {
    #[inline]
    unsafe fn from_raw_socket(socket: RawSocket) -> Self {
        debug_assert_ne!(socket, INVALID_SOCKET as RawSocket);
        Self { socket }
    }
}

#[cfg(windows)]
impl HandleOrInvalid {
    /// Constructs a new instance of `Self` from the given `RawHandle` returned
    /// from a Windows API that uses `INVALID_HANDLE_VALUE` to indicate
    /// failure, such as `CreateFileW`.
    ///
    /// Use `HandleOrNull` instead of `HandleOrInvalid` for APIs that
    /// use null to indicate failure.
    ///
    /// # Safety
    ///
    /// The passed `handle` value must either satisfy the safety requirements
    /// of [`FromRawHandle::from_raw_handle`], or be
    /// `INVALID_HANDLE_VALUE` (-1). Note that not all Windows APIs use
    /// `INVALID_HANDLE_VALUE` for errors; see [here] for the full story.
    ///
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    #[inline]
    pub unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        Self(handle)
    }
}

#[cfg(windows)]
impl HandleOrNull {
    /// Constructs a new instance of `Self` from the given `RawHandle` returned
    /// from a Windows API that uses null to indicate failure, such as
    /// `CreateThread`.
    ///
    /// Use `HandleOrInvalid` instead of `HandleOrNull` for APIs that
    /// use `INVALID_HANDLE_VALUE` to indicate failure.
    ///
    /// # Safety
    ///
    /// The passed `handle` value must either satisfy the safety requirements
    /// of [`FromRawHandle::from_raw_handle`], or be null. Note that not all
    /// Windows APIs use null for errors; see [here] for the full story.
    ///
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    #[inline]
    pub unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        Self(handle)
    }
}

#[cfg(any(unix, target_os = "wasi"))]
impl Drop for OwnedFd {
    #[inline]
    fn drop(&mut self) {
        #[cfg(feature = "close")]
        unsafe {
            let _ = libc::close(self.fd as std::os::raw::c_int);
        }

        // If the `close` feature is disabled, we expect users to avoid letting
        // `OwnedFd` instances drop, so that we don't have to call `close`.
        #[cfg(not(feature = "close"))]
        {
            unreachable!("drop called without the \"close\" feature in io-lifetimes");
        }
    }
}

#[cfg(windows)]
impl Drop for OwnedHandle {
    #[inline]
    fn drop(&mut self) {
        #[cfg(feature = "close")]
        unsafe {
            let _ = winapi::um::handleapi::CloseHandle(self.handle);
        }

        // If the `close` feature is disabled, we expect users to avoid letting
        // `OwnedHandle` instances drop, so that we don't have to call `close`.
        #[cfg(not(feature = "close"))]
        {
            unreachable!("drop called without the \"close\" feature in io-lifetimes");
        }
    }
}

#[cfg(windows)]
impl Drop for HandleOrInvalid {
    #[inline]
    fn drop(&mut self) {
        #[cfg(feature = "close")]
        unsafe {
            let _ = winapi::um::handleapi::CloseHandle(self.0);
        }

        // If the `close` feature is disabled, we expect users to avoid letting
        // `HandleOrInvalid` instances drop, so that we don't have to call `close`.
        #[cfg(not(feature = "close"))]
        {
            unreachable!("drop called without the \"close\" feature in io-lifetimes");
        }
    }
}

#[cfg(windows)]
impl Drop for HandleOrNull {
    #[inline]
    fn drop(&mut self) {
        #[cfg(feature = "close")]
        unsafe {
            let _ = winapi::um::handleapi::CloseHandle(self.0);
        }

        // If the `close` feature is disabled, we expect users to avoid letting
        // `HandleOrNull` instances drop, so that we don't have to call `close`.
        #[cfg(not(feature = "close"))]
        {
            unreachable!("drop called without the \"close\" feature in io-lifetimes");
        }
    }
}

#[cfg(windows)]
impl Drop for OwnedSocket {
    #[inline]
    fn drop(&mut self) {
        #[cfg(feature = "close")]
        unsafe {
            let _ = winapi::um::winsock2::closesocket(self.socket as winapi::um::winsock2::SOCKET);
        }

        // If the `close` feature is disabled, we expect users to avoid letting
        // `OwnedSocket` instances drop, so that we don't have to call `close`.
        #[cfg(not(feature = "close"))]
        {
            unreachable!("drop called without the \"close\" feature in io-lifetimes");
        }
    }
}

#[cfg(any(unix, target_os = "wasi"))]
impl fmt::Debug for BorrowedFd<'_> {
    #[allow(clippy::missing_inline_in_public_items)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedFd").field("fd", &self.fd).finish()
    }
}

#[cfg(windows)]
impl fmt::Debug for BorrowedHandle<'_> {
    #[allow(clippy::missing_inline_in_public_items)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedHandle")
            .field("handle", &self.handle)
            .finish()
    }
}

#[cfg(windows)]
impl fmt::Debug for BorrowedSocket<'_> {
    #[allow(clippy::missing_inline_in_public_items)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedSocket")
            .field("socket", &self.socket)
            .finish()
    }
}

#[cfg(any(unix, target_os = "wasi"))]
impl fmt::Debug for OwnedFd {
    #[allow(clippy::missing_inline_in_public_items)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedFd").field("fd", &self.fd).finish()
    }
}

#[cfg(windows)]
impl fmt::Debug for OwnedHandle {
    #[allow(clippy::missing_inline_in_public_items)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedHandle")
            .field("handle", &self.handle)
            .finish()
    }
}

#[cfg(windows)]
impl fmt::Debug for OwnedSocket {
    #[allow(clippy::missing_inline_in_public_items)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedSocket")
            .field("socket", &self.socket)
            .finish()
    }
}
