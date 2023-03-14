#![deny(unsafe_code)]

use std::error::Error;
use std::fmt;
use std::io;
use std::str::FromStr;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
use libc::__rlimit_resource_t as resource_t;

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
use libc::c_int as resource_t;

/// Integer type used for resource values.
///
/// The actual type of [`RawResource`][RawResource] can be different on different platforms.
///
/// [RawResource]: type.RawResource.html
#[allow(clippy::module_name_repetitions)]
pub type RawResource = resource_t;

/// A kind of resource.
///
/// **Be careful**: The documentation of [`Resource`][Resource] constants are based on a few systems.
/// It may be inconsistent with other platforms.
///
/// # References
/// Linux: <https://man7.org/linux/man-pages/man2/getrlimit.2.html>
///
/// FreeBSD: <https://www.freebsd.org/cgi/man.cgi?query=getrlimit>
///
/// NetBSD: <https://man.netbsd.org/getrlimit.2>
///
/// [Resource]: struct.Resource.html
///
#[allow(clippy::doc_markdown)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Resource {
    tag: u16,
    value: u16,
}

impl fmt::Debug for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let idx = Self::VALUE_TABLE.iter().position(|v| v == self).unwrap();
        write!(f, "Resource::{}", Self::IDENT_TABLE[idx])
    }
}

impl FromStr for Resource {
    type Err = ParseResourceError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pos = Self::NAME_TABLE.iter().position(|&name| s == name);
        match pos {
            Some(idx) => Ok(Self::VALUE_TABLE[idx]),
            None => Err(ParseResourceError { _priv: () }),
        }
    }
}

/// An error returned when parsing a `Resource` using [`from_str`] fails
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResourceError {
    /// private place holder
    _priv: (),
}

impl fmt::Display for ParseResourceError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to parse Resource")
    }
}

impl Error for ParseResourceError {}

