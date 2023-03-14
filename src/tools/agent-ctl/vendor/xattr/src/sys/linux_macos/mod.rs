#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;

#[cfg(any(target_os = "linux", target_os = "android"))]
use self::linux::*;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
use self::macos::*;

use std::ffi::{OsStr, OsString};
use std::io;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::Path;

use libc::{c_char, c_void, size_t};

use util::{allocate_loop, name_to_c, path_to_c};

/// An iterator over a set of extended attributes names.
pub struct XAttrs {
    data: Box<[u8]>,
    offset: usize,
}

impl Clone for XAttrs {
    fn clone(&self) -> Self {
        XAttrs {
            data: Vec::from(&*self.data).into_boxed_slice(),
            offset: self.offset,
        }
    }
    fn clone_from(&mut self, other: &XAttrs) {
        self.offset = other.offset;

        let mut data = mem::replace(&mut self.data, Box::new([])).into_vec();
        data.extend(other.data.iter().cloned());
        self.data = data.into_boxed_slice();
    }
}

// Yes, I could avoid these allocations on linux/macos. However, if we ever want to be freebsd
// compatible, we need to be able to prepend the namespace to the extended attribute names.
// Furthermore, borrowing makes the API messy.
impl Iterator for XAttrs {
    type Item = OsString;
    fn next(&mut self) -> Option<OsString> {
        let data = &self.data[self.offset..];
        if data.is_empty() {
            None
        } else {
            // always null terminated (unless empty).
            let end = data.iter().position(|&b| b == 0u8).unwrap();
            self.offset += end + 1;
            Some(OsStr::from_bytes(&data[..end]).to_owned())
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.data.len() == self.offset {
            (0, Some(0))
        } else {
            (1, None)
        }
    }
}

pub fn get_fd(fd: RawFd, name: &OsStr) -> io::Result<Vec<u8>> {
    let name = name_to_c(name)?;
    unsafe {
        allocate_loop(|buf, len| fgetxattr(fd, name.as_ptr(), buf as *mut c_void, len as size_t))
    }
}

pub fn set_fd(fd: RawFd, name: &OsStr, value: &[u8]) -> io::Result<()> {
    let name = name_to_c(name)?;
    let ret = unsafe {
        fsetxattr(
            fd,
            name.as_ptr(),
            value.as_ptr() as *const c_void,
            value.len() as size_t,
        )
    };
    if ret != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn remove_fd(fd: RawFd, name: &OsStr) -> io::Result<()> {
    let name = name_to_c(name)?;
    let ret = unsafe { fremovexattr(fd, name.as_ptr()) };
    if ret != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn list_fd(fd: RawFd) -> io::Result<XAttrs> {
    let vec =
        unsafe { allocate_loop(|buf, len| flistxattr(fd, buf as *mut c_char, len as size_t))? };
    Ok(XAttrs {
        data: vec.into_boxed_slice(),
        offset: 0,
    })
}

pub fn get_path(path: &Path, name: &OsStr) -> io::Result<Vec<u8>> {
    let name = name_to_c(name)?;
    let path = path_to_c(path)?;
    unsafe {
        allocate_loop(|buf, len| {
            lgetxattr(
                path.as_ptr(),
                name.as_ptr(),
                buf as *mut c_void,
                len as size_t,
            )
        })
    }
}

pub fn set_path(path: &Path, name: &OsStr, value: &[u8]) -> io::Result<()> {
    let name = name_to_c(name)?;
    let path = path_to_c(path)?;
    let ret = unsafe {
        lsetxattr(
            path.as_ptr(),
            name.as_ptr(),
            value.as_ptr() as *const c_void,
            value.len() as size_t,
        )
    };
    if ret != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn remove_path(path: &Path, name: &OsStr) -> io::Result<()> {
    let name = name_to_c(name)?;
    let path = path_to_c(path)?;
    let ret = unsafe { lremovexattr(path.as_ptr(), name.as_ptr()) };
    if ret != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn list_path(path: &Path) -> io::Result<XAttrs> {
    let path = path_to_c(path)?;
    let vec = unsafe {
        allocate_loop(|buf, len| llistxattr(path.as_ptr(), buf as *mut c_char, len as size_t))?
    };
    Ok(XAttrs {
        data: vec.into_boxed_slice(),
        offset: 0,
    })
}
