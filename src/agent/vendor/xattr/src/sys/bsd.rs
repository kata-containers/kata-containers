//! FreeBSD and NetBSD xattr support.

use std::ffi::{CString, OsStr, OsString};
use std::io;
use std::mem;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::io::RawFd;
use std::path::Path;

use libc::{c_char, c_int, c_void, size_t, ssize_t, EPERM};
use util::{allocate_loop, path_to_c};

const EXTATTR_NAMESPACE_USER_STRING: &'static str = "user";
const EXTATTR_NAMESPACE_SYSTEM_STRING: &'static str = "system";
const EXTATTR_NAMESPACE_NAMES: [&'static str; 3] = [
    "empty",
    EXTATTR_NAMESPACE_USER_STRING,
    EXTATTR_NAMESPACE_SYSTEM_STRING,
];
const EXTATTR_NAMESPACE_USER: c_int = 1;
const EXTATTR_NAMESPACE_SYSTEM: c_int = 2;

extern "C" {
    pub fn extattr_list_fd(
        fd: c_int,
        attrnamespace: c_int,
        data: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_get_fd(
        fd: c_int,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_delete_fd(fd: c_int, attrname: c_int, attrname: *const c_char) -> c_int;
    pub fn extattr_set_fd(
        fd: c_int,
        attrname: c_int,
        attrname: *const c_char,
        data: *const c_void,
        nbytes: size_t,
    ) -> ssize_t;

    pub fn extattr_list_link(
        path: *const c_char,
        attrnamespace: c_int,
        data: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_get_link(
        path: *const c_char,
        attrnamespace: c_int,
        attrname: *const c_char,
        data: *mut c_void,
        nbytes: size_t,
    ) -> ssize_t;
    pub fn extattr_delete_link(
        path: *const c_char,
        attrname: c_int,
        attrname: *const c_char,
    ) -> c_int;
    pub fn extattr_set_link(
        path: *const c_char,
        attrname: c_int,
        attrname: *const c_char,
        data: *const c_void,
        nbytes: size_t,
    ) -> ssize_t;
}

/// An iterator over a set of extended attributes names.
pub struct XAttrs {
    user_attrs: Box<[u8]>,
    system_attrs: Box<[u8]>,
    offset: usize,
}

impl Clone for XAttrs {
    fn clone(&self) -> Self {
        XAttrs {
            user_attrs: Vec::from(&*self.user_attrs).into_boxed_slice(),
            system_attrs: Vec::from(&*self.system_attrs).into_boxed_slice(),
            offset: self.offset,
        }
    }

    fn clone_from(&mut self, other: &XAttrs) {
        self.offset = other.offset;

        let mut data = mem::replace(&mut self.user_attrs, Box::new([])).into_vec();
        data.extend(other.user_attrs.iter().cloned());
        self.user_attrs = data.into_boxed_slice();

        data = mem::replace(&mut self.system_attrs, Box::new([])).into_vec();
        data.extend(other.system_attrs.iter().cloned());
        self.system_attrs = data.into_boxed_slice();
    }
}

impl Iterator for XAttrs {
    type Item = OsString;
    fn next(&mut self) -> Option<OsString> {
        if self.user_attrs.is_empty() && self.system_attrs.is_empty() {
            return None;
        }

        if self.offset == self.user_attrs.len() + self.system_attrs.len() {
            return None;
        }

        let data = if self.offset < self.system_attrs.len() {
            &self.system_attrs[self.offset..]
        } else {
            &self.user_attrs[self.offset - self.system_attrs.len()..]
        };

        let siz = data[0] as usize;

        self.offset += siz + 1;
        if self.offset < self.system_attrs.len() {
            Some(prefix_namespace(
                OsStr::from_bytes(&data[1..siz + 1]),
                EXTATTR_NAMESPACE_SYSTEM,
            ))
        } else {
            Some(prefix_namespace(
                OsStr::from_bytes(&data[1..siz + 1]),
                EXTATTR_NAMESPACE_USER,
            ))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.user_attrs.len() + self.system_attrs.len() == self.offset {
            (0, Some(0))
        } else {
            (1, None)
        }
    }
}

fn name_to_ns(name: &OsStr) -> io::Result<(c_int, CString)> {
    let mut groups = name.as_bytes().splitn(2, |&b| b == b'.').take(2);
    let nsname = match groups.next() {
        Some(s) => s,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "couldn't find namespace",
            ))
        }
    };

    let propname = match groups.next() {
        Some(s) => s,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "couldn't find attribute",
            ))
        }
    };

    let ns_int = match EXTATTR_NAMESPACE_NAMES
        .iter()
        .position(|&s| s.as_bytes() == nsname)
    {
        Some(i) => i,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no matching namespace",
            ))
        }
    };

    Ok((ns_int as c_int, CString::new(propname)?))
}

