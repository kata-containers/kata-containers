// Don't throw clippy warnings for manual string stripping.
// The suggested fix with `strip_prefix` removes support for Rust 1.33 and 1.38
#![allow(clippy::unknown_clippy_lints)]
#![allow(clippy::manual_strip)]
#![allow(clippy::from_str_radix_10)]
// `#[non_exhaustive]` require Rust 1.40+ but procfs minimal Rust version is 1.34
#![allow(clippy::manual_non_exhaustive)]
// Don't throw rustc lint warnings for the deprecated name `intra_doc_link_resolution_failure`.
// The suggested rename to `broken_intra_doc_links` removes support for Rust 1.33 and 1.38.
#![allow(renamed_and_removed_lints)]
#![deny(intra_doc_link_resolution_failure)]
//! This crate provides to an interface into the linux `procfs` filesystem, usually mounted at
//! `/proc`.
//!
//! This is a pseudo-filesystem which is available on most every linux system and provides an
//! interface to kernel data structures.
//!
//!
//! # Kernel support
//!
//! Not all fields/data are available in each kernel.  Some fields were added in specific kernel
//! releases, and other fields are only present in certain kernel configuration options are
//! enabled.  These are represented as `Option` fields in this crate.
//!
//! This crate aims to support all 2.6 kernels (and newer).  WSL2 is also supported.
//!
//! # Documentation
//!
//! In almost all cases, the documentation is taken from the
//! [`proc.5`](http://man7.org/linux/man-pages/man5/proc.5.html) manual page.  This means that
//! sometimes the style of writing is not very "rusty", or may do things like reference related files
//! (instead of referencing related structs).  Contributions to improve this are welcome.
//!
//! # Panicing
//!
//! While previous versions of the library could panic, this current version aims to be panic-free
//! in a many situations as possible.  Whenever the procfs crate encounters a bug in its own
//! parsing code, it will return an [`InternalError`](enum.ProcError.html#variant.InternalError) error.  This should be considered a
//! bug and should be [reported](https://github.com/eminence/procfs).  If you encounter a panic,
//! please report that as well.
//!
//! # Cargo features
//!
//! The following cargo features are available:
//!
//! * `chrono` -- Default.  Optional.  This feature enables a few methods that return values as `DateTime` objects.
//! * `flate2` -- Default.  Optional.  This feature enables parsing gzip compressed `/proc/config.gz` file via the `procfs::kernel_config` method.
//! * `backtrace` -- Optional.  This feature lets you get a stack trace whenever an `InternalError` is raised.
//!
//! # Examples
//!
//! Examples can be found in the various modules shown below, or in the
//! [examples](https://github.com/eminence/procfs/tree/master/examples) folder of the code repository.
//!

use bitflags::bitflags;
use lazy_static::lazy_static;
use libc::pid_t;
use libc::sysconf;
use libc::{_SC_CLK_TCK, _SC_PAGESIZE};

use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::mem;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{collections::HashMap, time::Duration};
use std::{ffi::CStr, fs::OpenOptions};

#[cfg(feature = "chrono")]
use chrono::{DateTime, Local};

const PROC_CONFIG_GZ: &str = "/proc/config.gz";
const BOOT_CONFIG: &str = "/boot/config";

trait IntoOption<T> {
    fn into_option(t: Self) -> Option<T>;
}

impl<T> IntoOption<T> for Option<T> {
    fn into_option(t: Option<T>) -> Option<T> {
        t
    }
}

impl<T, R> IntoOption<T> for Result<T, R> {
    fn into_option(t: Result<T, R>) -> Option<T> {
        t.ok()
    }
}

pub(crate) trait IntoResult<T, E> {
    fn into(t: Self) -> Result<T, E>;
}

macro_rules! build_internal_error {
    ($err: expr) => {
        crate::ProcError::InternalError(crate::InternalError {
            msg: format!("Internal Unwrap Error: {}", $err),
            file: file!(),
            line: line!(),
            #[cfg(feature = "backtrace")]
            backtrace: backtrace::Backtrace::new(),
        })
    };
    ($err: expr, $msg: expr) => {
        crate::ProcError::InternalError(crate::InternalError {
            msg: format!("Internal Unwrap Error: {}: {}", $msg, $err),
            file: file!(),
            line: line!(),
            #[cfg(feature = "backtrace")]
            backtrace: backtrace::Backtrace::new(),
        })
    };
}

// custom NoneError, since std::option::NoneError is nightly-only
// See https://github.com/rust-lang/rust/issues/42327
struct NoneError;

impl std::fmt::Display for NoneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NoneError")
    }
}

impl<T> IntoResult<T, NoneError> for Option<T> {
    fn into(t: Option<T>) -> Result<T, NoneError> {
        t.ok_or(NoneError)
    }
}

impl<T, E> IntoResult<T, E> for Result<T, E> {
    fn into(t: Result<T, E>) -> Result<T, E> {
        t
    }
}

#[allow(unused_macros)]
macro_rules! proc_panic {
    ($e:expr) => {
        crate::IntoOption::into_option($e).unwrap_or_else(|| {
            panic!(
                "Failed to unwrap {}. Please report this as a procfs bug.",
                stringify!($e)
            )
        })
    };
    ($e:expr, $msg:expr) => {
        crate::IntoOption::into_option($e).unwrap_or_else(|| {
            panic!(
                "Failed to unwrap {} ({}). Please report this as a procfs bug.",
                stringify!($e),
                $msg
            )
        })
    };
}

