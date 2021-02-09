//! Named pipes

use std::ffi::OsStr;
use std::fs::{OpenOptions, File};
use std::io::prelude::*;
use std::io;
use std::os::windows::ffi::*;
use std::os::windows::io::*;
use std::time::Duration;

use winapi::*;
use kernel32::*;
use handle::Handle;

/// Readable half of an anonymous pipe.
#[derive(Debug)]
pub struct AnonRead(Handle);

/// Writable half of an anonymous pipe.
#[derive(Debug)]
pub struct AnonWrite(Handle);

/// A named pipe that can accept connections.
#[derive(Debug)]
pub struct NamedPipe(Handle);

/// A builder structure for creating a new named pipe.
#[derive(Debug)]
pub struct NamedPipeBuilder {
    name: Vec<u16>,
    dwOpenMode: DWORD,
    dwPipeMode: DWORD,
    nMaxInstances: DWORD,
    nOutBufferSize: DWORD,
    nInBufferSize: DWORD,
    nDefaultTimeOut: DWORD,
}

/// Creates a new anonymous in-memory pipe, returning the read/write ends of the
/// pipe.
///
/// The buffer size for this pipe may also be specified, but the system will
/// normally use this as a suggestion and it's not guaranteed that the buffer
/// will be precisely this size.
pub fn anonymous(buffer_size: u32) -> io::Result<(AnonRead, AnonWrite)> {
    let mut read = 0 as HANDLE;
    let mut write = 0 as HANDLE;
    try!(::cvt(unsafe {
        CreatePipe(&mut read, &mut write, 0 as *mut _, buffer_size)
    }));
    Ok((AnonRead(Handle::new(read)), AnonWrite(Handle::new(write))))
}

impl Read for AnonRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { self.0.read(buf) }
}
impl<'a> Read for &'a AnonRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { self.0.read(buf) }
}

impl AsRawHandle for AnonRead {
    fn as_raw_handle(&self) -> HANDLE { self.0.raw() }
}
impl FromRawHandle for AnonRead {
    unsafe fn from_raw_handle(handle: HANDLE) -> AnonRead {
        AnonRead(Handle::new(handle))
    }
}
impl IntoRawHandle for AnonRead {
    fn into_raw_handle(self) -> HANDLE { self.0.into_raw() }
}

impl Write for AnonWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> { self.0.write(buf) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
impl<'a> Write for &'a AnonWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> { self.0.write(buf) }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl AsRawHandle for AnonWrite {
    fn as_raw_handle(&self) -> HANDLE { self.0.raw() }
}
impl FromRawHandle for AnonWrite {
    unsafe fn from_raw_handle(handle: HANDLE) -> AnonWrite {
        AnonWrite(Handle::new(handle))
    }
}
impl IntoRawHandle for AnonWrite {
    fn into_raw_handle(self) -> HANDLE { self.0.into_raw() }
}

/// A convenience function to connect to a named pipe.
///
/// This function will block the calling process until it can connect to the
/// pipe server specified by `addr`. This will use `NamedPipe::wait` internally
/// to block until it can connect.
pub fn connect<A: AsRef<OsStr>>(addr: A) -> io::Result<File> {
    _connect(addr.as_ref())
}

fn _connect(addr: &OsStr) -> io::Result<File> {
    let mut r = OpenOptions::new();
    let mut w = OpenOptions::new();
    let mut rw = OpenOptions::new();
    r.read(true);
    w.write(true);
    rw.read(true).write(true);
    loop {
        let res = rw.open(addr).or_else(|_| r.open(addr))
                               .or_else(|_| w.open(addr));
        match res {
            Ok(f) => return Ok(f),
            Err(ref e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32)
                => {}
            Err(e) => return Err(e),
        }

        try!(NamedPipe::wait(addr, Some(Duration::new(20, 0))));
    }
}

impl NamedPipe {
    /// Creates a new initial named pipe.
    ///
    /// This function is equivalent to:
    ///
    /// ```
    /// use miow::pipe::NamedPipeBuilder;
    ///
    /// # let addr = "foo";
    /// NamedPipeBuilder::new(addr)
    ///                  .first(true)
    ///                  .inbound(true)
    ///                  .outbound(true)
    ///                  .out_buffer_size(65536)
    ///                  .in_buffer_size(65536)
    ///                  .create();
    /// ```
    pub fn new<A: AsRef<OsStr>>(addr: A) -> io::Result<NamedPipe> {
        NamedPipeBuilder::new(addr).create()
    }

