#[cfg(not(target_os = "dragonfly"))]
use libc;
use libc::{c_int, c_void};
use std::{fmt, io, error};
use {Error, Result};

pub use self::consts::*;

cfg_if! {
    if #[cfg(any(target_os = "freebsd",
                 target_os = "ios",
                 target_os = "macos"))] {
        unsafe fn errno_location() -> *mut c_int {
            libc::__error()
        }
    } else if #[cfg(target_os = "dragonfly")] {
        // DragonFly uses a thread-local errno variable, but #[thread_local] is
        // feature-gated and not available in stable Rust as of this writing
        // (Rust 1.21.0). We have to use a C extension to access it
        // (src/errno_dragonfly.c).
        //
        // Tracking issue for `thread_local` stabilization:
        //
        //     https://github.com/rust-lang/rust/issues/29594
        //
        // Once this becomes stable, we can remove build.rs,
        // src/errno_dragonfly.c, and use:
        //
        //     extern { #[thread_local] static errno: c_int; }
        //
        #[link(name="errno_dragonfly", kind="static")]
        extern {
            pub fn errno_location() -> *mut c_int;
        }
    } else if #[cfg(any(target_os = "android",
                        target_os = "netbsd",
                        target_os = "openbsd"))] {
        unsafe fn errno_location() -> *mut c_int {
            libc::__errno()
        }
    } else if #[cfg(target_os = "linux")] {
        unsafe fn errno_location() -> *mut c_int {
            libc::__errno_location()
        }
    }
}

/// Sets the platform-specific errno to no-error
unsafe fn clear() {
    *errno_location() = 0;
}

/// Returns the platform-specific value of errno
pub fn errno() -> i32 {
    unsafe {
        (*errno_location()) as i32
    }
}

impl Errno {
    pub fn last() -> Self {
        last()
    }

    pub fn desc(self) -> &'static str {
        desc(self)
    }

    pub fn from_i32(err: i32) -> Errno {
        from_i32(err)
    }

    pub unsafe fn clear() {
        clear()
    }

    /// Returns `Ok(value)` if it does not contain the sentinel value. This
    /// should not be used when `-1` is not the errno sentinel value.
    pub fn result<S: ErrnoSentinel + PartialEq<S>>(value: S) -> Result<S> {
        if value == S::sentinel() {
            Err(Error::Sys(Self::last()))
        } else {
            Ok(value)
        }
    }
}

/// The sentinel value indicates that a function failed and more detailed
/// information about the error can be found in `errno`
pub trait ErrnoSentinel: Sized {
    fn sentinel() -> Self;
}

impl ErrnoSentinel for isize {
    fn sentinel() -> Self { -1 }
}

impl ErrnoSentinel for i32 {
    fn sentinel() -> Self { -1 }
}

impl ErrnoSentinel for i64 {
    fn sentinel() -> Self { -1 }
}

impl ErrnoSentinel for *mut c_void {
    fn sentinel() -> Self { (-1 as isize) as *mut c_void }
}

impl ErrnoSentinel for libc::sighandler_t {
    fn sentinel() -> Self { libc::SIG_ERR }
}

impl error::Error for Errno {}

impl fmt::Display for Errno {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}: {}", self, self.desc())
    }
}

impl From<Errno> for io::Error {
    fn from(err: Errno) -> Self {
        io::Error::from_raw_os_error(err as i32)
    }
}

fn last() -> Errno {
    Errno::from_i32(errno())
}