macro_rules! expect {
    ($e:expr) => {
        match crate::IntoResult::into($e) {
            Ok(v) => v,
            Err(e) => return Err(build_internal_error!(e)),
        }
    };
    ($e:expr, $msg:expr) => {
        match crate::IntoResult::into($e) {
            Ok(v) => v,
            Err(e) => return Err(build_internal_error!(e, $msg)),
        }
    };
}

macro_rules! from_str {
    ($t:tt, $e:expr) => {{
        let e = $e;
        expect!(
            $t::from_str_radix(e, 10),
            format!("Failed to parse {} ({:?}) as a {}", stringify!($e), e, stringify!($t),)
        )
    }};
    ($t:tt, $e:expr, $radix:expr) => {{
        let e = $e;
        expect!(
            $t::from_str_radix(e, $radix),
            format!("Failed to parse {} ({:?}) as a {}", stringify!($e), e, stringify!($t))
        )
    }};
    ($t:tt, $e:expr, $radix:expr, pid:$pid:expr) => {{
        let e = $e;
        expect!(
            $t::from_str_radix(e, $radix),
            format!(
                "Failed to parse {} ({:?}) as a {} (pid {})",
                stringify!($e),
                e,
                stringify!($t),
                $pid
            )
        )
    }};
}

macro_rules! wrap_io_error {
    ($path:expr, $expr:expr) => {
        match $expr {
            Ok(v) => Ok(v),
            Err(e) => {
                let kind = e.kind();
                Err(::std::io::Error::new(
                    kind,
                    crate::IoErrorWrapper {
                        path: $path.to_owned(),
                        inner: e.into_inner(),
                    },
                ))
            }
        }
    };
}

pub(crate) fn read_file<P: AsRef<Path>>(path: P) -> ProcResult<String> {
    let mut f = FileWrapper::open(path)?;
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;
    Ok(buf)
}

pub(crate) fn write_file<P: AsRef<Path>, T: AsRef<[u8]>>(path: P, buf: T) -> ProcResult<()> {
    let mut f = OpenOptions::new().read(false).write(true).open(path)?;
    f.write_all(buf.as_ref())?;
    Ok(())
}

pub(crate) fn read_value<P, T, E>(path: P) -> ProcResult<T>
where
    P: AsRef<Path>,
    T: FromStr<Err = E>,
    ProcError: From<E>,
{
    let val = read_file(path)?;
    Ok(<T as FromStr>::from_str(val.trim())?)
    //Ok(val.trim().parse()?)
}

pub(crate) fn write_value<P: AsRef<Path>, T: fmt::Display>(path: P, value: T) -> ProcResult<()> {
    write_file(path, value.to_string().as_bytes())
}

pub(crate) fn from_iter<'a, I, U>(i: I) -> ProcResult<U>
where
    I: IntoIterator<Item = &'a str>,
    U: FromStr,
{
    let mut iter = i.into_iter();
    let val = expect!(iter.next());
    match FromStr::from_str(val) {
        Ok(u) => Ok(u),
        Err(..) => Err(build_internal_error!("Failed to convert")),
    }
}

pub mod process;

mod meminfo;
pub use crate::meminfo::*;

mod sysvipc_shm;
pub use crate::sysvipc_shm::*;

pub mod net;

mod cpuinfo;
pub use crate::cpuinfo::*;

mod cgroups;
pub use crate::cgroups::*;

pub mod sys;
pub use crate::sys::kernel::BuildInfo as KernelBuildInfo;
pub use crate::sys::kernel::Type as KernelType;
pub use crate::sys::kernel::Version as KernelVersion;

mod pressure;
pub use crate::pressure::*;

mod diskstats;
pub use diskstats::*;

mod locks;
pub use locks::*;

pub mod keyring;

mod uptime;
pub use uptime::*;

lazy_static! {
    /// The number of clock ticks per second.
    ///
    /// This is calculated from `sysconf(_SC_CLK_TCK)`.
    static ref TICKS_PER_SECOND: ProcResult<i64> = {
        Ok(ticks_per_second()?)
    };
    /// The version of the currently running kernel.
    ///
    /// This is a lazily constructed static.  You can also get this information via
    /// [KernelVersion::new()].
    static ref KERNEL: ProcResult<KernelVersion> = {
        KernelVersion::current()
    };
    /// Memory page size, in bytes.
    ///
    /// This is calculated from `sysconf(_SC_PAGESIZE)`.
    static ref PAGESIZE: ProcResult<i64> = {
        Ok(page_size()?)
    };
}

fn convert_to_kibibytes(num: u64, unit: &str) -> ProcResult<u64> {
    match unit {
        "B" => Ok(num),
        "KiB" | "kiB" | "kB" | "KB" => Ok(num * 1024),
        "MiB" | "miB" | "MB" | "mB" => Ok(num * 1024 * 1024),
        "GiB" | "giB" | "GB" | "gB" => Ok(num * 1024 * 1024 * 1024),
        unknown => Err(build_internal_error!(format!("Unknown unit type {}", unknown))),
    }
}

trait FromStrRadix: Sized {
    fn from_str_radix(t: &str, radix: u32) -> Result<Self, std::num::ParseIntError>;
}

