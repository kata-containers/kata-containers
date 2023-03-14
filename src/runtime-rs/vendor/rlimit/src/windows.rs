use std::io;
use std::os::raw::c_int;

extern "C" {
    fn _setmaxstdio(new_max: c_int) -> c_int;
    fn _getmaxstdio() -> c_int;
}

/// Sets a maximum for the number of simultaneously open files at the stream I/O level.
///
/// See <https://docs.microsoft.com/en-us/cpp/c-runtime-library/reference/setmaxstdio?view=msvc-170>
///
/// # Errors
/// See the official documentation
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub fn setmaxstdio(new_max: u32) -> io::Result<u32> {
    // A negative `new_max` will cause EINVAL.
    // A negative `ret` should never appear.
    // It is safe even if the return value is wrong.
    #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    unsafe {
        let ret = _setmaxstdio(new_max as c_int);
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ret as u32)
    }
}

/// Returns the number of simultaneously open files permitted at the stream I/O level.
///
/// See <https://docs.microsoft.com/en-us/cpp/c-runtime-library/reference/getmaxstdio?view=msvc-170>
#[cfg_attr(docsrs, doc(cfg(windows)))]
#[must_use]
pub fn getmaxstdio() -> u32 {
    // A negative `ret` should never appear.
    // It is safe even if the return value is wrong.
    #[allow(clippy::cast_sign_loss)]
    unsafe {
        let ret = _getmaxstdio();
        debug_assert!(ret >= 0);
        ret as u32
    }
}