    /// Waits until either a time-out interval elapses or an instance of the
    /// specified named pipe is available for connection.
    ///
    /// If this function succeeds the process can create a `File` to connect to
    /// the named pipe.
    pub fn wait<A: AsRef<OsStr>>(addr: A, timeout: Option<Duration>)
                                 -> io::Result<()> {
        NamedPipe::_wait(addr.as_ref(), timeout)
    }

    fn _wait(addr: &OsStr, timeout: Option<Duration>) -> io::Result<()> {
        let addr = addr.encode_wide().chain(Some(0)).collect::<Vec<_>>();
        let timeout = ::dur2ms(timeout);
        ::cvt(unsafe {
            WaitNamedPipeW(addr.as_ptr(), timeout)
        }).map(|_| ())
    }

    /// Connects this named pipe to a client, blocking until one becomes
    /// available.
    ///
    /// This function will call the `ConnectNamedPipe` function to await for a
    /// client to connect. This can be called immediately after the pipe is
    /// created, or after it has been disconnected from a previous client.
    pub fn connect(&self) -> io::Result<()> {
        match ::cvt(unsafe { ConnectNamedPipe(self.0.raw(), 0 as *mut _) }) {
            Ok(_) => Ok(()),
            Err(ref e) if e.raw_os_error() == Some(ERROR_PIPE_CONNECTED as i32)
                => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Issue a connection request with the specified overlapped operation.
    ///
    /// This function will issue a request to connect a client to this server,
    /// returning immediately after starting the overlapped operation.
    ///
    /// If this function immediately succeeds then `Ok(true)` is returned. If
    /// the overlapped operation is enqueued and pending, then `Ok(false)` is
    /// returned. Otherwise an error is returned indicating what went wrong.
    ///
    /// # Unsafety
    ///
    /// This function is unsafe because the kernel requires that the
    /// `overlapped` pointer is valid until the end of the I/O operation. The
    /// kernel also requires that `overlapped` is unique for this I/O operation
    /// and is not in use for any other I/O.
    ///
    /// To safely use this function callers must ensure that this pointer is
    /// valid until the I/O operation is completed, typically via completion
    /// ports and waiting to receive the completion notification on the port.
    pub unsafe fn connect_overlapped(&self, overlapped: *mut OVERLAPPED)
                                     -> io::Result<bool> {
        match ::cvt(ConnectNamedPipe(self.0.raw(), overlapped)) {
            Ok(_) => Ok(true),
            Err(ref e) if e.raw_os_error() == Some(ERROR_PIPE_CONNECTED as i32)
                => Ok(true),
            Err(ref e) if e.raw_os_error() == Some(ERROR_IO_PENDING as i32)
                => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Disconnects this named pipe from any connected client.
    pub fn disconnect(&self) -> io::Result<()> {
        ::cvt(unsafe {
            DisconnectNamedPipe(self.0.raw())
        }).map(|_| ())
    }

    /// Issues an overlapped read operation to occur on this pipe.
    ///
    /// This function will issue an asynchronous read to occur in an overlapped
    /// fashion, returning immediately. The `buf` provided will be filled in
    /// with data and the request is tracked by the `overlapped` function
    /// provided.
    ///
    /// If the operation succeeds immediately, `Ok(Some(n))` is returned where
    /// `n` is the number of bytes read. If an asynchronous operation is
    /// enqueued, then `Ok(None)` is returned. Otherwise if an error occurred
    /// it is returned.
    ///
    /// When this operation completes (or if it completes immediately), another
    /// mechanism must be used to learn how many bytes were transferred (such as
    /// looking at the filed in the IOCP status message).
    ///
    /// # Unsafety
    ///
    /// This function is unsafe because the kernel requires that the `buf` and
    /// `overlapped` pointers to be valid until the end of the I/O operation.
    /// The kernel also requires that `overlapped` is unique for this I/O
    /// operation and is not in use for any other I/O.
    ///
    /// To safely use this function callers must ensure that the pointers are
    /// valid until the I/O operation is completed, typically via completion
    /// ports and waiting to receive the completion notification on the port.
    pub unsafe fn read_overlapped(&self,
                                  buf: &mut [u8],
                                  overlapped: *mut OVERLAPPED)
                                  -> io::Result<Option<usize>> {
        self.0.read_overlapped(buf, overlapped)
    }

    /// Issues an overlapped write operation to occur on this pipe.
    ///
    /// This function will issue an asynchronous write to occur in an overlapped
    /// fashion, returning immediately. The `buf` provided will be filled in
    /// with data and the request is tracked by the `overlapped` function
    /// provided.
    ///
    /// If the operation succeeds immediately, `Ok(Some(n))` is returned where
    /// `n` is the number of bytes written. If an asynchronous operation is
    /// enqueued, then `Ok(None)` is returned. Otherwise if an error occurred
    /// it is returned.
    ///
    /// When this operation completes (or if it completes immediately), another
    /// mechanism must be used to learn how many bytes were transferred (such as
    /// looking at the filed in the IOCP status message).
    ///
    /// # Unsafety
    ///
    /// This function is unsafe because the kernel requires that the `buf` and
    /// `overlapped` pointers to be valid until the end of the I/O operation.
    /// The kernel also requires that `overlapped` is unique for this I/O
    /// operation and is not in use for any other I/O.
    ///
    /// To safely use this function callers must ensure that the pointers are
    /// valid until the I/O operation is completed, typically via completion
    /// ports and waiting to receive the completion notification on the port.
    pub unsafe fn write_overlapped(&self,
                                   buf: &[u8],
                                   overlapped: *mut OVERLAPPED)
                                   -> io::Result<Option<usize>> {
        self.0.write_overlapped(buf, overlapped)
    }

    /// Calls the `GetOverlappedResult` function to get the result of an
    /// overlapped operation for this handle.
    ///
    /// This function takes the `OVERLAPPED` argument which must have been used
    /// to initiate an overlapped I/O operation, and returns either the
    /// successful number of bytes transferred during the operation or an error
    /// if one occurred.
    ///
    /// # Unsafety
    ///
    /// This function is unsafe as `overlapped` must have previously been used
    /// to execute an operation for this handle, and it must also be a valid
    /// pointer to an `Overlapped` instance.
    ///
    /// # Panics
    ///
    /// This function will panic
    pub unsafe fn result(&self, overlapped: *mut OVERLAPPED)
                         -> io::Result<usize> {
        let mut transferred = 0;
        let r = GetOverlappedResult(self.0.raw(),
                                    overlapped,
                                    &mut transferred,
                                    FALSE);
        if r == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(transferred as usize)
        }
    }
}

impl Read for NamedPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { self.0.read(buf) }
}
impl<'a> Read for &'a NamedPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> { self.0.read(buf) }
}

impl Write for NamedPipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> { self.0.write(buf) }
    fn flush(&mut self) -> io::Result<()> {
        <&NamedPipe as Write>::flush(&mut &*self)
    }
}
impl<'a> Write for &'a NamedPipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> { self.0.write(buf) }
    fn flush(&mut self) -> io::Result<()> {
        ::cvt(unsafe { FlushFileBuffers(self.0.raw()) }).map(|_| ())
    }
}

