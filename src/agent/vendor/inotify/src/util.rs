use std::os::unix::io::RawFd;

use inotify_sys as ffi;
use libc::{
    c_void,
    size_t,
};


pub fn read_into_buffer(fd: RawFd, buffer: &mut [u8]) -> isize {
    unsafe {
        ffi::read(
            fd,
            buffer.as_mut_ptr() as *mut c_void,
            buffer.len() as size_t
        )
    }
}