fn prefix_namespace(attr: &OsStr, ns: c_int) -> OsString {
    let nsname = EXTATTR_NAMESPACE_NAMES[ns as usize];
    let mut v = Vec::with_capacity(nsname.as_bytes().len() + attr.as_bytes().len() + 1);
    v.extend(nsname.as_bytes());
    v.extend(".".as_bytes());
    v.extend(attr.as_bytes());
    OsString::from_vec(v)
}

pub fn get_fd(fd: RawFd, name: &OsStr) -> io::Result<Vec<u8>> {
    let (ns, name) = name_to_ns(name)?;
    unsafe {
        allocate_loop(|buf, len| {
            extattr_get_fd(fd, ns, name.as_ptr(), buf as *mut c_void, len as size_t)
        })
    }
}

pub fn set_fd(fd: RawFd, name: &OsStr, value: &[u8]) -> io::Result<()> {
    let (ns, name) = name_to_ns(name)?;
    let ret = unsafe {
        extattr_set_fd(
            fd,
            ns,
            name.as_ptr(),
            value.as_ptr() as *const c_void,
            value.len() as size_t,
        )
    };
    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn remove_fd(fd: RawFd, name: &OsStr) -> io::Result<()> {
    let (ns, name) = name_to_ns(name)?;
    let ret = unsafe { extattr_delete_fd(fd, ns, name.as_ptr()) };
    if ret != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn list_fd(fd: RawFd) -> io::Result<XAttrs> {
    let sysvec = unsafe {
        let res = allocate_loop(|buf, len| {
            extattr_list_fd(
                fd,
                EXTATTR_NAMESPACE_SYSTEM,
                buf as *mut c_void,
                len as size_t,
            )
        });
        // On FreeBSD, system attributes require root privileges to view. However,
        // to mimic the behavior of listxattr in linux and osx, we need to query
        // them anyway and return empty results if we get EPERM
        match res {
            Ok(v) => v,
            Err(err) => {
                if err.raw_os_error() == Some(EPERM) {
                    Vec::new()
                } else {
                    return Err(err);
                }
            }
        }
    };

    let uservec = unsafe {
        let res = allocate_loop(|buf, len| {
            extattr_list_fd(
                fd,
                EXTATTR_NAMESPACE_USER,
                buf as *mut c_void,
                len as size_t,
            )
        });
        match res {
            Ok(v) => v,
            Err(err) => return Err(err),
        }
    };

    Ok(XAttrs {
        system_attrs: sysvec.into_boxed_slice(),
        user_attrs: uservec.into_boxed_slice(),
        offset: 0,
    })
}

pub fn get_path(path: &Path, name: &OsStr) -> io::Result<Vec<u8>> {
    let (ns, name) = name_to_ns(name)?;
    let path = path_to_c(path)?;
    unsafe {
        allocate_loop(|buf, len| {
            extattr_get_link(
                path.as_ptr(),
                ns,
                name.as_ptr(),
                buf as *mut c_void,
                len as size_t,
            )
        })
    }
}

pub fn set_path(path: &Path, name: &OsStr, value: &[u8]) -> io::Result<()> {
    let (ns, name) = name_to_ns(name)?;
    let path = path_to_c(path)?;
    let ret = unsafe {
        extattr_set_link(
            path.as_ptr(),
            ns,
            name.as_ptr(),
            value.as_ptr() as *const c_void,
            value.len() as size_t,
        )
    };
    if ret == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn remove_path(path: &Path, name: &OsStr) -> io::Result<()> {
    let (ns, name) = name_to_ns(name)?;
    let path = path_to_c(path)?;
    let ret = unsafe { extattr_delete_link(path.as_ptr(), ns, name.as_ptr()) };
    if ret != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn list_path(path: &Path) -> io::Result<XAttrs> {
    let path = path_to_c(path)?;
    let sysvec = unsafe {
        let res = allocate_loop(|buf, len| {
            extattr_list_link(
                path.as_ptr(),
                EXTATTR_NAMESPACE_SYSTEM,
                buf as *mut c_void,
                len as size_t,
            )
        });
        // On FreeBSD, system attributes require root privileges to view. However,
        // to mimic the behavior of listxattr in linux and osx, we need to query
        // them anyway and return empty results if we get EPERM
        match res {
            Ok(v) => v,
            Err(err) => {
                if err.raw_os_error() == Some(EPERM) {
                    Vec::new()
                } else {
                    return Err(err);
                }
            }
        }
    };

    let uservec = unsafe {
        let res = allocate_loop(|buf, len| {
            extattr_list_link(
                path.as_ptr(),
                EXTATTR_NAMESPACE_USER,
                buf as *mut c_void,
                len as size_t,
            )
        });
        match res {
            Ok(v) => v,
            Err(err) => return Err(err),
        }
    };

    Ok(XAttrs {
        system_attrs: sysvec.into_boxed_slice(),
        user_attrs: uservec.into_boxed_slice(),
        offset: 0,
    })
}
