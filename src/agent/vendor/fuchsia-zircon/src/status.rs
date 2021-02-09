use std::ffi::NulError;
use std::io;
use sys;

/// Status type indicating the result of a Fuchsia syscall.
///
/// This type is generally used to indicate the reason for an error.
/// While this type can contain `Status::OK` (`ZX_OK` in C land), elements of this type are
/// generally constructed using the `ok` method, which checks for `ZX_OK` and returns a
/// `Result<(), Status>` appropriately.
#[repr(C)]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Status(sys::zx_status_t);
impl Status {
    /// Returns `Ok(())` if the status was `OK`,
    /// otherwise returns `Err(status)`.
    pub fn ok(raw: sys::zx_status_t) -> Result<(), Status> {
        if raw == Status::OK.0 {
            Ok(())
        } else {
            Err(Status(raw))
        }
    }

    pub fn from_raw(raw: sys::zx_status_t) -> Self {
        Status(raw)
    }

    pub fn into_raw(self) -> sys::zx_status_t {
        self.0
    }
}
assoc_consts!(Status, [
    OK                 = sys::ZX_OK;
    INTERNAL           = sys::ZX_ERR_INTERNAL;
    NOT_SUPPORTED      = sys::ZX_ERR_NOT_SUPPORTED;
    NO_RESOURCES       = sys::ZX_ERR_NO_RESOURCES;
    NO_MEMORY          = sys::ZX_ERR_NO_MEMORY;
    CALL_FAILED        = sys::ZX_ERR_CALL_FAILED;
    INTERRUPTED_RETRY  = sys::ZX_ERR_INTERRUPTED_RETRY;
    INVALID_ARGS       = sys::ZX_ERR_INVALID_ARGS;
    BAD_HANDLE         = sys::ZX_ERR_BAD_HANDLE;
    WRONG_TYPE         = sys::ZX_ERR_WRONG_TYPE;
    BAD_SYSCALL        = sys::ZX_ERR_BAD_SYSCALL;
    OUT_OF_RANGE       = sys::ZX_ERR_OUT_OF_RANGE;
    BUFFER_TOO_SMALL   = sys::ZX_ERR_BUFFER_TOO_SMALL;
    BAD_STATE          = sys::ZX_ERR_BAD_STATE;
    TIMED_OUT          = sys::ZX_ERR_TIMED_OUT;
    SHOULD_WAIT        = sys::ZX_ERR_SHOULD_WAIT;
    CANCELED           = sys::ZX_ERR_CANCELED;
    PEER_CLOSED        = sys::ZX_ERR_PEER_CLOSED;
    NOT_FOUND          = sys::ZX_ERR_NOT_FOUND;
    ALREADY_EXISTS     = sys::ZX_ERR_ALREADY_EXISTS;
    ALREADY_BOUND      = sys::ZX_ERR_ALREADY_BOUND;
    UNAVAILABLE        = sys::ZX_ERR_UNAVAILABLE;
    ACCESS_DENIED      = sys::ZX_ERR_ACCESS_DENIED;
    IO                 = sys::ZX_ERR_IO;
    IO_REFUSED         = sys::ZX_ERR_IO_REFUSED;
    IO_DATA_INTEGRITY  = sys::ZX_ERR_IO_DATA_INTEGRITY;
    IO_DATA_LOSS       = sys::ZX_ERR_IO_DATA_LOSS;
    BAD_PATH           = sys::ZX_ERR_BAD_PATH;
    NOT_DIR            = sys::ZX_ERR_NOT_DIR;
    NOT_FILE           = sys::ZX_ERR_NOT_FILE;
    FILE_BIG           = sys::ZX_ERR_FILE_BIG;
    NO_SPACE           = sys::ZX_ERR_NO_SPACE;
    STOP               = sys::ZX_ERR_STOP;
    NEXT               = sys::ZX_ERR_NEXT;
]);

impl Status {
    pub fn into_io_error(self) -> io::Error {
        self.into()
    }
}

impl From<io::ErrorKind> for Status {
    fn from(kind: io::ErrorKind) -> Self {
        use std::io::ErrorKind::*;
        match kind {
            NotFound => Status::NOT_FOUND,
            PermissionDenied => Status::ACCESS_DENIED,
            ConnectionRefused => Status::IO_REFUSED,
            ConnectionAborted => Status::PEER_CLOSED,
            AddrInUse => Status::ALREADY_BOUND,
            AddrNotAvailable => Status::UNAVAILABLE,
            BrokenPipe => Status::PEER_CLOSED,
            AlreadyExists => Status::ALREADY_EXISTS,
            WouldBlock => Status::SHOULD_WAIT,
            InvalidInput => Status::INVALID_ARGS,
            TimedOut => Status::TIMED_OUT,
            Interrupted => Status::INTERRUPTED_RETRY,
            UnexpectedEof |
            WriteZero |
            ConnectionReset |
            NotConnected |
            Other | _ => Status::IO,
        }
    }
}

impl From<Status> for io::ErrorKind {
    fn from(status: Status) -> io::ErrorKind {
        use std::io::ErrorKind::*;
        match status {
            Status::INTERRUPTED_RETRY => Interrupted,
            Status::BAD_HANDLE => BrokenPipe,
            Status::TIMED_OUT => TimedOut,
            Status::SHOULD_WAIT => WouldBlock,
            Status::PEER_CLOSED => ConnectionAborted,
            Status::NOT_FOUND => NotFound,
            Status::ALREADY_EXISTS => AlreadyExists,
            Status::ALREADY_BOUND => AlreadyExists,
            Status::UNAVAILABLE => AddrNotAvailable,
            Status::ACCESS_DENIED => PermissionDenied,
            Status::IO_REFUSED => ConnectionRefused,
            Status::IO_DATA_INTEGRITY => InvalidData,

            Status::BAD_PATH |
            Status::INVALID_ARGS |
            Status::OUT_OF_RANGE |
            Status::WRONG_TYPE => InvalidInput,

            Status::OK |
            Status::NEXT |
            Status::STOP |
            Status::NO_SPACE |
            Status::FILE_BIG |
            Status::NOT_FILE |
            Status::NOT_DIR |
            Status::IO_DATA_LOSS |
            Status::IO |
            Status::CANCELED |
            Status::BAD_STATE |
            Status::BUFFER_TOO_SMALL |
            Status::BAD_SYSCALL |
            Status::INTERNAL |
            Status::NOT_SUPPORTED |
            Status::NO_RESOURCES |
            Status::NO_MEMORY |
            Status::CALL_FAILED |
            _ => Other,
        }
    }
}

impl From<io::Error> for Status {
    fn from(err: io::Error) -> Status {
        err.kind().into()
    }
}

impl From<Status> for io::Error {
    fn from(status: Status) -> io::Error {
        io::Error::from(io::ErrorKind::from(status))
    }
}

impl From<NulError> for Status {
    fn from(_error: NulError) -> Status {
        Status::INVALID_ARGS
    }
}