fn desc(errno: Errno) -> &'static str {
    use self::Errno::*;
    match errno {
        UnknownErrno    => "Unknown errno",
        EPERM           => "Operation not permitted",
        ENOENT          => "No such file or directory",
        ESRCH           => "No such process",
        EINTR           => "Interrupted system call",
        EIO             => "I/O error",
        ENXIO           => "No such device or address",
        E2BIG           => "Argument list too long",
        ENOEXEC         => "Exec format error",
        EBADF           => "Bad file number",
        ECHILD          => "No child processes",
        EAGAIN          => "Try again",
        ENOMEM          => "Out of memory",
        EACCES          => "Permission denied",
        EFAULT          => "Bad address",
        ENOTBLK         => "Block device required",
        EBUSY           => "Device or resource busy",
        EEXIST          => "File exists",
        EXDEV           => "Cross-device link",
        ENODEV          => "No such device",
        ENOTDIR         => "Not a directory",
        EISDIR          => "Is a directory",
        EINVAL          => "Invalid argument",
        ENFILE          => "File table overflow",
        EMFILE          => "Too many open files",
        ENOTTY          => "Not a typewriter",
        ETXTBSY         => "Text file busy",
        EFBIG           => "File too large",
        ENOSPC          => "No space left on device",
        ESPIPE          => "Illegal seek",
        EROFS           => "Read-only file system",
        EMLINK          => "Too many links",
        EPIPE           => "Broken pipe",
        EDOM            => "Math argument out of domain of func",
        ERANGE          => "Math result not representable",
        EDEADLK         => "Resource deadlock would occur",
        ENAMETOOLONG    => "File name too long",
        ENOLCK          => "No record locks available",
        ENOSYS          => "Function not implemented",
        ENOTEMPTY       => "Directory not empty",
        ELOOP           => "Too many symbolic links encountered",
        ENOMSG          => "No message of desired type",
        EIDRM           => "Identifier removed",
        EINPROGRESS     => "Operation now in progress",
        EALREADY        => "Operation already in progress",
        ENOTSOCK        => "Socket operation on non-socket",
        EDESTADDRREQ    => "Destination address required",
        EMSGSIZE        => "Message too long",
        EPROTOTYPE      => "Protocol wrong type for socket",
        ENOPROTOOPT     => "Protocol not available",
        EPROTONOSUPPORT => "Protocol not supported",
        ESOCKTNOSUPPORT => "Socket type not supported",
        EPFNOSUPPORT    => "Protocol family not supported",
        EAFNOSUPPORT    => "Address family not supported by protocol",
        EADDRINUSE      => "Address already in use",
        EADDRNOTAVAIL   => "Cannot assign requested address",
        ENETDOWN        => "Network is down",
        ENETUNREACH     => "Network is unreachable",
        ENETRESET       => "Network dropped connection because of reset",
        ECONNABORTED    => "Software caused connection abort",
        ECONNRESET      => "Connection reset by peer",
        ENOBUFS         => "No buffer space available",
        EISCONN         => "Transport endpoint is already connected",
        ENOTCONN        => "Transport endpoint is not connected",
        ESHUTDOWN       => "Cannot send after transport endpoint shutdown",
        ETOOMANYREFS    => "Too many references: cannot splice",
        ETIMEDOUT       => "Connection timed out",
        ECONNREFUSED    => "Connection refused",
        EHOSTDOWN       => "Host is down",
        EHOSTUNREACH    => "No route to host",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ECHRNG          => "Channel number out of range",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EL2NSYNC        => "Level 2 not synchronized",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EL3HLT          => "Level 3 halted",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EL3RST          => "Level 3 reset",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ELNRNG          => "Link number out of range",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EUNATCH         => "Protocol driver not attached",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENOCSI          => "No CSI structure available",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EL2HLT          => "Level 2 halted",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EBADE           => "Invalid exchange",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EBADR           => "Invalid request descriptor",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EXFULL          => "Exchange full",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENOANO          => "No anode",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EBADRQC         => "Invalid request code",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EBADSLT         => "Invalid slot",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EBFONT          => "Bad font file format",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENOSTR          => "Device not a stream",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENODATA         => "No data available",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ETIME           => "Timer expired",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENOSR           => "Out of streams resources",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENONET          => "Machine is not on the network",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENOPKG          => "Package not installed",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EREMOTE         => "Object is remote",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENOLINK         => "Link has been severed",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EADV            => "Advertise error",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ESRMNT          => "Srmount error",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ECOMM           => "Communication error on send",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EPROTO          => "Protocol error",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EMULTIHOP       => "Multihop attempted",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EDOTDOT         => "RFS specific error",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EBADMSG         => "Not a data message",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EOVERFLOW       => "Value too large for defined data type",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENOTUNIQ        => "Name not unique on network",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EBADFD          => "File descriptor in bad state",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EREMCHG         => "Remote address changed",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ELIBACC         => "Can not access a needed shared library",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ELIBBAD         => "Accessing a corrupted shared library",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ELIBSCN         => ".lib section in a.out corrupted",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ELIBMAX         => "Attempting to link in too many shared libraries",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ELIBEXEC        => "Cannot exec a shared library directly",

        #[cfg(any(target_os = "linux", target_os = "android", target_os = "openbsd"))]
        EILSEQ          => "Illegal byte sequence",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ERESTART        => "Interrupted system call should be restarted",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ESTRPIPE        => "Streams pipe error",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EUSERS          => "Too many users",

        #[cfg(any(target_os = "linux", target_os = "android", target_os = "netbsd"))]
        EOPNOTSUPP      => "Operation not supported on transport endpoint",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ESTALE          => "Stale file handle",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EUCLEAN         => "Structure needs cleaning",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENOTNAM         => "Not a XENIX named type file",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENAVAIL         => "No XENIX semaphores available",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EISNAM          => "Is a named type file",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EREMOTEIO       => "Remote I/O error",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EDQUOT          => "Quota exceeded",

        #[cfg(any(target_os = "linux", target_os = "android",
                  target_os = "openbsd", target_os = "dragonfly"))]
        ENOMEDIUM       => "No medium found",

        #[cfg(any(target_os = "linux", target_os = "android", target_os = "openbsd"))]
        EMEDIUMTYPE     => "Wrong medium type",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ECANCELED       => "Operation canceled",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENOKEY          => "Required key not available",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EKEYEXPIRED     => "Key has expired",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EKEYREVOKED     => "Key has been revoked",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EKEYREJECTED    => "Key was rejected by service",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        EOWNERDEAD      => "Owner died",

        #[cfg(any(target_os = "linux", target_os = "android"))]
        ENOTRECOVERABLE => "State not recoverable",

        #[cfg(all(target_os = "linux", not(target_arch="mips")))]
        ERFKILL         => "Operation not possible due to RF-kill",

        #[cfg(all(target_os = "linux", not(target_arch="mips")))]
        EHWPOISON       => "Memory page has hardware error",

        #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
        EDOOFUS         => "Programming error",

        #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
        EMULTIHOP       => "Multihop attempted",

        #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
        ENOLINK         => "Link has been severed",

        #[cfg(target_os = "freebsd")]
        ENOTCAPABLE     => "Capabilities insufficient",

        #[cfg(target_os = "freebsd")]
        ECAPMODE        => "Not permitted in capability mode",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        ENEEDAUTH       => "Need authenticator",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EOVERFLOW       => "Value too large to be stored in data type",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "netbsd"))]
        EILSEQ          => "Illegal byte sequence",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        ENOATTR         => "Attribute not found",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EBADMSG         => "Bad message",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EPROTO          => "Protocol error",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "ios", target_os = "openbsd", ))]
        ENOTRECOVERABLE => "State not recoverable",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "ios", target_os = "openbsd"))]
        EOWNERDEAD      => "Previous owner died",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        ENOTSUP         => "Operation not supported",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EPROCLIM        => "Too many processes",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EUSERS          => "Too many users",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EDQUOT          => "Disc quota exceeded",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        ESTALE          => "Stale NFS file handle",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EREMOTE         => "Too many levels of remote in path",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EBADRPC         => "RPC struct is bad",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        ERPCMISMATCH    => "RPC version wrong",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EPROGUNAVAIL    => "RPC prog. not avail",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EPROGMISMATCH   => "Program version wrong",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EPROCUNAVAIL    => "Bad procedure for program",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EFTYPE          => "Inappropriate file type or format",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        EAUTH           => "Authentication error",

        #[cfg(any(target_os = "macos", target_os = "freebsd",
                  target_os = "dragonfly", target_os = "ios",
                  target_os = "openbsd", target_os = "netbsd"))]
        ECANCELED       => "Operation canceled",

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        EPWROFF         => "Device power is off",

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        EDEVERR         => "Device error, e.g. paper out",

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        EBADEXEC        => "Bad executable",

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        EBADARCH        => "Bad CPU type in executable",

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        ESHLIBVERS      => "Shared library version mismatch",

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        EBADMACHO       => "Malformed Macho file",

        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "netbsd"))]
        EMULTIHOP       => "Reserved",

        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "netbsd"))]
        ENODATA         => "No message available on STREAM",

        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "netbsd"))]
        ENOLINK         => "Reserved",

        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "netbsd"))]
        ENOSR           => "No STREAM resources",

        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "netbsd"))]
        ENOSTR          => "Not a STREAM",

        #[cfg(any(target_os = "macos", target_os = "ios", target_os = "netbsd"))]
        ETIME           => "STREAM ioctl timeout",

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        EOPNOTSUPP      => "Operation not supported on socket",

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        ENOPOLICY       => "No such policy registered",

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        EQFULL          => "Interface output queue is full",

        #[cfg(target_os = "openbsd")]
        EOPNOTSUPP      => "Operation not supported",

        #[cfg(target_os = "openbsd")]
        EIPSEC          => "IPsec processing failure",

        #[cfg(target_os = "dragonfly")]
        EASYNC          => "Async",
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod consts {
    use libc;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    pub enum Errno {
        UnknownErrno    = 0,
        EPERM           = libc::EPERM,
        ENOENT          = libc::ENOENT,
        ESRCH           = libc::ESRCH,
        EINTR           = libc::EINTR,
        EIO             = libc::EIO,
        ENXIO           = libc::ENXIO,
        E2BIG           = libc::E2BIG,
        ENOEXEC         = libc::ENOEXEC,
        EBADF           = libc::EBADF,
        ECHILD          = libc::ECHILD,
        EAGAIN          = libc::EAGAIN,
        ENOMEM          = libc::ENOMEM,
        EACCES          = libc::EACCES,
        EFAULT          = libc::EFAULT,
        ENOTBLK         = libc::ENOTBLK,
        EBUSY           = libc::EBUSY,
        EEXIST          = libc::EEXIST,
        EXDEV           = libc::EXDEV,
        ENODEV          = libc::ENODEV,
        ENOTDIR         = libc::ENOTDIR,
        EISDIR          = libc::EISDIR,
        EINVAL          = libc::EINVAL,
        ENFILE          = libc::ENFILE,
        EMFILE          = libc::EMFILE,
        ENOTTY          = libc::ENOTTY,
        ETXTBSY         = libc::ETXTBSY,
        EFBIG           = libc::EFBIG,
        ENOSPC          = libc::ENOSPC,
        ESPIPE          = libc::ESPIPE,
        EROFS           = libc::EROFS,
        EMLINK          = libc::EMLINK,
        EPIPE           = libc::EPIPE,
        EDOM            = libc::EDOM,
        ERANGE          = libc::ERANGE,
        EDEADLK         = libc::EDEADLK,
        ENAMETOOLONG    = libc::ENAMETOOLONG,
        ENOLCK          = libc::ENOLCK,
        ENOSYS          = libc::ENOSYS,
        ENOTEMPTY       = libc::ENOTEMPTY,
        ELOOP           = libc::ELOOP,
        ENOMSG          = libc::ENOMSG,
        EIDRM           = libc::EIDRM,
        ECHRNG          = libc::ECHRNG,
        EL2NSYNC        = libc::EL2NSYNC,
        EL3HLT          = libc::EL3HLT,
        EL3RST          = libc::EL3RST,
        ELNRNG          = libc::ELNRNG,
        EUNATCH         = libc::EUNATCH,
        ENOCSI          = libc::ENOCSI,
        EL2HLT          = libc::EL2HLT,
        EBADE           = libc::EBADE,
        EBADR           = libc::EBADR,
        EXFULL          = libc::EXFULL,
        ENOANO          = libc::ENOANO,
        EBADRQC         = libc::EBADRQC,
        EBADSLT         = libc::EBADSLT,
        EBFONT          = libc::EBFONT,
        ENOSTR          = libc::ENOSTR,
        ENODATA         = libc::ENODATA,
        ETIME           = libc::ETIME,
        ENOSR           = libc::ENOSR,
        ENONET          = libc::ENONET,
        ENOPKG          = libc::ENOPKG,
        EREMOTE         = libc::EREMOTE,
        ENOLINK         = libc::ENOLINK,
        EADV            = libc::EADV,
        ESRMNT          = libc::ESRMNT,
        ECOMM           = libc::ECOMM,
        EPROTO          = libc::EPROTO,
        EMULTIHOP       = libc::EMULTIHOP,
        EDOTDOT         = libc::EDOTDOT,
        EBADMSG         = libc::EBADMSG,
        EOVERFLOW       = libc::EOVERFLOW,
        ENOTUNIQ        = libc::ENOTUNIQ,
        EBADFD          = libc::EBADFD,
        EREMCHG         = libc::EREMCHG,
        ELIBACC         = libc::ELIBACC,
        ELIBBAD         = libc::ELIBBAD,
        ELIBSCN         = libc::ELIBSCN,
        ELIBMAX         = libc::ELIBMAX,
        ELIBEXEC        = libc::ELIBEXEC,
        EILSEQ          = libc::EILSEQ,
        ERESTART        = libc::ERESTART,
        ESTRPIPE        = libc::ESTRPIPE,
        EUSERS          = libc::EUSERS,
        ENOTSOCK        = libc::ENOTSOCK,
        EDESTADDRREQ    = libc::EDESTADDRREQ,
        EMSGSIZE        = libc::EMSGSIZE,
        EPROTOTYPE      = libc::EPROTOTYPE,
        ENOPROTOOPT     = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        EOPNOTSUPP      = libc::EOPNOTSUPP,
        EPFNOSUPPORT    = libc::EPFNOSUPPORT,
        EAFNOSUPPORT    = libc::EAFNOSUPPORT,
        EADDRINUSE      = libc::EADDRINUSE,
        EADDRNOTAVAIL   = libc::EADDRNOTAVAIL,
        ENETDOWN        = libc::ENETDOWN,
        ENETUNREACH     = libc::ENETUNREACH,
        ENETRESET       = libc::ENETRESET,
        ECONNABORTED    = libc::ECONNABORTED,
        ECONNRESET      = libc::ECONNRESET,
        ENOBUFS         = libc::ENOBUFS,
        EISCONN         = libc::EISCONN,
        ENOTCONN        = libc::ENOTCONN,
        ESHUTDOWN       = libc::ESHUTDOWN,
        ETOOMANYREFS    = libc::ETOOMANYREFS,
        ETIMEDOUT       = libc::ETIMEDOUT,
        ECONNREFUSED    = libc::ECONNREFUSED,
        EHOSTDOWN       = libc::EHOSTDOWN,
        EHOSTUNREACH    = libc::EHOSTUNREACH,
        EALREADY        = libc::EALREADY,
        EINPROGRESS     = libc::EINPROGRESS,
        ESTALE          = libc::ESTALE,
        EUCLEAN         = libc::EUCLEAN,
        ENOTNAM         = libc::ENOTNAM,
        ENAVAIL         = libc::ENAVAIL,
        EISNAM          = libc::EISNAM,
        EREMOTEIO       = libc::EREMOTEIO,
        EDQUOT          = libc::EDQUOT,
        ENOMEDIUM       = libc::ENOMEDIUM,
        EMEDIUMTYPE     = libc::EMEDIUMTYPE,
        ECANCELED       = libc::ECANCELED,
        ENOKEY          = libc::ENOKEY,
        EKEYEXPIRED     = libc::EKEYEXPIRED,
        EKEYREVOKED     = libc::EKEYREVOKED,
        EKEYREJECTED    = libc::EKEYREJECTED,
        EOWNERDEAD      = libc::EOWNERDEAD,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        #[cfg(not(any(target_os = "android", target_arch="mips")))]
        ERFKILL         = libc::ERFKILL,
        #[cfg(not(any(target_os = "android", target_arch="mips")))]
        EHWPOISON       = libc::EHWPOISON,
    }

    pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
    pub const EDEADLOCK:   Errno = Errno::EDEADLK;
    pub const ENOTSUP:     Errno = Errno::EOPNOTSUPP;

    pub fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EAGAIN => EAGAIN,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EDEADLK => EDEADLK,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::ELOOP => ELOOP,
            libc::ENOMSG => ENOMSG,
            libc::EIDRM => EIDRM,
            libc::ECHRNG => ECHRNG,
            libc::EL2NSYNC => EL2NSYNC,
            libc::EL3HLT => EL3HLT,
            libc::EL3RST => EL3RST,
            libc::ELNRNG => ELNRNG,
            libc::EUNATCH => EUNATCH,
            libc::ENOCSI => ENOCSI,
            libc::EL2HLT => EL2HLT,
            libc::EBADE => EBADE,
            libc::EBADR => EBADR,
            libc::EXFULL => EXFULL,
            libc::ENOANO => ENOANO,
            libc::EBADRQC => EBADRQC,
            libc::EBADSLT => EBADSLT,
            libc::EBFONT => EBFONT,
            libc::ENOSTR => ENOSTR,
            libc::ENODATA => ENODATA,
            libc::ETIME => ETIME,
            libc::ENOSR => ENOSR,
            libc::ENONET => ENONET,
            libc::ENOPKG => ENOPKG,
            libc::EREMOTE => EREMOTE,
            libc::ENOLINK => ENOLINK,
            libc::EADV => EADV,
            libc::ESRMNT => ESRMNT,
            libc::ECOMM => ECOMM,
            libc::EPROTO => EPROTO,
            libc::EMULTIHOP => EMULTIHOP,
            libc::EDOTDOT => EDOTDOT,
            libc::EBADMSG => EBADMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::ENOTUNIQ => ENOTUNIQ,
            libc::EBADFD => EBADFD,
            libc::EREMCHG => EREMCHG,
            libc::ELIBACC => ELIBACC,
            libc::ELIBBAD => ELIBBAD,
            libc::ELIBSCN => ELIBSCN,
            libc::ELIBMAX => ELIBMAX,
            libc::ELIBEXEC => ELIBEXEC,
            libc::EILSEQ => EILSEQ,
            libc::ERESTART => ERESTART,
            libc::ESTRPIPE => ESTRPIPE,
            libc::EUSERS => EUSERS,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::EALREADY => EALREADY,
            libc::EINPROGRESS => EINPROGRESS,
            libc::ESTALE => ESTALE,
            libc::EUCLEAN => EUCLEAN,
            libc::ENOTNAM => ENOTNAM,
            libc::ENAVAIL => ENAVAIL,
            libc::EISNAM => EISNAM,
            libc::EREMOTEIO => EREMOTEIO,
            libc::EDQUOT => EDQUOT,
            libc::ENOMEDIUM => ENOMEDIUM,
            libc::EMEDIUMTYPE => EMEDIUMTYPE,
            libc::ECANCELED => ECANCELED,
            libc::ENOKEY => ENOKEY,
            libc::EKEYEXPIRED => EKEYEXPIRED,
            libc::EKEYREVOKED => EKEYREVOKED,
            libc::EKEYREJECTED => EKEYREJECTED,
            libc::EOWNERDEAD => EOWNERDEAD,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            #[cfg(not(any(target_os = "android", target_arch="mips")))]
            libc::ERFKILL => ERFKILL,
            #[cfg(not(any(target_os = "android", target_arch="mips")))]
            libc::EHWPOISON => EHWPOISON,
            _   => UnknownErrno,
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod consts {
    use libc;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    pub enum Errno {
        UnknownErrno    = 0,
        EPERM           = libc::EPERM,
        ENOENT          = libc::ENOENT,
        ESRCH           = libc::ESRCH,
        EINTR           = libc::EINTR,
        EIO             = libc::EIO,
        ENXIO           = libc::ENXIO,
        E2BIG           = libc::E2BIG,
        ENOEXEC         = libc::ENOEXEC,
        EBADF           = libc::EBADF,
        ECHILD          = libc::ECHILD,
        EDEADLK         = libc::EDEADLK,
        ENOMEM          = libc::ENOMEM,
        EACCES          = libc::EACCES,
        EFAULT          = libc::EFAULT,
        ENOTBLK         = libc::ENOTBLK,
        EBUSY           = libc::EBUSY,
        EEXIST          = libc::EEXIST,
        EXDEV           = libc::EXDEV,
        ENODEV          = libc::ENODEV,
        ENOTDIR         = libc::ENOTDIR,
        EISDIR          = libc::EISDIR,
        EINVAL          = libc::EINVAL,
        ENFILE          = libc::ENFILE,
        EMFILE          = libc::EMFILE,
        ENOTTY          = libc::ENOTTY,
        ETXTBSY         = libc::ETXTBSY,
        EFBIG           = libc::EFBIG,
        ENOSPC          = libc::ENOSPC,
        ESPIPE          = libc::ESPIPE,
        EROFS           = libc::EROFS,
        EMLINK          = libc::EMLINK,
        EPIPE           = libc::EPIPE,
        EDOM            = libc::EDOM,
        ERANGE          = libc::ERANGE,
        EAGAIN          = libc::EAGAIN,
        EINPROGRESS     = libc::EINPROGRESS,
        EALREADY        = libc::EALREADY,
        ENOTSOCK        = libc::ENOTSOCK,
        EDESTADDRREQ    = libc::EDESTADDRREQ,
        EMSGSIZE        = libc::EMSGSIZE,
        EPROTOTYPE      = libc::EPROTOTYPE,
        ENOPROTOOPT     = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        ENOTSUP         = libc::ENOTSUP,
        EPFNOSUPPORT    = libc::EPFNOSUPPORT,
        EAFNOSUPPORT    = libc::EAFNOSUPPORT,
        EADDRINUSE      = libc::EADDRINUSE,
        EADDRNOTAVAIL   = libc::EADDRNOTAVAIL,
        ENETDOWN        = libc::ENETDOWN,
        ENETUNREACH     = libc::ENETUNREACH,
        ENETRESET       = libc::ENETRESET,
        ECONNABORTED    = libc::ECONNABORTED,
        ECONNRESET      = libc::ECONNRESET,
        ENOBUFS         = libc::ENOBUFS,
        EISCONN         = libc::EISCONN,
        ENOTCONN        = libc::ENOTCONN,
        ESHUTDOWN       = libc::ESHUTDOWN,
        ETOOMANYREFS    = libc::ETOOMANYREFS,
        ETIMEDOUT       = libc::ETIMEDOUT,
        ECONNREFUSED    = libc::ECONNREFUSED,
        ELOOP           = libc::ELOOP,
        ENAMETOOLONG    = libc::ENAMETOOLONG,
        EHOSTDOWN       = libc::EHOSTDOWN,
        EHOSTUNREACH    = libc::EHOSTUNREACH,
        ENOTEMPTY       = libc::ENOTEMPTY,
        EPROCLIM        = libc::EPROCLIM,
        EUSERS          = libc::EUSERS,
        EDQUOT          = libc::EDQUOT,
        ESTALE          = libc::ESTALE,
        EREMOTE         = libc::EREMOTE,
        EBADRPC         = libc::EBADRPC,
        ERPCMISMATCH    = libc::ERPCMISMATCH,
        EPROGUNAVAIL    = libc::EPROGUNAVAIL,
        EPROGMISMATCH   = libc::EPROGMISMATCH,
        EPROCUNAVAIL    = libc::EPROCUNAVAIL,
        ENOLCK          = libc::ENOLCK,
        ENOSYS          = libc::ENOSYS,
        EFTYPE          = libc::EFTYPE,
        EAUTH           = libc::EAUTH,
        ENEEDAUTH       = libc::ENEEDAUTH,
        EPWROFF         = libc::EPWROFF,
        EDEVERR         = libc::EDEVERR,
        EOVERFLOW       = libc::EOVERFLOW,
        EBADEXEC        = libc::EBADEXEC,
        EBADARCH        = libc::EBADARCH,
        ESHLIBVERS      = libc::ESHLIBVERS,
        EBADMACHO       = libc::EBADMACHO,
        ECANCELED       = libc::ECANCELED,
        EIDRM           = libc::EIDRM,
        ENOMSG          = libc::ENOMSG,
        EILSEQ          = libc::EILSEQ,
        ENOATTR         = libc::ENOATTR,
        EBADMSG         = libc::EBADMSG,
        EMULTIHOP       = libc::EMULTIHOP,
        ENODATA         = libc::ENODATA,
        ENOLINK         = libc::ENOLINK,
        ENOSR           = libc::ENOSR,
        ENOSTR          = libc::ENOSTR,
        EPROTO          = libc::EPROTO,
        ETIME           = libc::ETIME,
        EOPNOTSUPP      = libc::EOPNOTSUPP,
        ENOPOLICY       = libc::ENOPOLICY,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        EOWNERDEAD      = libc::EOWNERDEAD,
        EQFULL          = libc::EQFULL,
    }

    pub const ELAST: Errno       = Errno::EQFULL;
    pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
    pub const EDEADLOCK:   Errno = Errno::EDEADLK;

    pub const EL2NSYNC: Errno = Errno::UnknownErrno;

    pub fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::ENOTSUP => ENOTSUP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::EPWROFF => EPWROFF,
            libc::EDEVERR => EDEVERR,
            libc::EOVERFLOW => EOVERFLOW,
            libc::EBADEXEC => EBADEXEC,
            libc::EBADARCH => EBADARCH,
            libc::ESHLIBVERS => ESHLIBVERS,
            libc::EBADMACHO => EBADMACHO,
            libc::ECANCELED => ECANCELED,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::EILSEQ => EILSEQ,
            libc::ENOATTR => ENOATTR,
            libc::EBADMSG => EBADMSG,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENODATA => ENODATA,
            libc::ENOLINK => ENOLINK,
            libc::ENOSR => ENOSR,
            libc::ENOSTR => ENOSTR,
            libc::EPROTO => EPROTO,
            libc::ETIME => ETIME,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::ENOPOLICY => ENOPOLICY,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            libc::EOWNERDEAD => EOWNERDEAD,
            libc::EQFULL => EQFULL,
            _   => UnknownErrno,
        }
    }
}

