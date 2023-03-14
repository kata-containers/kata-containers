#![allow(non_camel_case_types)]

mod resource;

pub use self::resource::{RawResource, Resource};

#[cfg(target_os = "linux")]
group! {
    mod proc_limits;
    pub use self::proc_limits::{ProcLimit, ProcLimits};
}

// #begin-codegen
// generated from rust-lang/libc ec88c377ab1695d7bdd721332382e7cecc07b7e3
#[cfg(any(target_os = "emscripten", target_os = "fuchsia", target_os = "linux",))]
group! {
    type c_rlimit = libc::rlimit64;
    use libc::setrlimit64 as c_setrlimit;
    use libc::getrlimit64 as c_getrlimit;
    const RLIM_INFINITY: u64 = u64::MAX;
    const RLIM_SAVED_CUR: u64 = u64::MAX;
    const RLIM_SAVED_MAX: u64 = u64::MAX;
}

#[cfg(not(any(target_os = "emscripten", target_os = "fuchsia", target_os = "linux",)))]
group! {
    type c_rlimit = libc::rlimit;
    use libc::setrlimit as c_setrlimit;
    use libc::getrlimit as c_getrlimit;
    #[cfg(any(
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        all(target_os = "linux", target_env = "uclibc", target_arch = "arm"),
        all(target_os = "linux", target_env = "uclibc", target_arch = "mips"),
        all(target_os = "linux", target_env = "uclibc", target_arch = "mips64"),
        all(target_os = "linux", target_env = "uclibc", target_arch = "x86_64"),
        any(target_os = "freebsd", target_os = "dragonfly"),
        any(target_os = "macos", target_os = "ios"),
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "android",
        target_os = "emscripten",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "solarish",
    ))]
    const RLIM_INFINITY: u64 = libc::RLIM_INFINITY as u64;
    #[cfg(any(
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "emscripten",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "linux",
    ))]
    const RLIM_SAVED_CUR: u64 = libc::RLIM_SAVED_CUR as u64;
    #[cfg(any(
        any(target_os = "openbsd", target_os = "netbsd"),
        target_os = "emscripten",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "linux",
    ))]
    const RLIM_SAVED_MAX: u64 = libc::RLIM_SAVED_MAX as u64;
}

/// A value indicating no limit.
#[cfg(any(
    all(target_os = "linux", target_env = "gnu"),
    all(target_os = "linux", target_env = "musl"),
    all(target_os = "linux", target_env = "uclibc", target_arch = "arm"),
    all(target_os = "linux", target_env = "uclibc", target_arch = "mips"),
    all(target_os = "linux", target_env = "uclibc", target_arch = "mips64"),
    all(target_os = "linux", target_env = "uclibc", target_arch = "x86_64"),
    any(target_os = "freebsd", target_os = "dragonfly"),
    any(target_os = "macos", target_os = "ios"),
    any(target_os = "openbsd", target_os = "netbsd"),
    target_os = "android",
    target_os = "emscripten",
    target_os = "fuchsia",
    target_os = "haiku",
    target_os = "solarish",
))]
pub const INFINITY: u64 = RLIM_INFINITY;

/// A value indicating an unrepresentable saved soft limit.
#[cfg(any(
    any(target_os = "openbsd", target_os = "netbsd"),
    target_os = "emscripten",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "linux",
))]
pub const SAVED_CUR: u64 = RLIM_SAVED_CUR;

/// A value indicating an unrepresentable saved hard limit.
#[cfg(any(
    any(target_os = "openbsd", target_os = "netbsd"),
    target_os = "emscripten",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "linux",
))]
pub const SAVED_MAX: u64 = RLIM_SAVED_MAX;

// #end-codegen

use std::io;
use std::mem;

use libc::c_int;

#[cfg(any(doc, target_os = "linux"))]
use libc::pid_t;

/// Set resource limits.
/// # Errors
/// \[Linux\] See <https://man7.org/linux/man-pages/man2/setrlimit.2.html>
#[inline]
pub fn setrlimit(resource: Resource, soft: u64, hard: u64) -> io::Result<()> {
    let raw_resource = resource.as_raw();

    let rlim = c_rlimit {
        rlim_cur: soft.min(INFINITY) as _,
        rlim_max: hard.min(INFINITY) as _,
    };

    let ret: c_int = unsafe { c_setrlimit(raw_resource, &rlim) };

    if ret == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Get resource limits.
/// # Errors
/// \[Linux\] See <https://man7.org/linux/man-pages/man2/getrlimit.2.html>
#[inline]
pub fn getrlimit(resource: Resource) -> io::Result<(u64, u64)> {
    let raw_resource = resource.as_raw();

    let mut rlim: c_rlimit = unsafe { mem::zeroed() };

    let ret: c_int = unsafe { c_getrlimit(raw_resource, &mut rlim) };

    if ret == 0 {
        let soft = rlim.rlim_cur as u64;
        let hard = rlim.rlim_max as u64;
        Ok((soft, hard))
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Set and get the resource limits of an arbitrary process.
/// # Errors
/// See <https://man7.org/linux/man-pages/man2/prlimit.2.html>
#[inline]
#[cfg(any(doc, target_os = "linux"))]
#[cfg_attr(docsrs, doc(cfg(target_os = "linux")))]
pub fn prlimit(
    pid: pid_t,
    resource: Resource,
    new_limit: Option<(u64, u64)>,
    old_limit: Option<(&mut u64, &mut u64)>,
) -> io::Result<()> {
    use std::ptr;

    let raw_resource = resource.as_raw();

    let new_rlim: Option<libc::rlimit> = new_limit.map(|(soft, hard)| libc::rlimit {
        rlim_cur: soft.min(INFINITY) as _,
        rlim_max: hard.min(INFINITY) as _,
    });

    let new_rlimit_ptr: *const libc::rlimit = match new_rlim {
        Some(ref rlim) => rlim,
        None => ptr::null(),
    };

    let mut old_rlim: libc::rlimit = unsafe { mem::zeroed() };

    let old_rlimit_ptr: *mut libc::rlimit = if old_limit.is_some() {
        &mut old_rlim
    } else {
        ptr::null_mut()
    };

    let ret: c_int = unsafe { libc::prlimit(pid, raw_resource, new_rlimit_ptr, old_rlimit_ptr) };

    if ret == 0 {
        if let Some((soft, hard)) = old_limit {
            *soft = old_rlim.rlim_cur as u64;
            *hard = old_rlim.rlim_max as u64;
        }

        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