impl FromStrRadix for u64 {
    fn from_str_radix(s: &str, radix: u32) -> Result<u64, std::num::ParseIntError> {
        u64::from_str_radix(s, radix)
    }
}
impl FromStrRadix for i32 {
    fn from_str_radix(s: &str, radix: u32) -> Result<i32, std::num::ParseIntError> {
        i32::from_str_radix(s, radix)
    }
}

fn split_into_num<T: FromStrRadix>(s: &str, sep: char, radix: u32) -> ProcResult<(T, T)> {
    let mut s = s.split(sep);
    let a = expect!(FromStrRadix::from_str_radix(expect!(s.next()), radix));
    let b = expect!(FromStrRadix::from_str_radix(expect!(s.next()), radix));
    Ok((a, b))
}

/// This is used to hold both an IO error as well as the path of the file that originated the error
#[derive(Debug)]
struct IoErrorWrapper {
    path: PathBuf,
    inner: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl std::error::Error for IoErrorWrapper {}
impl fmt::Display for IoErrorWrapper {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if let Some(inner) = &self.inner {
            write!(f, "IO Error({}): {}", self.path.display(), inner)
        } else {
            write!(f, "IO Error({})", self.path.display())
        }
    }
}

/// A wrapper around a `File` that remembers the name of the path
struct FileWrapper {
    inner: File,
    path: PathBuf,
}

impl FileWrapper {
    fn open<P: AsRef<Path>>(path: P) -> Result<FileWrapper, io::Error> {
        let p = path.as_ref();
        match File::open(&p) {
            Ok(f) => Ok(FileWrapper {
                inner: f,
                path: p.to_owned(),
            }),
            Err(e) => {
                let kind = e.kind();
                Err(io::Error::new(
                    kind,
                    IoErrorWrapper {
                        path: p.to_owned(),
                        inner: e.into_inner(),
                    },
                ))
            }
        }
    }
}

impl Read for FileWrapper {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        wrap_io_error!(self.path, self.inner.read(buf))
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        wrap_io_error!(self.path, self.inner.read_to_end(buf))
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        wrap_io_error!(self.path, self.inner.read_to_string(buf))
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        wrap_io_error!(self.path, self.inner.read_exact(buf))
    }
}

/// The main error type for the procfs crate.
///
/// For more info, see the [ProcError] type.
pub type ProcResult<T> = Result<T, ProcError>;

/// The various error conditions in the procfs crate.
///
/// Most of the variants have an `Option<PathBuf>` component.  If the error root cause was related
/// to some operation on a file, the path of this file will be stored in this component.
#[derive(Debug)]
pub enum ProcError {
    /// A standard permission denied error.
    ///
    /// This will be a common error, since some files in the procfs filesystem are only readable by
    /// the root user.
    PermissionDenied(Option<PathBuf>),
    /// This might mean that the process no longer exists, or that your kernel doesn't support the
    /// feature you are trying to use.
    NotFound(Option<PathBuf>),
    /// This might mean that a procfs file has incomplete contents.
    ///
    /// If you encounter this error, consider retrying the operation.
    Incomplete(Option<PathBuf>),
    /// Any other IO error (rare).
    Io(std::io::Error, Option<PathBuf>),
    /// Any other non-IO error (very rare).
    Other(String),
    /// This error indicates that some unexpected error occurred.  This is a bug.  The inner
    /// [InternalError] struct will contain some more info.
    ///
    /// If you ever encounter this error, consider it a bug in the procfs crate and please report
    /// it on github.
    InternalError(InternalError),
}

/// An internal error in the procfs crate
///
/// If you encounter this error, consider it a bug and please report it on
/// [github](https://github.com/eminence/procfs).
///
/// If you compile with the optional `backtrace` feature (disabled by default),
/// you can gain access to a stack trace of where the error happened.
pub struct InternalError {
    pub msg: String,
    pub file: &'static str,
    pub line: u32,
    #[cfg(feature = "backtrace")]
    pub backtrace: backtrace::Backtrace,
}

impl std::fmt::Debug for InternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "bug at {}:{} (please report this procfs bug)\n{}",
            self.file, self.line, self.msg
        )
    }
}

impl std::fmt::Display for InternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "bug at {}:{} (please report this procfs bug)\n{}",
            self.file, self.line, self.msg
        )
    }
}

impl From<std::io::Error> for ProcError {
    fn from(io: std::io::Error) -> Self {
        use std::io::ErrorKind;
        let kind = io.kind();
        let path: Option<PathBuf> = io
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<IoErrorWrapper>().map(|inner| inner.path.clone()));
        match kind {
            ErrorKind::PermissionDenied => ProcError::PermissionDenied(path),
            ErrorKind::NotFound => ProcError::NotFound(path),
            _other => ProcError::Io(io, path),
        }
    }
}

impl From<&'static str> for ProcError {
    fn from(val: &'static str) -> Self {
        ProcError::Other(val.to_owned())
    }
}

impl From<std::num::ParseIntError> for ProcError {
    fn from(val: std::num::ParseIntError) -> Self {
        ProcError::Other(format!("ParseIntError: {}", val))
    }
}

impl From<std::string::ParseError> for ProcError {
    fn from(e: std::string::ParseError) -> Self {
        match e {}
    }
}