#[cfg(target_os = "freebsd")]
mod consts {
    use libc;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    pub enum Errno {
        UnknownErrno    = 0,
        EPERM           = libc::EPERM,
        ENOENT          = libc::ENOENT,
        ESRCH           = libc::ESRCH,
        EINTR           = libc::EINTR,
        EIO             = libc::EIO,
        ENXIO           = libc::ENXIO,
        E2BIG           = libc::E2BIG,
        ENOEXEC         = libc::ENOEXEC,
        EBADF           = libc::EBADF,
        ECHILD          = libc::ECHILD,
        EDEADLK         = libc::EDEADLK,
        ENOMEM          = libc::ENOMEM,
        EACCES          = libc::EACCES,
        EFAULT          = libc::EFAULT,
        ENOTBLK         = libc::ENOTBLK,
        EBUSY           = libc::EBUSY,
        EEXIST          = libc::EEXIST,
        EXDEV           = libc::EXDEV,
        ENODEV          = libc::ENODEV,
        ENOTDIR         = libc::ENOTDIR,
        EISDIR          = libc::EISDIR,
        EINVAL          = libc::EINVAL,
        ENFILE          = libc::ENFILE,
        EMFILE          = libc::EMFILE,
        ENOTTY          = libc::ENOTTY,
        ETXTBSY         = libc::ETXTBSY,
        EFBIG           = libc::EFBIG,
        ENOSPC          = libc::ENOSPC,
        ESPIPE          = libc::ESPIPE,
        EROFS           = libc::EROFS,
        EMLINK          = libc::EMLINK,
        EPIPE           = libc::EPIPE,
        EDOM            = libc::EDOM,
        ERANGE          = libc::ERANGE,
        EAGAIN          = libc::EAGAIN,
        EINPROGRESS     = libc::EINPROGRESS,
        EALREADY        = libc::EALREADY,
        ENOTSOCK        = libc::ENOTSOCK,
        EDESTADDRREQ    = libc::EDESTADDRREQ,
        EMSGSIZE        = libc::EMSGSIZE,
        EPROTOTYPE      = libc::EPROTOTYPE,
        ENOPROTOOPT     = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        ENOTSUP         = libc::ENOTSUP,
        EPFNOSUPPORT    = libc::EPFNOSUPPORT,
        EAFNOSUPPORT    = libc::EAFNOSUPPORT,
        EADDRINUSE      = libc::EADDRINUSE,
        EADDRNOTAVAIL   = libc::EADDRNOTAVAIL,
        ENETDOWN        = libc::ENETDOWN,
        ENETUNREACH     = libc::ENETUNREACH,
        ENETRESET       = libc::ENETRESET,
        ECONNABORTED    = libc::ECONNABORTED,
        ECONNRESET      = libc::ECONNRESET,
        ENOBUFS         = libc::ENOBUFS,
        EISCONN         = libc::EISCONN,
        ENOTCONN        = libc::ENOTCONN,
        ESHUTDOWN       = libc::ESHUTDOWN,
        ETOOMANYREFS    = libc::ETOOMANYREFS,
        ETIMEDOUT       = libc::ETIMEDOUT,
        ECONNREFUSED    = libc::ECONNREFUSED,
        ELOOP           = libc::ELOOP,
        ENAMETOOLONG    = libc::ENAMETOOLONG,
        EHOSTDOWN       = libc::EHOSTDOWN,
        EHOSTUNREACH    = libc::EHOSTUNREACH,
        ENOTEMPTY       = libc::ENOTEMPTY,
        EPROCLIM        = libc::EPROCLIM,
        EUSERS          = libc::EUSERS,
        EDQUOT          = libc::EDQUOT,
        ESTALE          = libc::ESTALE,
        EREMOTE         = libc::EREMOTE,
        EBADRPC         = libc::EBADRPC,
        ERPCMISMATCH    = libc::ERPCMISMATCH,
        EPROGUNAVAIL    = libc::EPROGUNAVAIL,
        EPROGMISMATCH   = libc::EPROGMISMATCH,
        EPROCUNAVAIL    = libc::EPROCUNAVAIL,
        ENOLCK          = libc::ENOLCK,
        ENOSYS          = libc::ENOSYS,
        EFTYPE          = libc::EFTYPE,
        EAUTH           = libc::EAUTH,
        ENEEDAUTH       = libc::ENEEDAUTH,
        EIDRM           = libc::EIDRM,
        ENOMSG          = libc::ENOMSG,
        EOVERFLOW       = libc::EOVERFLOW,
        ECANCELED       = libc::ECANCELED,
        EILSEQ          = libc::EILSEQ,
        ENOATTR         = libc::ENOATTR,
        EDOOFUS         = libc::EDOOFUS,
        EBADMSG         = libc::EBADMSG,
        EMULTIHOP       = libc::EMULTIHOP,
        ENOLINK         = libc::ENOLINK,
        EPROTO          = libc::EPROTO,
        ENOTCAPABLE     = libc::ENOTCAPABLE,
        ECAPMODE        = libc::ECAPMODE,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        EOWNERDEAD      = libc::EOWNERDEAD,
    }