impl AsRawHandle for NamedPipe {
    fn as_raw_handle(&self) -> HANDLE { self.0.raw() }
}
impl FromRawHandle for NamedPipe {
    unsafe fn from_raw_handle(handle: HANDLE) -> NamedPipe {
        NamedPipe(Handle::new(handle))
    }
}
impl IntoRawHandle for NamedPipe {
    fn into_raw_handle(self) -> HANDLE { self.0.into_raw() }
}

fn flag(slot: &mut DWORD, on: bool, val: DWORD) {
    if on {
        *slot |= val;
    } else {
        *slot &= !val;
    }
}

impl NamedPipeBuilder {
    /// Creates a new named pipe builder with the default settings.
    pub fn new<A: AsRef<OsStr>>(addr: A) -> NamedPipeBuilder {
        NamedPipeBuilder {
            name: addr.as_ref().encode_wide().chain(Some(0)).collect(),
            dwOpenMode: PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE |
                        FILE_FLAG_OVERLAPPED,
            dwPipeMode: PIPE_TYPE_BYTE,
            nMaxInstances: PIPE_UNLIMITED_INSTANCES,
            nOutBufferSize: 65536,
            nInBufferSize: 65536,
            nDefaultTimeOut: 0,
        }
    }

    /// Indicates whether data is allowed to flow from the client to the server.
    pub fn inbound(&mut self, allowed: bool) -> &mut Self {
        flag(&mut self.dwOpenMode, allowed, PIPE_ACCESS_INBOUND);
        self
    }