impl std::fmt::Display for ProcError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            // Variants with paths:
            ProcError::PermissionDenied(Some(p)) => write!(f, "Permission Denied: {}", p.display()),
            ProcError::NotFound(Some(p)) => write!(f, "File not found: {}", p.display()),
            ProcError::Incomplete(Some(p)) => write!(f, "Data incomplete: {}", p.display()),
            ProcError::Io(inner, Some(p)) => {
                write!(f, "Unexpected IO error({}): {}", p.display(), inner)
            }
            // Variants without paths:
            ProcError::PermissionDenied(None) => write!(f, "Permission Denied"),
            ProcError::NotFound(None) => write!(f, "File not found"),
            ProcError::Incomplete(None) => write!(f, "Data incomplete"),
            ProcError::Io(inner, None) => write!(f, "Unexpected IO error: {}", inner),

            ProcError::Other(s) => write!(f, "Unknown error {}", s),
            ProcError::InternalError(e) => write!(f, "Internal error: {}", e),
        }
    }
}

impl std::error::Error for ProcError {}

/// Load average figures.
///
/// Load averages are calculated as the number of jobs in the run queue (state R) or waiting for
/// disk I/O (state D) averaged over 1, 5, and 15 minutes.
#[derive(Debug, Clone)]
pub struct LoadAverage {
    /// The one-minute load average
    pub one: f32,
    /// The five-minute load average
    pub five: f32,
    /// The fifteen-minute load average
    pub fifteen: f32,
    /// The number of currently runnable kernel scheduling  entities  (processes,  threads).
    pub cur: u32,
    /// The number of kernel scheduling entities that currently exist on the system.
    pub max: u32,
    /// The fifth field is the PID of the process that was most recently created on the system.
    pub latest_pid: u32,
}

impl LoadAverage {
    /// Reads load average info from `/proc/loadavg`
    pub fn new() -> ProcResult<LoadAverage> {
        let mut f = FileWrapper::open("/proc/loadavg")?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        let mut s = s.split_whitespace();

        let one = expect!(f32::from_str(expect!(s.next())));
        let five = expect!(f32::from_str(expect!(s.next())));
        let fifteen = expect!(f32::from_str(expect!(s.next())));
        let curmax = expect!(s.next());
        let latest_pid = expect!(u32::from_str(expect!(s.next())));

        let mut s = curmax.split('/');
        let cur = expect!(u32::from_str(expect!(s.next())));
        let max = expect!(u32::from_str(expect!(s.next())));

        Ok(LoadAverage {
            one,
            five,
            fifteen,
            cur,
            max,
            latest_pid,
        })
    }
}

/// Return the number of ticks per second.
///
/// This isn't part of the proc file system, but it's a useful thing to have, since several fields
/// count in ticks.  This is calculated from `sysconf(_SC_CLK_TCK)`.
pub fn ticks_per_second() -> std::io::Result<i64> {
    if cfg!(unix) {
        match unsafe { sysconf(_SC_CLK_TCK) } {
            -1 => Err(std::io::Error::last_os_error()),
            #[cfg(target_pointer_width = "64")]
            x => Ok(x),
            #[cfg(target_pointer_width = "32")]
            x => Ok(x.into())
        }
    } else {
        panic!("Not supported on non-unix platforms")
    }
}

/// The boot time of the system, as a `DateTime` object.
///
/// This is calculated from `/proc/stat`.
///
/// This function requires the "chrono" features to be enabled (which it is by default).
#[cfg(feature = "chrono")]
pub fn boot_time() -> ProcResult<DateTime<Local>> {
    use chrono::TimeZone;
    let secs = boot_time_secs()?;

    Ok(chrono::Local.timestamp(secs as i64, 0))
}

/// The boottime of the system, in seconds since the epoch
///
/// This is calculated from `/proc/stat`.
///
#[cfg_attr(
    not(feature = "chrono"),
    doc = "If you compile with the optional `chrono` feature, you can use the `boot_time()` method to get the boot time as a `DateTime` object."
)]
#[cfg_attr(
    feature = "chrono",
    doc = "See also [boot_time()] to get the boot time as a `DateTime`"
)]
pub fn boot_time_secs() -> ProcResult<u64> {
    BOOT_TIME.with(|x| {
        let mut btime = x.borrow_mut();
        if let Some(btime) = *btime {
            Ok(btime)
        } else {
            let stat = KernelStats::new()?;
            *btime = Some(stat.btime);
            Ok(stat.btime)
        }
    })
}

thread_local! {
    static BOOT_TIME : std::cell::RefCell<Option<u64>> = std::cell::RefCell::new(None);
}

/// Memory page size, in bytes.
///
/// This is calculated from `sysconf(_SC_PAGESIZE)`.
pub fn page_size() -> std::io::Result<i64> {
    if cfg!(unix) {
        match unsafe { sysconf(_SC_PAGESIZE) } {
            -1 => Err(std::io::Error::last_os_error()),
            #[cfg(target_pointer_width = "64")]
            x => Ok(x),
            #[cfg(target_pointer_width = "32")]
            x => Ok(x.into())        }
    } else {
        panic!("Not supported on non-unix platforms")
    }
}

