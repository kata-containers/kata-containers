use std::ffi::CString;
use std::ffi::OsStr;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use libc::{ssize_t, ERANGE};

// Need to use this one as libc only defines this on supported platforms. Given
// that we want to at least compile on unsupported platforms, we define this in
// our platform-specific modules.
use sys::ENOATTR;

#[allow(dead_code)]
pub fn name_to_c(name: &OsStr) -> io::Result<CString> {
    match CString::new(name.as_bytes()) {
        Ok(name) => Ok(name),
        Err(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "name must not contain null bytes",
        )),
    }
}

pub fn path_to_c(path: &Path) -> io::Result<CString> {
    match CString::new(path.as_os_str().as_bytes()) {
        Ok(name) => Ok(name),
        Err(_) => Err(io::Error::new(io::ErrorKind::NotFound, "file not found")),
    }
}

pub fn extract_noattr(result: io::Result<Vec<u8>>) -> io::Result<Option<Vec<u8>>> {
    result.map(Some).or_else(|e| match e.raw_os_error() {
        Some(ENOATTR) => Ok(None),
        _ => Err(e),
    })
}

pub unsafe fn allocate_loop<F: FnMut(*mut u8, usize) -> ssize_t>(mut f: F) -> io::Result<Vec<u8>> {
    let mut vec: Vec<u8> = Vec::new();
    loop {
        let ret = (f)(ptr::null_mut(), 0);
        if ret < 0 {
            return Err(io::Error::last_os_error());
        } else if ret == 0 {
            break;
        }
        vec.reserve_exact(ret as usize);

        let ret = (f)(vec.as_mut_ptr(), vec.capacity());
        if ret >= 0 {
            vec.set_len(ret as usize);
            break;
        } else {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERANGE) {
                continue;
            }
            return Err(error);
        }
    }
    vec.shrink_to_fit();
    Ok(vec)
}