macro_rules! declare_resource {
    {$($(#[$attr:meta])* $id:ident = $tag:expr => $c_enum:ident,)+} => {
        impl Resource{
            $(
                $(#[$attr])*
                pub const $id: Self = Self{ tag: $tag, value: libc::$c_enum as u16 };
            )+
        }

        #[allow(unused_doc_comments)]
        impl Resource{
            const NAME_TABLE: &'static [&'static str] = &[
                $(
                    $(#[$attr])*
                    {
                        stringify!($c_enum)
                    },
                )+
            ];

            const VALUE_TABLE: &'static [Self] = &[
                $(
                    $(#[$attr])*
                    {
                        Self::$id
                    },
                )+
            ];

            const IDENT_TABLE: &'static [&'static str] = &[
                $(
                    $(#[$attr])*
                    {
                        stringify!($id)
                    },
                )+
            ];
        }

        #[cfg(test)]
        mod tests{
            use super::*;

            #[allow(unused_comparisons)]
            #[allow(unused_doc_comments)]
            #[test]
            fn name_value(){
                $(
                    $(#[$attr])*
                    {
                        assert_eq!(Resource::$id.as_name(), stringify!($c_enum));
                        assert_eq!(Resource::from_str(stringify!($c_enum)).unwrap(), Resource::$id);
                        assert!((0..=128).contains(&libc::$c_enum));
                    }
                )+
            }

            #[allow(unused_doc_comments)]
            #[test]
            fn unique_tag(){
                use std::collections::HashSet;

                let tags = [
                    $(
                        $(#[$attr])*
                        { $tag },
                    )+
                ];

                let s: HashSet<u16> = tags.iter().copied().collect();
                assert_eq!(s.len(), Resource::NAME_TABLE.len());
            }

            #[allow(unused_doc_comments)]
            #[test]
            fn raw_eq(){
                $(
                    $(#[$attr])*
                    {
                        assert_eq!(Resource::$id.as_raw(), libc::$c_enum);
                    }
                )+
            }

            #[allow(unused_doc_comments)]
            #[test]
            fn from_str(){
                $(
                    $(#[$attr])*
                    {
                        assert_eq!(Resource::from_str(stringify!($c_enum)), Ok(Resource::$id));
                    }
                )+

                assert!(Resource::from_str("asdqwe").is_err());
            }

            #[test]
            fn available(){
                assert_eq!(
                    Resource::available_names().len(),
                    Resource::available_resources().len()
                );
            }
        }
    };
}

impl Resource {
    /// Set resource limits.
    /// # Errors
    /// See [`setrlimit`](fn.setrlimit.html)
    #[inline]
    pub fn set(self, soft: u64, hard: u64) -> io::Result<()> {
        super::setrlimit(self, soft, hard)
    }

    /// Get resource limits.
    /// # Errors
    /// See [`getrlimit`](fn.getrlimit.html)
    #[inline]
    pub fn get(self) -> io::Result<(u64, u64)> {
        super::getrlimit(self)
    }

    /// Returns the name of the resource.
    ///
    /// # Example
    /// ```
    /// # use rlimit::Resource;
    /// assert_eq!(Resource::NOFILE.as_name(), "RLIMIT_NOFILE");
    /// ```
    #[must_use]
    #[allow(clippy::missing_panics_doc)] // this method should never panic
    pub fn as_name(self) -> &'static str {
        let idx = Self::VALUE_TABLE.iter().position(|&v| v == self).unwrap();
        Self::NAME_TABLE[idx]
    }

    /// Returns available resource names.
    #[must_use]
    pub const fn available_names() -> &'static [&'static str] {
        Self::NAME_TABLE
    }

    /// Returns available resources.
    #[must_use]
    pub const fn available_resources() -> &'static [Self] {
        Self::VALUE_TABLE
    }

    /// Returns the raw resource type.
    ///
    /// **Be careful**: The actual type of [`RawResource`][RawResource] can be different on different platforms.
    ///
    /// [RawResource]: type.RawResource.html
    #[inline]
    #[must_use]
    pub const fn as_raw(self) -> RawResource {
        self.value as _
    }
}

// #begin-codegen
// generated from rust-lang/libc ec88c377ab1695d7bdd721332382e7cecc07b7e3
declare_resource! {

    /// The maximum size (in bytes)
    /// of the process's virtual memory (address space).
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl", any(target_arch = "x86", target_arch = "mips", target_arch = "powerpc", target_arch = "hexagon", target_arch = "arm")),
        all(target_os = "linux", target_env = "musl", any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "mips64", target_arch = "powerpc64")),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "netbsd",
        target_os = "solarish",
    ))]
    AS = 1 => RLIMIT_AS,


    /// The maximum size (in bytes)
    /// of a core file that the process may dump.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "solarish",
    ))]
    CORE = 2 => RLIMIT_CORE,


    /// A limit (in seconds)
    /// on the amount of CPU time that the process can consume.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "solarish",
    ))]
    CPU = 3 => RLIMIT_CPU,


    /// The maximum size (in bytes)
    /// of the process's data segment
    /// (initialized data, uninitialized data, and heap).
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "solarish",
    ))]
    DATA = 4 => RLIMIT_DATA,


    /// The maximum size (in bytes)
    /// of files that the process may create.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "solarish",
    ))]
    FSIZE = 5 => RLIMIT_FSIZE,


    /// The maximum number of kqueues this user id is allowed to create.
    #[cfg(any(
        target_os = "freebsd",
    ))]
    KQUEUES = 6 => RLIMIT_KQUEUES,


    /// (early Linux 2.4 only)
    ///
    /// A limit on the combined number
    /// of `flock(2)` locks and `fcntl(2)` leases
    /// that this process may establish.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
    ))]
    LOCKS = 7 => RLIMIT_LOCKS,


    /// The maximum number (in bytes)
    /// of memory that may be locked into RAM.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl", any(target_arch = "x86", target_arch = "mips", target_arch = "powerpc", target_arch = "hexagon", target_arch = "arm")),
        all(target_os = "linux", target_env = "musl", any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "mips64", target_arch = "powerpc64")),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
    ))]
    MEMLOCK = 8 => RLIMIT_MEMLOCK,


    /// A limit on the number
    /// of bytes that can be allocated for POSIX message queues
    /// for the real user ID of the calling process.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
    ))]
    MSGQUEUE = 9 => RLIMIT_MSGQUEUE,


    /// This specifies a ceiling
    /// to which the process's nice value can be raised
    /// using `setpriority(2)` or `nice(2)`.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
    ))]
    NICE = 10 => RLIMIT_NICE,


    /// This specifies a value
    /// one greater than the maximum file descriptor number
    /// that can be opened by this process.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl", any(target_arch = "x86", target_arch = "mips", target_arch = "powerpc", target_arch = "hexagon", target_arch = "arm")),
        all(target_os = "linux", target_env = "musl", any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "mips64", target_arch = "powerpc64")),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "solarish",
    ))]
    NOFILE = 11 => RLIMIT_NOFILE,


    /// The number of open vnode monitors.
    #[cfg(any(
        target_os = "haiku",
    ))]
    NOVMON = 12 => RLIMIT_NOVMON,


    /// A limit on the number of extant process (or, more precisely on Linux, threads)
    /// for the real user ID of the calling process.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl", any(target_arch = "x86", target_arch = "mips", target_arch = "powerpc", target_arch = "hexagon", target_arch = "arm")),
        all(target_os = "linux", target_env = "musl", any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "mips64", target_arch = "powerpc64")),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
    ))]
    NPROC = 13 => RLIMIT_NPROC,


    /// The maximum number of pseudo-terminals this user id is allowed to create.
    #[cfg(any(
        target_os = "freebsd",
    ))]
    NPTS = 14 => RLIMIT_NPTS,


    /// The maximum number of simultaneous threads (Lightweight
    /// Processes) for this user id.  Kernel threads and the
    /// first thread of each process are not counted against this
    /// limit.
    #[cfg(any(
        target_os = "netbsd",
    ))]
    NTHR = 15 => RLIMIT_NTHR,


    /// The maximum number of POSIX-type advisory-mode locks available to this user.
    #[cfg(any(
        target_os = "dragonfly",
    ))]
    POSIXLOCKS = 16 => RLIMIT_POSIXLOCKS,


    /// A limit (in bytes)
    /// on the process's resident set
    /// (the number of virtual pages resident in RAM).
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl", any(target_arch = "x86", target_arch = "mips", target_arch = "powerpc", target_arch = "hexagon", target_arch = "arm")),
        all(target_os = "linux", target_env = "musl", any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "mips64", target_arch = "powerpc64")),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
    ))]
    RSS = 17 => RLIMIT_RSS,


    /// This specifies a ceiling on the real-time priority
    /// that may be set for this process
    /// using `sched_setscheduler(2)` and `sched_setparam(2)`.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
    ))]
    RTPRIO = 18 => RLIMIT_RTPRIO,


    /// A limit (in microseconds) on the amount of CPU time
    /// that a process scheduled under a real-time scheduling policy
    /// may consume without making a blocking system call.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        target_os = "fuchsia",
    ))]
    RTTIME = 19 => RLIMIT_RTTIME,


    /// The maximum size (in bytes) of socket buffer usage for
    /// this user. This limits the amount of network memory, and
    /// hence the amount of mbufs, that this user may hold at any
    /// time.
    #[cfg(any(
        any(target_os = "freebsd", target_os = "dragonfly"),
        target_os = "netbsd",
    ))]
    SBSIZE = 20 => RLIMIT_SBSIZE,


    /// A limit on the number
    /// of signals that may be queued
    /// for the real user ID of the calling process.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
    ))]
    SIGPENDING = 21 => RLIMIT_SIGPENDING,


    /// The maximum size (in bytes)
    /// of the process stack.
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "solarish",
    ))]
    STACK = 22 => RLIMIT_STACK,


    /// The maximum size (in bytes) of the swap space that may be
    /// reserved or used by all of this user id's processes.
    #[cfg(any(
        target_os = "freebsd",
    ))]
    SWAP = 23 => RLIMIT_SWAP,


    /// The number of shared locks a given user may create simultaneously.
    #[cfg(any(
        target_os = "freebsd",
    ))]
    UMTXP = 24 => RLIMIT_UMTXP,


    /// An alias for RLIMIT_AS. The maximum size of a process's mapped address space in bytes.
    #[cfg(any(
        any(target_os = "freebsd", target_os = "dragonfly"),
        target_os = "solarish",
    ))]
    VMEM = 25 => RLIMIT_VMEM,

}
// #end-codegen