/// Possible values for a kernel config option
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSetting {
    Yes,
    Module,
    Value(String),
}
/// Returns a configuration options used to build the currently running kernel
///
/// If CONFIG_KCONFIG_PROC is available, the config is read from `/proc/config.gz`.
/// Else look in `/boot/config-$(uname -r)` or `/boot/config` (in that order).
///
/// # Notes
/// Reading the compress `/proc/config.gz` is only supported if the `flate2` feature is enabled
/// (which it is by default).
#[cfg_attr(feature = "flate2", doc = "The flate2 feature is currently enabled")]
#[cfg_attr(not(feature = "flate2"), doc = "The flate2 feature is NOT currently enabled")]
pub fn kernel_config() -> ProcResult<HashMap<String, ConfigSetting>> {
    let reader: Box<dyn BufRead> = if Path::new(PROC_CONFIG_GZ).exists() && cfg!(feature = "flate2") {
        #[cfg(feature = "flate2")]
        {
            let file = FileWrapper::open(PROC_CONFIG_GZ)?;
            let decoder = flate2::read::GzDecoder::new(file);
            Box::new(BufReader::new(decoder))
        }
        #[cfg(not(feature = "flate2"))]
        {
            unreachable!("flate2 feature not enabled")
        }
    } else {
        let mut kernel: libc::utsname = unsafe { mem::zeroed() };

        if unsafe { libc::uname(&mut kernel) != 0 } {
            return Err(ProcError::Other("Failed to call uname()".to_string()));
        }

        let filename = format!(
            "{}-{}",
            BOOT_CONFIG,
            unsafe { CStr::from_ptr(kernel.release.as_ptr() as *const c_char) }.to_string_lossy()
        );

        if Path::new(&filename).exists() {
            let file = FileWrapper::open(filename)?;
            Box::new(BufReader::new(file))
        } else {
            let file = FileWrapper::open(BOOT_CONFIG)?;
            Box::new(BufReader::new(file))
        }
    };

    let mut map = HashMap::new();

    for line in reader.lines() {
        let line = line?;
        if line.starts_with('#') {
            continue;
        }
        if line.contains('=') {
            let mut s = line.splitn(2, '=');
            let name = expect!(s.next()).to_owned();
            let value = match expect!(s.next()) {
                "y" => ConfigSetting::Yes,
                "m" => ConfigSetting::Module,
                s => ConfigSetting::Value(s.to_owned()),
            };
            map.insert(name, value);
        }
    }

    Ok(map)
}

/// The amount of time, measured in ticks, the CPU has been in specific states
///
/// These fields are measured in ticks because the underlying data from the kernel is measured in ticks.
/// The number of ticks per second can be returned by [`ticks_per_second()`](crate::ticks_per_second)
/// and is generally 100 on most systems.

/// To convert this value to seconds, you can divide by the tps.  There are also convenience methods
/// that you can use too.
#[derive(Debug, Clone)]
pub struct CpuTime {
    /// Ticks spent in user mode
    pub user: u64,
    /// Ticks spent in user mode with low priority (nice)
    pub nice: u64,
    /// Ticks spent in system mode
    pub system: u64,
    /// Ticks spent in the idle state
    pub idle: u64,
    /// Ticks waiting for I/O to complete
    ///
    /// This value is not reliable, for the following reasons:
    ///
    /// 1. The CPU will not wait for I/O to complete; iowait is the time that a
    ///    task is waiting for I/O to complete.  When a CPU goes into idle state
    ///    for outstanding task I/O, another task will be scheduled on this CPU.
    ///
    /// 2. On a multi-core CPU, this task waiting for I/O to complete is not running
    ///    on any CPU, so the iowait for each CPU is difficult to calculate.
    ///
    /// 3. The value in this field may *decrease* in certain conditions.
    ///
    /// (Since Linux 2.5.41)
    pub iowait: Option<u64>,
    /// Ticks servicing interrupts
    ///
    /// (Since Linux 2.6.0)
    pub irq: Option<u64>,
    /// Ticks servicing softirqs
    ///
    /// (Since Linux 2.6.0)
    pub softirq: Option<u64>,
    /// Ticks of stolen time.
    ///
    /// Stolen time is the time spent in other operating systems when running in
    /// a virtualized environment.
    ///
    /// (Since Linux 2.6.11)
    pub steal: Option<u64>,
    /// Ticks spent running a virtual CPU for guest operating systems under control
    /// of the linux kernel
    ///
    /// (Since Linux 2.6.24)
    pub guest: Option<u64>,
    /// Ticks spent running a niced guest
    ///
    /// (Since Linux 2.6.33)
    pub guest_nice: Option<u64>,

    tps: u64,
}

impl CpuTime {
    fn from_str(s: &str) -> ProcResult<CpuTime> {
        let mut s = s.split_whitespace();

        // Store this field in the struct so we don't have to attempt to unwrap ticks_per_second() when we convert
        // from ticks into other time units
        let tps = crate::ticks_per_second()? as u64;

        s.next();
        let user = from_str!(u64, expect!(s.next()));
        let nice = from_str!(u64, expect!(s.next()));
        let system = from_str!(u64, expect!(s.next()));
        let idle = from_str!(u64, expect!(s.next()));

        let iowait = s.next().map(|s| Ok(from_str!(u64, s))).transpose()?;
        let irq = s.next().map(|s| Ok(from_str!(u64, s))).transpose()?;
        let softirq = s.next().map(|s| Ok(from_str!(u64, s))).transpose()?;
        let steal = s.next().map(|s| Ok(from_str!(u64, s))).transpose()?;
        let guest = s.next().map(|s| Ok(from_str!(u64, s))).transpose()?;
        let guest_nice = s.next().map(|s| Ok(from_str!(u64, s))).transpose()?;

        Ok(CpuTime {
            user,
            nice,
            system,
            idle,
            iowait,
            irq,
            softirq,
            steal,
            guest,
            guest_nice,
            tps,
        })
    }