    pub const ELAST: Errno       = Errno::EOWNERDEAD;
    pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
    pub const EDEADLOCK:   Errno = Errno::EDEADLK;

    pub const EL2NSYNC: Errno = Errno::UnknownErrno;

    pub fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::ENOTSUP => ENOTSUP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::ECANCELED => ECANCELED,
            libc::EILSEQ => EILSEQ,
            libc::ENOATTR => ENOATTR,
            libc::EDOOFUS => EDOOFUS,
            libc::EBADMSG => EBADMSG,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENOLINK => ENOLINK,
            libc::EPROTO => EPROTO,
            libc::ENOTCAPABLE => ENOTCAPABLE,
            libc::ECAPMODE => ECAPMODE,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            libc::EOWNERDEAD => EOWNERDEAD,
            _   => UnknownErrno,
        }
    }
}


#[cfg(target_os = "dragonfly")]
mod consts {
    use libc;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    pub enum Errno {
        UnknownErrno    = 0,
        EPERM           = libc::EPERM,
        ENOENT          = libc::ENOENT,
        ESRCH           = libc::ESRCH,
        EINTR           = libc::EINTR,
        EIO             = libc::EIO,
        ENXIO           = libc::ENXIO,
        E2BIG           = libc::E2BIG,
        ENOEXEC         = libc::ENOEXEC,
        EBADF           = libc::EBADF,
        ECHILD          = libc::ECHILD,
        EDEADLK         = libc::EDEADLK,
        ENOMEM          = libc::ENOMEM,
        EACCES          = libc::EACCES,
        EFAULT          = libc::EFAULT,
        ENOTBLK         = libc::ENOTBLK,
        EBUSY           = libc::EBUSY,
        EEXIST          = libc::EEXIST,
        EXDEV           = libc::EXDEV,
        ENODEV          = libc::ENODEV,
        ENOTDIR         = libc::ENOTDIR,
        EISDIR          = libc::EISDIR,
        EINVAL          = libc::EINVAL,
        ENFILE          = libc::ENFILE,
        EMFILE          = libc::EMFILE,
        ENOTTY          = libc::ENOTTY,
        ETXTBSY         = libc::ETXTBSY,
        EFBIG           = libc::EFBIG,
        ENOSPC          = libc::ENOSPC,
        ESPIPE          = libc::ESPIPE,
        EROFS           = libc::EROFS,
        EMLINK          = libc::EMLINK,
        EPIPE           = libc::EPIPE,
        EDOM            = libc::EDOM,
        ERANGE          = libc::ERANGE,
        EAGAIN          = libc::EAGAIN,
        EINPROGRESS     = libc::EINPROGRESS,
        EALREADY        = libc::EALREADY,
        ENOTSOCK        = libc::ENOTSOCK,
        EDESTADDRREQ    = libc::EDESTADDRREQ,
        EMSGSIZE        = libc::EMSGSIZE,
        EPROTOTYPE      = libc::EPROTOTYPE,
        ENOPROTOOPT     = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        ENOTSUP         = libc::ENOTSUP,
        EPFNOSUPPORT    = libc::EPFNOSUPPORT,
        EAFNOSUPPORT    = libc::EAFNOSUPPORT,
        EADDRINUSE      = libc::EADDRINUSE,
        EADDRNOTAVAIL   = libc::EADDRNOTAVAIL,
        ENETDOWN        = libc::ENETDOWN,
        ENETUNREACH     = libc::ENETUNREACH,
        ENETRESET       = libc::ENETRESET,
        ECONNABORTED    = libc::ECONNABORTED,
        ECONNRESET      = libc::ECONNRESET,
        ENOBUFS         = libc::ENOBUFS,
        EISCONN         = libc::EISCONN,
        ENOTCONN        = libc::ENOTCONN,
        ESHUTDOWN       = libc::ESHUTDOWN,
        ETOOMANYREFS    = libc::ETOOMANYREFS,
        ETIMEDOUT       = libc::ETIMEDOUT,
        ECONNREFUSED    = libc::ECONNREFUSED,
        ELOOP           = libc::ELOOP,
        ENAMETOOLONG    = libc::ENAMETOOLONG,
        EHOSTDOWN       = libc::EHOSTDOWN,
        EHOSTUNREACH    = libc::EHOSTUNREACH,
        ENOTEMPTY       = libc::ENOTEMPTY,
        EPROCLIM        = libc::EPROCLIM,
        EUSERS          = libc::EUSERS,
        EDQUOT          = libc::EDQUOT,
        ESTALE          = libc::ESTALE,
        EREMOTE         = libc::EREMOTE,
        EBADRPC         = libc::EBADRPC,
        ERPCMISMATCH    = libc::ERPCMISMATCH,
        EPROGUNAVAIL    = libc::EPROGUNAVAIL,
        EPROGMISMATCH   = libc::EPROGMISMATCH,
        EPROCUNAVAIL    = libc::EPROCUNAVAIL,
        ENOLCK          = libc::ENOLCK,
        ENOSYS          = libc::ENOSYS,
        EFTYPE          = libc::EFTYPE,
        EAUTH           = libc::EAUTH,
        ENEEDAUTH       = libc::ENEEDAUTH,
        EIDRM           = libc::EIDRM,
        ENOMSG          = libc::ENOMSG,
        EOVERFLOW       = libc::EOVERFLOW,
        ECANCELED       = libc::ECANCELED,
        EILSEQ          = libc::EILSEQ,
        ENOATTR         = libc::ENOATTR,
        EDOOFUS         = libc::EDOOFUS,
        EBADMSG         = libc::EBADMSG,
        EMULTIHOP       = libc::EMULTIHOP,
        ENOLINK         = libc::ENOLINK,
        EPROTO          = libc::EPROTO,
        ENOMEDIUM       = libc::ENOMEDIUM,
        EASYNC          = libc::EASYNC,
    }

