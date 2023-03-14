//! linux_raw syscalls supporting `rustix::rand`.
//!
//! # Safety
//!
//! See the `rustix::imp::syscalls` module documentation for details.

#![allow(unsafe_code)]

use super::super::arch::choose::syscall3;
use super::super::conv::{c_uint, ret_usize, slice_mut};
use super::super::reg::nr;
use crate::io;
use crate::rand::GetRandomFlags;
use linux_raw_sys::general::__NR_getrandom;

#[inline]
pub(crate) fn getrandom(buf: &mut [u8], flags: GetRandomFlags) -> io::Result<usize> {
    let (buf_addr_mut, buf_len) = slice_mut(buf);
    unsafe {
        ret_usize(syscall3(
            nr(__NR_getrandom),
            buf_addr_mut,
            buf_len,
            c_uint(flags.bits()),
        ))
    }
}