    /// Milliseconds spent in user mode
    pub fn user_ms(&self) -> u64 {
        let ms_per_tick = 1000 / self.tps;
        self.user * ms_per_tick
    }

    /// Time spent in user mode
    pub fn user_duration(&self) -> Duration {
        Duration::from_millis(self.user_ms())
    }

    /// Milliseconds spent in user mode with low priority (nice)
    pub fn nice_ms(&self) -> u64 {
        let ms_per_tick = 1000 / self.tps;
        self.nice * ms_per_tick
    }

    /// Time spent in user mode with low priority (nice)
    pub fn nice_duration(&self) -> Duration {
        Duration::from_millis(self.nice_ms())
    }

    /// Milliseconds spent in system mode
    pub fn system_ms(&self) -> u64 {
        let ms_per_tick = 1000 / self.tps;
        self.system * ms_per_tick
    }

    /// Time spent in system mode
    pub fn system_duration(&self) -> Duration {
        Duration::from_millis(self.system_ms())
    }

    /// Milliseconds spent in the idle state
    pub fn idle_ms(&self) -> u64 {
        let ms_per_tick = 1000 / self.tps;
        self.idle * ms_per_tick
    }

    /// Time spent in the idle state
    pub fn idle_duration(&self) -> Duration {
        Duration::from_millis(self.idle_ms())
    }

    /// Milliseconds spent waiting for I/O to complete
    pub fn iowait_ms(&self) -> Option<u64> {
        let ms_per_tick = 1000 / self.tps;
        self.iowait.map(|io| io * ms_per_tick)
    }

    /// Time spent waiting for I/O to complete
    pub fn iowait_duration(&self) -> Option<Duration> {
        self.iowait_ms().map(Duration::from_millis)
    }

    /// Milliseconds spent servicing interrupts
    pub fn irq_ms(&self) -> Option<u64> {
        let ms_per_tick = 1000 / self.tps;
        self.irq.map(|ms| ms * ms_per_tick)
    }

    /// Time spent servicing interrupts
    pub fn irq_duration(&self) -> Option<Duration> {
        self.irq_ms().map(Duration::from_millis)
    }

    /// Milliseconds spent servicing softirqs
    pub fn softirq_ms(&self) -> Option<u64> {
        let ms_per_tick = 1000 / self.tps;
        self.softirq.map(|ms| ms * ms_per_tick)
    }

    /// Time spent servicing softirqs
    pub fn softirq_duration(&self) -> Option<Duration> {
        self.softirq_ms().map(Duration::from_millis)
    }

    /// Milliseconds of stolen time
    pub fn steal_ms(&self) -> Option<u64> {
        let ms_per_tick = 1000 / self.tps;
        self.steal.map(|ms| ms * ms_per_tick)
    }

    /// Amount of stolen time
    pub fn steal_duration(&self) -> Option<Duration> {
        self.steal_ms().map(Duration::from_millis)
    }

    /// Milliseconds spent running a virtual CPU for guest operating systems under control of the linux kernel
    pub fn guest_ms(&self) -> Option<u64> {
        let ms_per_tick = 1000 / self.tps;
        self.guest.map(|ms| ms * ms_per_tick)
    }

    /// Time spent running a virtual CPU for guest operating systems under control of the linux kernel
    pub fn guest_duration(&self) -> Option<Duration> {
        self.guest_ms().map(Duration::from_millis)
    }

    /// Milliseconds spent running a niced guest
    pub fn guest_nice_ms(&self) -> Option<u64> {
        let ms_per_tick = 1000 / self.tps;
        self.guest_nice.map(|ms| ms * ms_per_tick)
    }

    /// Time spent running a niced guest
    pub fn guest_nice_duration(&self) -> Option<Duration> {
        self.guest_nice_ms().map(Duration::from_millis)
    }
}

/// Kernel/system statistics, from `/proc/stat`
#[derive(Debug, Clone)]
pub struct KernelStats {
    /// The amount of time the system spent in various states
    pub total: CpuTime,
    /// The amount of time that specific CPUs spent in various states
    pub cpu_time: Vec<CpuTime>,

    /// The number of context switches that the system underwent
    pub ctxt: u64,

    /// Boot time, in number of seconds since the Epoch
    pub btime: u64,

    /// Number of forks since boot
    pub processes: u64,

    /// Number of processes in runnable state
    ///
    /// (Since Linux 2.5.45)
    pub procs_running: Option<u32>,

    /// Number of processes blocked waiting for I/O
    ///
    /// (Since Linux 2.5.45)
    pub procs_blocked: Option<u32>,
}