    pub const ELAST: Errno       = Errno::EASYNC;
    pub const EWOULDBLOCK: Errno = Errno::EAGAIN;
    pub const EDEADLOCK:   Errno = Errno::EDEADLK;
    pub const EOPNOTSUPP:  Errno = Errno::ENOTSUP;

    pub const EL2NSYNC: Errno = Errno::UnknownErrno;

    pub fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR=> EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::ENOTSUP => ENOTSUP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::ECANCELED => ECANCELED,
            libc::EILSEQ => EILSEQ,
            libc::ENOATTR => ENOATTR,
            libc::EDOOFUS => EDOOFUS,
            libc::EBADMSG => EBADMSG,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENOLINK => ENOLINK,
            libc::EPROTO => EPROTO,
            libc::ENOMEDIUM => ENOMEDIUM,
            libc::EASYNC => EASYNC,
            _   => UnknownErrno,
        }
    }
}


#[cfg(target_os = "openbsd")]
mod consts {
    use libc;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    pub enum Errno {
        UnknownErrno    = 0,
        EPERM           = libc::EPERM,
        ENOENT          = libc::ENOENT,
        ESRCH           = libc::ESRCH,
        EINTR           = libc::EINTR,
        EIO             = libc::EIO,
        ENXIO           = libc::ENXIO,
        E2BIG           = libc::E2BIG,
        ENOEXEC         = libc::ENOEXEC,
        EBADF           = libc::EBADF,
        ECHILD          = libc::ECHILD,
        EDEADLK         = libc::EDEADLK,
        ENOMEM          = libc::ENOMEM,
        EACCES          = libc::EACCES,
        EFAULT          = libc::EFAULT,
        ENOTBLK         = libc::ENOTBLK,
        EBUSY           = libc::EBUSY,
        EEXIST          = libc::EEXIST,
        EXDEV           = libc::EXDEV,
        ENODEV          = libc::ENODEV,
        ENOTDIR         = libc::ENOTDIR,
        EISDIR          = libc::EISDIR,
        EINVAL          = libc::EINVAL,
        ENFILE          = libc::ENFILE,
        EMFILE          = libc::EMFILE,
        ENOTTY          = libc::ENOTTY,
        ETXTBSY         = libc::ETXTBSY,
        EFBIG           = libc::EFBIG,
        ENOSPC          = libc::ENOSPC,
        ESPIPE          = libc::ESPIPE,
        EROFS           = libc::EROFS,
        EMLINK          = libc::EMLINK,
        EPIPE           = libc::EPIPE,
        EDOM            = libc::EDOM,
        ERANGE          = libc::ERANGE,
        EAGAIN          = libc::EAGAIN,
        EINPROGRESS     = libc::EINPROGRESS,
        EALREADY        = libc::EALREADY,
        ENOTSOCK        = libc::ENOTSOCK,
        EDESTADDRREQ    = libc::EDESTADDRREQ,
        EMSGSIZE        = libc::EMSGSIZE,
        EPROTOTYPE      = libc::EPROTOTYPE,
        ENOPROTOOPT     = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        EOPNOTSUPP      = libc::EOPNOTSUPP,
        EPFNOSUPPORT    = libc::EPFNOSUPPORT,
        EAFNOSUPPORT    = libc::EAFNOSUPPORT,
        EADDRINUSE      = libc::EADDRINUSE,
        EADDRNOTAVAIL   = libc::EADDRNOTAVAIL,
        ENETDOWN        = libc::ENETDOWN,
        ENETUNREACH     = libc::ENETUNREACH,
        ENETRESET       = libc::ENETRESET,
        ECONNABORTED    = libc::ECONNABORTED,
        ECONNRESET      = libc::ECONNRESET,
        ENOBUFS         = libc::ENOBUFS,
        EISCONN         = libc::EISCONN,
        ENOTCONN        = libc::ENOTCONN,
        ESHUTDOWN       = libc::ESHUTDOWN,
        ETOOMANYREFS    = libc::ETOOMANYREFS,
        ETIMEDOUT       = libc::ETIMEDOUT,
        ECONNREFUSED    = libc::ECONNREFUSED,
        ELOOP           = libc::ELOOP,
        ENAMETOOLONG    = libc::ENAMETOOLONG,
        EHOSTDOWN       = libc::EHOSTDOWN,
        EHOSTUNREACH    = libc::EHOSTUNREACH,
        ENOTEMPTY       = libc::ENOTEMPTY,
        EPROCLIM        = libc::EPROCLIM,
        EUSERS          = libc::EUSERS,
        EDQUOT          = libc::EDQUOT,
        ESTALE          = libc::ESTALE,
        EREMOTE         = libc::EREMOTE,
        EBADRPC         = libc::EBADRPC,
        ERPCMISMATCH    = libc::ERPCMISMATCH,
        EPROGUNAVAIL    = libc::EPROGUNAVAIL,
        EPROGMISMATCH   = libc::EPROGMISMATCH,
        EPROCUNAVAIL    = libc::EPROCUNAVAIL,
        ENOLCK          = libc::ENOLCK,
        ENOSYS          = libc::ENOSYS,
        EFTYPE          = libc::EFTYPE,
        EAUTH           = libc::EAUTH,
        ENEEDAUTH       = libc::ENEEDAUTH,
        EIPSEC          = libc::EIPSEC,
        ENOATTR         = libc::ENOATTR,
        EILSEQ          = libc::EILSEQ,
        ENOMEDIUM       = libc::ENOMEDIUM,
        EMEDIUMTYPE     = libc::EMEDIUMTYPE,
        EOVERFLOW       = libc::EOVERFLOW,
        ECANCELED       = libc::ECANCELED,
        EIDRM           = libc::EIDRM,
        ENOMSG          = libc::ENOMSG,
        ENOTSUP         = libc::ENOTSUP,
        EBADMSG         = libc::EBADMSG,
        ENOTRECOVERABLE = libc::ENOTRECOVERABLE,
        EOWNERDEAD      = libc::EOWNERDEAD,
        EPROTO          = libc::EPROTO,
    }