    /// Indicates whether data is allowed to flow from the server to the client.
    pub fn outbound(&mut self, allowed: bool) -> &mut Self {
        flag(&mut self.dwOpenMode, allowed, PIPE_ACCESS_OUTBOUND);
        self
    }

    /// Indicates that this pipe must be the first instance.
    ///
    /// If set to true, then creation will fail if there's already an instance
    /// elsewhere.
    pub fn first(&mut self, first: bool) -> &mut Self {
        flag(&mut self.dwOpenMode, first, FILE_FLAG_FIRST_PIPE_INSTANCE);
        self
    }

    /// Indicates whether this server can accept remote clients or not.
    pub fn accept_remote(&mut self, accept: bool) -> &mut Self {
        flag(&mut self.dwPipeMode, !accept, PIPE_REJECT_REMOTE_CLIENTS);
        self
    }

    /// Specifies the maximum number of instances of the server pipe that are
    /// allowed.
    ///
    /// The first instance of a pipe can specify this value. A value of 255
    /// indicates that there is no limit to the number of instances.
    pub fn max_instances(&mut self, instances: u8) -> &mut Self {
        self.nMaxInstances = instances as DWORD;
        self
    }

    /// Specifies the number of bytes to reserver for the output buffer
    pub fn out_buffer_size(&mut self, buffer: u32) -> &mut Self {
        self.nOutBufferSize = buffer as DWORD;
        self
    }

    /// Specifies the number of bytes to reserver for the input buffer
    pub fn in_buffer_size(&mut self, buffer: u32) -> &mut Self {
        self.nInBufferSize = buffer as DWORD;
        self
    }