impl KernelStats {
    pub fn new() -> ProcResult<KernelStats> {
        KernelStats::from_reader(FileWrapper::open("/proc/stat")?)
    }
    /// Get KernelStatus from a custom Read instead of the default `/proc/stat`.
    pub fn from_reader<R: io::Read>(r: R) -> ProcResult<KernelStats> {
        let bufread = BufReader::new(r);
        let lines = bufread.lines();

        let mut total_cpu = None;
        let mut cpus = Vec::new();
        let mut ctxt = None;
        let mut btime = None;
        let mut processes = None;
        let mut procs_running = None;
        let mut procs_blocked = None;

        for line in lines {
            let line = line?;
            if line.starts_with("cpu ") {
                total_cpu = Some(CpuTime::from_str(&line)?);
            } else if line.starts_with("cpu") {
                cpus.push(CpuTime::from_str(&line)?);
            } else if line.starts_with("ctxt ") {
                ctxt = Some(from_str!(u64, &line[5..]));
            } else if line.starts_with("btime ") {
                btime = Some(from_str!(u64, &line[6..]));
            } else if line.starts_with("processes ") {
                processes = Some(from_str!(u64, &line[10..]));
            } else if line.starts_with("procs_running ") {
                procs_running = Some(from_str!(u32, &line[14..]));
            } else if line.starts_with("procs_blocked ") {
                procs_blocked = Some(from_str!(u32, &line[14..]));
            }
        }

        Ok(KernelStats {
            total: expect!(total_cpu),
            cpu_time: cpus,
            ctxt: expect!(ctxt),
            btime: expect!(btime),
            processes: expect!(processes),
            procs_running,
            procs_blocked,
        })
    }
}

/// Get various virtual memory statistics
///
/// Since the exact set of statistics will vary from kernel to kernel,
/// and because most of them are not well documented, this function
/// returns a HashMap instead of a struct.  Consult the kernel source
/// code for more details of this data.
///
/// This data is taken from the `/proc/vmstat` file.
///
/// (since Linux 2.6.0)
pub fn vmstat() -> ProcResult<HashMap<String, i64>> {
    let file = FileWrapper::open("/proc/vmstat")?;
    let reader = BufReader::new(file);
    let mut map = HashMap::new();
    for line in reader.lines() {
        let line = line?;
        let mut split = line.split_whitespace();
        let name = expect!(split.next());
        let val = from_str!(i64, expect!(split.next()));
        map.insert(name.to_owned(), val);
    }

    Ok(map)
}

/// Details about a loaded kernel module
///
/// For an example, see the [lsmod.rs](https://github.com/eminence/procfs/tree/master/examples)
/// example in the source repo.
#[derive(Debug, Clone)]
pub struct KernelModule {
    /// The name of the module
    pub name: String,

    /// The size of the module
    pub size: u32,

    /// The number of references in the kernel to this module.  This can be -1 if the module is unloading
    pub refcount: i32,

    /// A list of modules that depend on this module.
    pub used_by: Vec<String>,

    /// The module state
    ///
    /// This will probably always be "Live", but it could also be either "Unloading" or "Loading"
    pub state: String,
}

/// Get a list of loaded kernel modules
///
/// This corresponds to the data in `/proc/modules`.
pub fn modules() -> ProcResult<HashMap<String, KernelModule>> {
    // kernel reference: kernel/module.c m_show()

    let mut map = HashMap::new();
    let file = FileWrapper::open("/proc/modules")?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line: String = line?;
        let mut s = line.split_whitespace();
        let name = expect!(s.next());
        let size = from_str!(u32, expect!(s.next()));
        let refcount = from_str!(i32, expect!(s.next()));
        let used_by: &str = expect!(s.next());
        let state = expect!(s.next());

        map.insert(
            name.to_string(),
            KernelModule {
                name: name.to_string(),
                size,
                refcount,
                used_by: if used_by == "-" {
                    Vec::new()
                } else {
                    used_by
                        .split(',')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect()
                },
                state: state.to_string(),
            },
        );
    }

    Ok(map)
}

