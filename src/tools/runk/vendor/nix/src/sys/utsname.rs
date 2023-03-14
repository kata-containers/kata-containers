//! Get system identification
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::ffi::OsStr;
use libc::c_char;
use crate::{Errno, Result};

/// Describes the running system.  Return type of [`uname`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct UtsName(libc::utsname);

impl UtsName {
    /// Name of the operating system implementation.
    pub fn sysname(&self) -> &OsStr {
        cast_and_trim(&self.0.sysname)
    }

    /// Network name of this machine.
    pub fn nodename(&self) -> &OsStr {
        cast_and_trim(&self.0.nodename)
    }

    /// Release level of the operating system.
    pub fn release(&self) -> &OsStr {
        cast_and_trim(&self.0.release)
    }

    /// Version level of the operating system.
    pub fn version(&self) -> &OsStr {
        cast_and_trim(&self.0.version)
    }

    /// Machine hardware platform.
    pub fn machine(&self) -> &OsStr {
        cast_and_trim(&self.0.machine)
    }
}

/// Get system identification
pub fn uname() -> Result<UtsName> {
    unsafe {
        let mut ret = mem::MaybeUninit::zeroed();
        Errno::result(libc::uname(ret.as_mut_ptr()))?;
        Ok(UtsName(ret.assume_init()))
    }
}

fn cast_and_trim(slice: &[c_char]) -> &OsStr {
    let length = slice.iter().position(|&byte| byte == 0).unwrap_or(slice.len());
    let bytes = unsafe {
        std::slice::from_raw_parts(slice.as_ptr().cast(), length)
    };

    OsStr::from_bytes(bytes)
}

#[cfg(test)]
mod test {
    #[cfg(target_os = "linux")]
    #[test]
    pub fn test_uname_linux() {
        assert_eq!(super::uname().unwrap().sysname(), "Linux");
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    pub fn test_uname_darwin() {
        assert_eq!(super::uname().unwrap().sysname(), "Darwin");
    }

    #[cfg(target_os = "freebsd")]
    #[test]
    pub fn test_uname_freebsd() {
        assert_eq!(super::uname().unwrap().sysname(), "FreeBSD");
    }
}