    /// Using the options in this builder, attempt to create a new named pipe.
    ///
    /// This function will call the `CreateNamedPipe` function and return the
    /// result.
    pub fn create(&mut self) -> io::Result<NamedPipe> {
        let h = unsafe {
            CreateNamedPipeW(self.name.as_ptr(),
                             self.dwOpenMode, self.dwPipeMode,
                             self.nMaxInstances, self.nOutBufferSize,
                             self.nInBufferSize, self.nDefaultTimeOut,
                             0 as *mut _)
        };
        if h == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(NamedPipe(Handle::new(h)))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{File, OpenOptions};
    use std::io::prelude::*;
    use std::sync::mpsc::channel;
    use std::thread;
    use std::time::Duration;

    use rand::{thread_rng, Rng};

    use super::{anonymous, NamedPipe, NamedPipeBuilder};
    use iocp::CompletionPort;
    use Overlapped;

    fn name() -> String {
        let name = thread_rng().gen_ascii_chars().take(30).collect::<String>();
        format!(r"\\.\pipe\{}", name)
    }

    #[test]
    fn anon() {
        let (mut read, mut write) = t!(anonymous(256));
        assert_eq!(t!(write.write(&[1, 2, 3])), 3);
        let mut b = [0; 10];
        assert_eq!(t!(read.read(&mut b)), 3);
        assert_eq!(&b[..3], &[1, 2, 3]);
    }

    #[test]
    fn named_not_first() {
        let name = name();
        let _a = t!(NamedPipe::new(&name));
        assert!(NamedPipe::new(&name).is_err());

        t!(NamedPipeBuilder::new(&name).first(false).create());
    }

    #[test]
    fn named_connect() {
        let name = name();
        let a = t!(NamedPipe::new(&name));

        let t = thread::spawn(move || {
            t!(File::open(name));
        });

        t!(a.connect());
        t!(a.disconnect());
        t!(t.join());
    }

    #[test]
    fn named_wait() {
        let name = name();
        let a = t!(NamedPipe::new(&name));

        let (tx, rx) = channel();
        let t = thread::spawn(move || {
            t!(NamedPipe::wait(&name, None));
            t!(File::open(&name));
            assert!(NamedPipe::wait(&name, Some(Duration::from_millis(1))).is_err());
            t!(tx.send(()));
        });

        t!(a.connect());
        t!(rx.recv());
        t!(a.disconnect());
        t!(t.join());
    }

    #[test]
    fn named_connect_overlapped() {
        let name = name();
        let a = t!(NamedPipe::new(&name));

        let t = thread::spawn(move || {
            t!(File::open(name));
        });

        let cp = t!(CompletionPort::new(1));
        t!(cp.add_handle(2, &a));

        let over = Overlapped::zero();
        unsafe {
            t!(a.connect_overlapped(over.raw()));
        }

        let status = t!(cp.get(None));
        assert_eq!(status.bytes_transferred(), 0);
        assert_eq!(status.token(), 2);
        assert_eq!(status.overlapped(), over.raw());
        t!(t.join());
    }

    #[test]
    fn named_read_write() {
        let name = name();
        let mut a = t!(NamedPipe::new(&name));

        let t = thread::spawn(move || {
            let mut f = t!(OpenOptions::new().read(true).write(true).open(name));
            t!(f.write_all(&[1, 2, 3]));
            let mut b = [0; 10];
            assert_eq!(t!(f.read(&mut b)), 3);
            assert_eq!(&b[..3], &[1, 2, 3]);
        });

        t!(a.connect());
        let mut b = [0; 10];
        assert_eq!(t!(a.read(&mut b)), 3);
        assert_eq!(&b[..3], &[1, 2, 3]);
        t!(a.write_all(&[1, 2, 3]));
        t!(a.flush());
        t!(a.disconnect());
        t!(t.join());
    }

    #[test]
    fn named_read_overlapped() {
        let name = name();
        let a = t!(NamedPipe::new(&name));

        let t = thread::spawn(move || {
            let mut f = t!(File::create(name));
            t!(f.write_all(&[1, 2, 3]));
        });

        let cp = t!(CompletionPort::new(1));
        t!(cp.add_handle(3, &a));
        t!(a.connect());

        let mut b = [0; 10];
        let over = Overlapped::zero();
        unsafe {
            t!(a.read_overlapped(&mut b, over.raw()));
        }
        let status = t!(cp.get(None));
        assert_eq!(status.bytes_transferred(), 3);
        assert_eq!(status.token(), 3);
        assert_eq!(status.overlapped(), over.raw());
        assert_eq!(&b[..3], &[1, 2, 3]);

        t!(t.join());
    }

    #[test]
    fn named_write_overlapped() {
        let name = name();
        let a = t!(NamedPipe::new(&name));

        let t = thread::spawn(move || {
            let mut f = t!(super::connect(name));
            let mut b = [0; 10];
            assert_eq!(t!(f.read(&mut b)), 3);
            assert_eq!(&b[..3], &[1, 2, 3])
        });

        let cp = t!(CompletionPort::new(1));
        t!(cp.add_handle(3, &a));
        t!(a.connect());

        let over = Overlapped::zero();
        unsafe {
            t!(a.write_overlapped(&[1, 2, 3], over.raw()));
        }

        let status = t!(cp.get(None));
        assert_eq!(status.bytes_transferred(), 3);
        assert_eq!(status.token(), 3);
        assert_eq!(status.overlapped(), over.raw());

        t!(t.join());
    }
}