/// Get a list of the arguments passed to the Linux kernel at boot time
///
/// This corresponds to the data in `/proc/cmdline`
pub fn cmdline() -> ProcResult<Vec<String>> {
    let mut buf = String::new();
    let mut f = FileWrapper::open("/proc/cmdline")?;
    f.read_to_string(&mut buf)?;
    Ok(buf
        .split(' ')
        .filter_map(|s| if !s.is_empty() { Some(s.to_string()) } else { None })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statics() {
        println!("{:?}", *TICKS_PER_SECOND);
        println!("{:?}", *KERNEL);
        println!("{:?}", *PAGESIZE);
    }

    #[test]
    fn test_kernel_from_str() {
        let k = KernelVersion::from_str("1.2.3").unwrap();
        assert_eq!(k.major, 1);
        assert_eq!(k.minor, 2);
        assert_eq!(k.patch, 3);

        let k = KernelVersion::from_str("4.9.16-gentoo").unwrap();
        assert_eq!(k.major, 4);
        assert_eq!(k.minor, 9);
        assert_eq!(k.patch, 16);

        let k = KernelVersion::from_str("4.9.266-0.1.ac.225.84.332.metal1.x86_64").unwrap();
        assert_eq!(k.major, 4);
        assert_eq!(k.minor, 9);
        assert_eq!(k.patch, 266);
    }

    #[test]
    fn test_kernel_cmp() {
        let a = KernelVersion::from_str("1.2.3").unwrap();
        let b = KernelVersion::from_str("1.2.3").unwrap();
        let c = KernelVersion::from_str("1.2.4").unwrap();
        let d = KernelVersion::from_str("1.5.4").unwrap();
        let e = KernelVersion::from_str("2.5.4").unwrap();

        assert_eq!(a, b);
        assert!(a < c);
        assert!(a < d);
        assert!(a < e);
        assert!(e > d);
        assert!(e > c);
        assert!(e > b);
    }

    #[test]
    fn test_loadavg() {
        let load = LoadAverage::new().unwrap();
        println!("{:?}", load);
    }

    #[test]
    fn test_from_str() -> ProcResult<()> {
        assert_eq!(from_str!(u8, "12"), 12);
        assert_eq!(from_str!(u8, "A", 16), 10);
        Ok(())
    }

    #[test]
    fn test_from_str_fail() {
        fn inner() -> ProcResult<()> {
            let s = "four";
            from_str!(u8, s);
            unreachable!()
        }

        assert!(inner().is_err())
    }

    #[test]
    fn test_kernel_config() {
        // TRAVIS
        // we don't have access to the kernel_config on travis, so skip that test there
        match std::env::var("TRAVIS") {
            Ok(ref s) if s == "true" => return,
            _ => {}
        }
        if !Path::new(PROC_CONFIG_GZ).exists() && !Path::new(BOOT_CONFIG).exists() {
            return;
        }

        let config = kernel_config().unwrap();
        println!("{:#?}", config);
    }

    #[test]
    fn test_file_io_errors() {
        fn inner<P: AsRef<Path>>(p: P) -> Result<(), ProcError> {
            let mut file = FileWrapper::open(p)?;

            let mut buf = [0; 128];
            file.read_exact(&mut buf[0..128])?;

            Ok(())
        }

        let err = inner("/this_should_not_exist").unwrap_err();
        println!("{}", err);

        match err {
            ProcError::NotFound(Some(p)) => {
                assert_eq!(p, Path::new("/this_should_not_exist"));
            }
            x => panic!("Unexpected return value: {:?}", x),
        }

        match inner("/proc/loadavg") {
            Err(ProcError::Io(_, Some(p))) => {
                assert_eq!(p, Path::new("/proc/loadavg"));
            }
            x => panic!("Unexpected return value: {:?}", x),
        }
    }

    #[test]
    fn test_nopanic() {
        fn _inner() -> ProcResult<bool> {
            let x: Option<bool> = None;
            let y: bool = expect!(x);
            Ok(y)
        }

        let r = _inner();
        println!("{:?}", r);
        assert!(r.is_err());

        fn _inner2() -> ProcResult<bool> {
            let _f: std::fs::File = expect!(std::fs::File::open("/doesnotexist"));
            Ok(true)
        }

        let r = _inner2();
        println!("{:?}", r);
        assert!(r.is_err());
    }

    #[cfg(feature = "backtrace")]
    #[test]
    fn test_backtrace() {
        fn _inner() -> ProcResult<bool> {
            let _f: std::fs::File = expect!(std::fs::File::open("/doesnotexist"));
            Ok(true)
        }

        let r = _inner();
        println!("{:?}", r);
    }

    #[test]
    fn test_kernel_stat() {
        let stat = KernelStats::new().unwrap();
        println!("{:#?}", stat);

        // the boottime from KernelStats should match the boottime from /proc/uptime
        let boottime = boot_time_secs().unwrap();

        let diff = (boottime as i32 - stat.btime as i32).abs();
        assert!(diff <= 1);

        let cpuinfo = CpuInfo::new().unwrap();
        assert_eq!(cpuinfo.num_cores(), stat.cpu_time.len());

        // the sum of each individual CPU should be equal to the total cpu entry
        // note: on big machines with 128 cores, it seems that the differences can be rather high,
        // especially when heavily loaded.  So this test tolerates a 6000-tick discrepancy
        // (60 seconds in a 100-tick-per-second kernel)

        let user: u64 = stat.cpu_time.iter().map(|i| i.user).sum();
        let nice: u64 = stat.cpu_time.iter().map(|i| i.nice).sum();
        let system: u64 = stat.cpu_time.iter().map(|i| i.system).sum();
        assert!(
            (stat.total.user as i64 - user as i64).abs() < 6000,
            "sum:{} total:{} diff:{}",
            stat.total.user,
            user,
            stat.total.user - user
        );
        assert!(
            (stat.total.nice as i64 - nice as i64).abs() < 6000,
            "sum:{} total:{} diff:{}",
            stat.total.nice,
            nice,
            stat.total.nice - nice
        );
        assert!(
            (stat.total.system as i64 - system as i64).abs() < 6000,
            "sum:{} total:{} diff:{}",
            stat.total.system,
            system,
            stat.total.system - system
        );

        let diff = stat.total.idle as i64 - (stat.cpu_time.iter().map(|i| i.idle).sum::<u64>() as i64).abs();
        assert!(diff < 1000, "idle time difference too high: {}", diff);
    }

    #[test]
    fn test_vmstat() {
        let stat = vmstat().unwrap();
        println!("{:?}", stat);
    }

    #[test]
    fn test_modules() {
        let mods = modules().unwrap();
        for module in mods.values() {
            println!("{:?}", module);
        }
    }

    #[test]
    fn tests_tps() {
        let tps = ticks_per_second().unwrap();
        println!("{} ticks per second", tps);
    }

    #[test]
    fn test_cmdline() {
        let cmdline = cmdline().unwrap();

        for argument in cmdline {
            println!("{}", argument);
        }
    }
}