    pub const ELAST: Errno       = Errno::ENOTSUP;
    pub const EWOULDBLOCK: Errno = Errno::EAGAIN;

    pub const EL2NSYNC: Errno = Errno::UnknownErrno;

    pub fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::EIPSEC => EIPSEC,
            libc::ENOATTR => ENOATTR,
            libc::EILSEQ => EILSEQ,
            libc::ENOMEDIUM => ENOMEDIUM,
            libc::EMEDIUMTYPE => EMEDIUMTYPE,
            libc::EOVERFLOW => EOVERFLOW,
            libc::ECANCELED => ECANCELED,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::ENOTSUP => ENOTSUP,
            libc::EBADMSG => EBADMSG,
            libc::ENOTRECOVERABLE => ENOTRECOVERABLE,
            libc::EOWNERDEAD => EOWNERDEAD,
            libc::EPROTO => EPROTO,
            _   => UnknownErrno,
        }
    }
}

#[cfg(target_os = "netbsd")]
mod consts {
    use libc;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(i32)]
    pub enum Errno {
        UnknownErrno    = 0,
        EPERM           = libc::EPERM,
        ENOENT          = libc::ENOENT,
        ESRCH           = libc::ESRCH,
        EINTR           = libc::EINTR,
        EIO             = libc::EIO,
        ENXIO           = libc::ENXIO,
        E2BIG           = libc::E2BIG,
        ENOEXEC         = libc::ENOEXEC,
        EBADF           = libc::EBADF,
        ECHILD          = libc::ECHILD,
        EDEADLK         = libc::EDEADLK,
        ENOMEM          = libc::ENOMEM,
        EACCES          = libc::EACCES,
        EFAULT          = libc::EFAULT,
        ENOTBLK         = libc::ENOTBLK,
        EBUSY           = libc::EBUSY,
        EEXIST          = libc::EEXIST,
        EXDEV           = libc::EXDEV,
        ENODEV          = libc::ENODEV,
        ENOTDIR         = libc::ENOTDIR,
        EISDIR          = libc::EISDIR,
        EINVAL          = libc::EINVAL,
        ENFILE          = libc::ENFILE,
        EMFILE          = libc::EMFILE,
        ENOTTY          = libc::ENOTTY,
        ETXTBSY         = libc::ETXTBSY,
        EFBIG           = libc::EFBIG,
        ENOSPC          = libc::ENOSPC,
        ESPIPE          = libc::ESPIPE,
        EROFS           = libc::EROFS,
        EMLINK          = libc::EMLINK,
        EPIPE           = libc::EPIPE,
        EDOM            = libc::EDOM,
        ERANGE          = libc::ERANGE,
        EAGAIN          = libc::EAGAIN,
        EINPROGRESS     = libc::EINPROGRESS,
        EALREADY        = libc::EALREADY,
        ENOTSOCK        = libc::ENOTSOCK,
        EDESTADDRREQ    = libc::EDESTADDRREQ,
        EMSGSIZE        = libc::EMSGSIZE,
        EPROTOTYPE      = libc::EPROTOTYPE,
        ENOPROTOOPT     = libc::ENOPROTOOPT,
        EPROTONOSUPPORT = libc::EPROTONOSUPPORT,
        ESOCKTNOSUPPORT = libc::ESOCKTNOSUPPORT,
        EOPNOTSUPP      = libc::EOPNOTSUPP,
        EPFNOSUPPORT    = libc::EPFNOSUPPORT,
        EAFNOSUPPORT    = libc::EAFNOSUPPORT,
        EADDRINUSE      = libc::EADDRINUSE,
        EADDRNOTAVAIL   = libc::EADDRNOTAVAIL,
        ENETDOWN        = libc::ENETDOWN,
        ENETUNREACH     = libc::ENETUNREACH,
        ENETRESET       = libc::ENETRESET,
        ECONNABORTED    = libc::ECONNABORTED,
        ECONNRESET      = libc::ECONNRESET,
        ENOBUFS         = libc::ENOBUFS,
        EISCONN         = libc::EISCONN,
        ENOTCONN        = libc::ENOTCONN,
        ESHUTDOWN       = libc::ESHUTDOWN,
        ETOOMANYREFS    = libc::ETOOMANYREFS,
        ETIMEDOUT       = libc::ETIMEDOUT,
        ECONNREFUSED    = libc::ECONNREFUSED,
        ELOOP           = libc::ELOOP,
        ENAMETOOLONG    = libc::ENAMETOOLONG,
        EHOSTDOWN       = libc::EHOSTDOWN,
        EHOSTUNREACH    = libc::EHOSTUNREACH,
        ENOTEMPTY       = libc::ENOTEMPTY,
        EPROCLIM        = libc::EPROCLIM,
        EUSERS          = libc::EUSERS,
        EDQUOT          = libc::EDQUOT,
        ESTALE          = libc::ESTALE,
        EREMOTE         = libc::EREMOTE,
        EBADRPC         = libc::EBADRPC,
        ERPCMISMATCH    = libc::ERPCMISMATCH,
        EPROGUNAVAIL    = libc::EPROGUNAVAIL,
        EPROGMISMATCH   = libc::EPROGMISMATCH,
        EPROCUNAVAIL    = libc::EPROCUNAVAIL,
        ENOLCK          = libc::ENOLCK,
        ENOSYS          = libc::ENOSYS,
        EFTYPE          = libc::EFTYPE,
        EAUTH           = libc::EAUTH,
        ENEEDAUTH       = libc::ENEEDAUTH,
        EIDRM           = libc::EIDRM,
        ENOMSG          = libc::ENOMSG,
        EOVERFLOW       = libc::EOVERFLOW,
        EILSEQ          = libc::EILSEQ,
        ENOTSUP         = libc::ENOTSUP,
        ECANCELED       = libc::ECANCELED,
        EBADMSG         = libc::EBADMSG,
        ENODATA         = libc::ENODATA,
        ENOSR           = libc::ENOSR,
        ENOSTR          = libc::ENOSTR,
        ETIME           = libc::ETIME,
        ENOATTR         = libc::ENOATTR,
        EMULTIHOP       = libc::EMULTIHOP,
        ENOLINK         = libc::ENOLINK,
        EPROTO          = libc::EPROTO,
    }

    pub const ELAST: Errno       = Errno::ENOTSUP;
    pub const EWOULDBLOCK: Errno = Errno::EAGAIN;

    pub const EL2NSYNC: Errno = Errno::UnknownErrno;

    pub fn from_i32(e: i32) -> Errno {
        use self::Errno::*;

        match e {
            libc::EPERM => EPERM,
            libc::ENOENT => ENOENT,
            libc::ESRCH => ESRCH,
            libc::EINTR => EINTR,
            libc::EIO => EIO,
            libc::ENXIO => ENXIO,
            libc::E2BIG => E2BIG,
            libc::ENOEXEC => ENOEXEC,
            libc::EBADF => EBADF,
            libc::ECHILD => ECHILD,
            libc::EDEADLK => EDEADLK,
            libc::ENOMEM => ENOMEM,
            libc::EACCES => EACCES,
            libc::EFAULT => EFAULT,
            libc::ENOTBLK => ENOTBLK,
            libc::EBUSY => EBUSY,
            libc::EEXIST => EEXIST,
            libc::EXDEV => EXDEV,
            libc::ENODEV => ENODEV,
            libc::ENOTDIR => ENOTDIR,
            libc::EISDIR => EISDIR,
            libc::EINVAL => EINVAL,
            libc::ENFILE => ENFILE,
            libc::EMFILE => EMFILE,
            libc::ENOTTY => ENOTTY,
            libc::ETXTBSY => ETXTBSY,
            libc::EFBIG => EFBIG,
            libc::ENOSPC => ENOSPC,
            libc::ESPIPE => ESPIPE,
            libc::EROFS => EROFS,
            libc::EMLINK => EMLINK,
            libc::EPIPE => EPIPE,
            libc::EDOM => EDOM,
            libc::ERANGE => ERANGE,
            libc::EAGAIN => EAGAIN,
            libc::EINPROGRESS => EINPROGRESS,
            libc::EALREADY => EALREADY,
            libc::ENOTSOCK => ENOTSOCK,
            libc::EDESTADDRREQ => EDESTADDRREQ,
            libc::EMSGSIZE => EMSGSIZE,
            libc::EPROTOTYPE => EPROTOTYPE,
            libc::ENOPROTOOPT => ENOPROTOOPT,
            libc::EPROTONOSUPPORT => EPROTONOSUPPORT,
            libc::ESOCKTNOSUPPORT => ESOCKTNOSUPPORT,
            libc::EOPNOTSUPP => EOPNOTSUPP,
            libc::EPFNOSUPPORT => EPFNOSUPPORT,
            libc::EAFNOSUPPORT => EAFNOSUPPORT,
            libc::EADDRINUSE => EADDRINUSE,
            libc::EADDRNOTAVAIL => EADDRNOTAVAIL,
            libc::ENETDOWN => ENETDOWN,
            libc::ENETUNREACH => ENETUNREACH,
            libc::ENETRESET => ENETRESET,
            libc::ECONNABORTED => ECONNABORTED,
            libc::ECONNRESET => ECONNRESET,
            libc::ENOBUFS => ENOBUFS,
            libc::EISCONN => EISCONN,
            libc::ENOTCONN => ENOTCONN,
            libc::ESHUTDOWN => ESHUTDOWN,
            libc::ETOOMANYREFS => ETOOMANYREFS,
            libc::ETIMEDOUT => ETIMEDOUT,
            libc::ECONNREFUSED => ECONNREFUSED,
            libc::ELOOP => ELOOP,
            libc::ENAMETOOLONG => ENAMETOOLONG,
            libc::EHOSTDOWN => EHOSTDOWN,
            libc::EHOSTUNREACH => EHOSTUNREACH,
            libc::ENOTEMPTY => ENOTEMPTY,
            libc::EPROCLIM => EPROCLIM,
            libc::EUSERS => EUSERS,
            libc::EDQUOT => EDQUOT,
            libc::ESTALE => ESTALE,
            libc::EREMOTE => EREMOTE,
            libc::EBADRPC => EBADRPC,
            libc::ERPCMISMATCH => ERPCMISMATCH,
            libc::EPROGUNAVAIL => EPROGUNAVAIL,
            libc::EPROGMISMATCH => EPROGMISMATCH,
            libc::EPROCUNAVAIL => EPROCUNAVAIL,
            libc::ENOLCK => ENOLCK,
            libc::ENOSYS => ENOSYS,
            libc::EFTYPE => EFTYPE,
            libc::EAUTH => EAUTH,
            libc::ENEEDAUTH => ENEEDAUTH,
            libc::EIDRM => EIDRM,
            libc::ENOMSG => ENOMSG,
            libc::EOVERFLOW => EOVERFLOW,
            libc::EILSEQ => EILSEQ,
            libc::ENOTSUP => ENOTSUP,
            libc::ECANCELED => ECANCELED,
            libc::EBADMSG => EBADMSG,
            libc::ENODATA => ENODATA,
            libc::ENOSR => ENOSR,
            libc::ENOSTR => ENOSTR,
            libc::ETIME => ETIME,
            libc::ENOATTR => ENOATTR,
            libc::EMULTIHOP => EMULTIHOP,
            libc::ENOLINK => ENOLINK,
            libc::EPROTO => EPROTO,
            _   => UnknownErrno,
        }
    }
}
