//! System call arguments and return values are all `usize`. This module
//! provides functions for converting into and out of `usize` values.
//!
//! # Safety
//!
//! Some of these functions are `unsafe` because they `transmute` `Option`
//! types knowing their layouts, or construct owned file descriptors.
#![allow(unsafe_code)]

use super::c;
use super::fd::{AsRawFd, BorrowedFd, FromRawFd, RawFd};
use super::fs::{FileType, Mode, OFlags};
#[cfg(not(debug_assertions))]
use super::io::error::decode_usize_infallible;
#[cfg(target_pointer_width = "64")]
use super::io::error::try_decode_u64;
use super::io::error::{
    try_decode_c_int, try_decode_c_uint, try_decode_error, try_decode_raw_fd, try_decode_usize,
    try_decode_void, try_decode_void_star,
};
use super::reg::{raw_arg, ArgNumber, ArgReg, RetReg, R0};
use super::time::{ClockId, TimerfdClockId};
use crate::ffi::ZStr;
use crate::io::{self, OwnedFd};
use crate::process::{Pid, Resource, Signal};
use crate::{as_mut_ptr, as_ptr};
use core::mem::{transmute, MaybeUninit};
use core::ptr::null_mut;
#[cfg(target_pointer_width = "64")]
use linux_raw_sys::general::__kernel_loff_t;
#[cfg(target_pointer_width = "32")]
use linux_raw_sys::general::O_LARGEFILE;
use linux_raw_sys::general::{__kernel_clockid_t, socklen_t};

/// Convert `SYS_*` constants for socketcall.
#[cfg(target_arch = "x86")]
#[inline]
pub(super) fn x86_sys<'a, Num: ArgNumber>(sys: u32) -> ArgReg<'a, Num> {
    pass_usize(sys as usize)
}

/// Pass the "low" half of the endian-specific memory encoding of a `u64`, for
/// 32-bit architectures.
#[cfg(target_pointer_width = "32")]
#[inline]
pub(super) fn lo<'a, Num: ArgNumber>(x: u64) -> ArgReg<'a, Num> {
    #[cfg(target_endian = "little")]
    let x = x >> 32;
    #[cfg(target_endian = "big")]
    let x = x & 0xffff_ffff;

    pass_usize(x as usize)
}

/// Pass the "high" half of the endian-specific memory encoding of a `u64`, for
/// 32-bit architectures.
#[cfg(target_pointer_width = "32")]
#[inline]
pub(super) fn hi<'a, Num: ArgNumber>(x: u64) -> ArgReg<'a, Num> {
    #[cfg(target_endian = "little")]
    let x = x & 0xffff_ffff;
    #[cfg(target_endian = "big")]
    let x = x >> 32;

    pass_usize(x as usize)
}

/// Pass a zero, or null, argument.
#[inline]
pub(super) fn zero<'a, Num: ArgNumber>() -> ArgReg<'a, Num> {
    raw_arg(null_mut())
}

/// Pass the `mem::size_of` of a type.
#[inline]
pub(super) fn size_of<'a, T: Sized, Num: ArgNumber>() -> ArgReg<'a, Num> {
    pass_usize(core::mem::size_of::<T>())
}

/// Pass an arbitrary `usize` value.
///
/// For passing pointers, use `void_star` or other functions which take a raw
/// pointer instead of casting to `usize`, so that provenance is preserved.
#[inline]
pub(super) fn pass_usize<'a, Num: ArgNumber>(t: usize) -> ArgReg<'a, Num> {
    raw_arg(t as *mut _)
}

#[inline]
pub(super) fn void_star<'a, Num: ArgNumber>(c: *mut c::c_void) -> ArgReg<'a, Num> {
    raw_arg(c.cast())
}

#[inline]
pub(super) fn const_void_star<'a, Num: ArgNumber>(c: *const c::c_void) -> ArgReg<'a, Num> {
    let mut_ptr = c as *mut c::c_void;
    raw_arg(mut_ptr.cast())
}

#[inline]
pub(super) fn c_str<'a, Num: ArgNumber>(c: &'a ZStr) -> ArgReg<'a, Num> {
    let mut_ptr = c.as_ptr() as *mut u8;
    raw_arg(mut_ptr.cast())
}

#[inline]
pub(super) fn opt_c_str<'a, Num: ArgNumber>(t: Option<&'a ZStr>) -> ArgReg<'a, Num> {
    raw_arg(match t {
        Some(s) => {
            let mut_ptr = s.as_ptr() as *mut u8;
            mut_ptr.cast()
        }
        None => null_mut(),
    })
}

/// Pass a borrowed file-descriptor argument.
#[inline]
pub(super) fn borrowed_fd<'a, Num: ArgNumber>(fd: BorrowedFd<'a>) -> ArgReg<'a, Num> {
    // Safety: `BorrowedFd` ensures that the file descriptor is valid, and the
    // lifetime parameter on the resulting `ArgReg` ensures that the result is
    // bounded by the `BorrowedFd`'s lifetime.
    unsafe { raw_fd(fd.as_raw_fd()) }
}

/// Pass a raw file-descriptor argument. Most users should use [`borrowed_fd`]
/// instead, to preserve I/O safety as long as possible.
///
/// # Safety
///
/// `fd` must be a valid open file descriptor.
#[inline]
pub(super) unsafe fn raw_fd<'a, Num: ArgNumber>(fd: RawFd) -> ArgReg<'a, Num> {
    // Use `no_fd` when passing `-1` is intended.
    debug_assert!(fd == crate::fs::cwd().as_raw_fd() || fd >= 0);

    // Linux doesn't look at the high bits beyond the `c_int`, so use
    // zero-extension rather than sign-extension because it's a smaller
    // instruction.
    let fd: c::c_int = fd;
    pass_usize(fd as c::c_uint as usize)
}

/// Deliberately pass `-1` to a file-descriptor argument, for system calls
/// like `mmap` where this indicates the argument is omitted.
#[inline]
pub(super) fn no_fd<'a, Num: ArgNumber>() -> ArgReg<'a, Num> {
    pass_usize(!0_usize)
}

#[inline]
pub(super) fn slice_just_addr<'a, T: Sized, Num: ArgNumber>(v: &'a [T]) -> ArgReg<'a, Num> {
    let mut_ptr = v.as_ptr() as *mut T;
    raw_arg(mut_ptr.cast())
}

#[inline]
pub(super) fn slice<'a, T: Sized, Num0: ArgNumber, Num1: ArgNumber>(
    v: &'a [T],
) -> (ArgReg<'a, Num0>, ArgReg<'a, Num1>) {
    (slice_just_addr(v), pass_usize(v.len()))
}

#[inline]
pub(super) fn slice_mut<'a, T: Sized, Num0: ArgNumber, Num1: ArgNumber>(
    v: &mut [T],
) -> (ArgReg<'a, Num0>, ArgReg<'a, Num1>) {
    (raw_arg(v.as_mut_ptr().cast()), pass_usize(v.len()))
}

#[inline]
pub(super) fn by_ref<'a, T: Sized, Num: ArgNumber>(t: &'a T) -> ArgReg<'a, Num> {
    let mut_ptr = as_ptr(t) as *mut T;
    raw_arg(mut_ptr.cast())
}

#[inline]
pub(super) fn by_mut<'a, T: Sized, Num: ArgNumber>(t: &'a mut T) -> ArgReg<'a, Num> {
    raw_arg(as_mut_ptr(t).cast())
}

/// Convert an optional mutable reference into a `usize` for passing to a
/// syscall.
///
/// # Safety
///
/// `Option<&mut T>` is represented as a nullable pointer to `T`, which is the
/// same size as a `usize`, so we can directly transmute it and pass the result
/// to syscalls expecting nullable pointers.
#[inline]
pub(super) unsafe fn opt_mut<'a, T: Sized, Num: ArgNumber>(
    t: Option<&'a mut T>,
) -> ArgReg<'a, Num> {
    transmute(t)
}

/// Convert an optional immutable reference into a `usize` for passing to a
/// syscall.
///
/// # Safety
///
/// `Option<&T>` is represented as a nullable pointer to `T`, which is the
/// same size as a `usize`, so we can directly transmute it and pass the result
/// to syscalls expecting nullable pointers.
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
#[inline]
pub(super) unsafe fn opt_ref<'a, T: Sized, Num: ArgNumber>(t: Option<&'a T>) -> ArgReg<'a, Num> {
    transmute(t)
}

#[inline]
pub(super) fn c_int<'a, Num: ArgNumber>(i: c::c_int) -> ArgReg<'a, Num> {
    pass_usize(i as usize)
}

#[inline]
pub(super) fn c_uint<'a, Num: ArgNumber>(i: c::c_uint) -> ArgReg<'a, Num> {
    pass_usize(i as usize)
}

#[cfg(target_pointer_width = "64")]
#[inline]
pub(super) fn loff_t<'a, Num: ArgNumber>(i: __kernel_loff_t) -> ArgReg<'a, Num> {
    pass_usize(i as usize)
}

#[cfg(target_pointer_width = "64")]
#[inline]
pub(super) fn loff_t_from_u64<'a, Num: ArgNumber>(i: u64) -> ArgReg<'a, Num> {
    // `loff_t` is signed, but syscalls which expect `loff_t` return `EINVAL`
    // if it's outside the signed `i64` range, so we can silently cast.
    pass_usize(i as usize)
}

#[inline]
pub(super) fn clockid_t<'a, Num: ArgNumber>(i: ClockId) -> ArgReg<'a, Num> {
    pass_usize(i as __kernel_clockid_t as usize)
}

#[inline]
pub(super) fn timerfd_clockid_t<'a, Num: ArgNumber>(i: TimerfdClockId) -> ArgReg<'a, Num> {
    pass_usize(i as __kernel_clockid_t as usize)
}

#[inline]
pub(super) fn socklen_t<'a, Num: ArgNumber>(i: socklen_t) -> ArgReg<'a, Num> {
    pass_usize(i as usize)
}

#[inline]
pub(super) fn mode_as<'a, Num: ArgNumber>(mode: Mode) -> ArgReg<'a, Num> {
    pass_usize(mode.bits() as usize)
}

#[inline]
pub(super) fn mode_and_type_as<'a, Num: ArgNumber>(
    mode: Mode,
    file_type: FileType,
) -> ArgReg<'a, Num> {
    pass_usize(mode.as_raw_mode() as usize | file_type.as_raw_mode() as usize)
}

#[cfg(target_pointer_width = "64")]
#[inline]
pub(super) fn dev_t<'a, Num: ArgNumber>(dev: u64) -> ArgReg<'a, Num> {
    pass_usize(dev as usize)
}

#[cfg(target_pointer_width = "32")]
#[inline]
pub(super) fn dev_t<'a, Num: ArgNumber>(dev: u64) -> io::Result<ArgReg<'a, Num>> {
    use core::convert::TryInto;
    Ok(pass_usize(dev.try_into().map_err(|_err| io::Error::INVAL)?))
}

#[cfg(target_pointer_width = "32")]
#[inline]
fn oflags_bits(oflags: OFlags) -> c::c_uint {
    let mut bits = oflags.bits();
    // Add `O_LARGEFILE`, unless `O_PATH` is set, as Linux returns `EINVAL`
    // when both are set.
    if !oflags.contains(OFlags::PATH) {
        bits |= O_LARGEFILE;
    }
    bits
}

#[cfg(target_pointer_width = "64")]
#[inline]
const fn oflags_bits(oflags: OFlags) -> c::c_uint {
    oflags.bits()
}

#[inline]
pub(super) fn oflags<'a, Num: ArgNumber>(oflags: OFlags) -> ArgReg<'a, Num> {
    pass_usize(oflags_bits(oflags) as usize)
}

#[inline]
pub(super) fn oflags_for_open_how(oflags: OFlags) -> u64 {
    u64::from(oflags_bits(oflags))
}

/// Convert a `Resource` into a syscall argument.
#[inline]
pub(super) fn resource<'a, Num: ArgNumber>(resource: Resource) -> ArgReg<'a, Num> {
    c_uint(resource as c::c_uint)
}

#[inline]
pub(super) fn regular_pid<'a, Num: ArgNumber>(pid: Pid) -> ArgReg<'a, Num> {
    pass_usize(pid.as_raw_nonzero().get() as usize)
}

#[inline]
pub(super) fn negative_pid<'a, Num: ArgNumber>(pid: Pid) -> ArgReg<'a, Num> {
    pass_usize(pid.as_raw_nonzero().get().wrapping_neg() as usize)
}

#[inline]
pub(super) fn signal<'a, Num: ArgNumber>(sig: Signal) -> ArgReg<'a, Num> {
    pass_usize(sig as usize)
}

#[inline]
pub(super) fn fs_advice<'a, Num: ArgNumber>(advice: crate::fs::Advice) -> ArgReg<'a, Num> {
    c_uint(advice as c::c_uint)
}

#[inline]
pub(super) fn out<'a, T: Sized, Num: ArgNumber>(t: &'a mut MaybeUninit<T>) -> ArgReg<'a, Num> {
    raw_arg(t.as_mut_ptr().cast())
}

/// Convert a `usize` returned from a syscall that effectively returns `()` on
/// success.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a syscall which
/// just returns 0 on success.
#[inline]
pub(super) unsafe fn ret(raw: RetReg<R0>) -> io::Result<()> {
    try_decode_void(raw)
}

/// Convert a `usize` returned from a syscall that doesn't return on success.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a syscall which
/// doesn't return on success.
#[inline]
pub(super) unsafe fn ret_error(raw: RetReg<R0>) -> io::Error {
    try_decode_error(raw)
}

/// Convert a `usize` returned from a syscall that effectively always returns
/// `()`.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a syscall which
/// always returns `()`.
#[inline]
pub(super) unsafe fn ret_infallible(raw: RetReg<R0>) {
    let _ = raw;
    #[cfg(debug_assertions)]
    {
        try_decode_void(raw).unwrap()
    }
}

/// Convert a `usize` returned from a syscall that effectively returns a
/// `c_int` on success.
#[inline]
pub(super) fn ret_c_int(raw: RetReg<R0>) -> io::Result<c::c_int> {
    try_decode_c_int(raw)
}

/// Convert a `usize` returned from a syscall that effectively returns a
/// `c_uint` on success.
#[inline]
pub(super) fn ret_c_uint(raw: RetReg<R0>) -> io::Result<c::c_uint> {
    try_decode_c_uint(raw)
}

/// Convert a `usize` returned from a syscall that effectively returns a `u64`
/// on success.
#[cfg(target_pointer_width = "64")]
#[inline]
pub(super) fn ret_u64(raw: RetReg<R0>) -> io::Result<u64> {
    try_decode_u64(raw)
}

/// Convert a `usize` returned from a syscall that effectively returns a
/// `usize` on success.
#[inline]
pub(super) fn ret_usize(raw: RetReg<R0>) -> io::Result<usize> {
    try_decode_usize(raw)
}

/// Convert a `usize` returned from a syscall that effectively always
/// returns a `usize`.
///
/// # Safety
///
/// This function must only be used with return values from infallible
/// syscalls.
#[inline]
pub(super) unsafe fn ret_usize_infallible(raw: RetReg<R0>) -> usize {
    #[cfg(debug_assertions)]
    {
        try_decode_usize(raw).unwrap()
    }
    #[cfg(not(debug_assertions))]
    {
        decode_usize_infallible(raw)
    }
}

/// Convert a `usize` returned from a syscall that effectively returns an
/// `OwnedFd` on success.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a syscall which
/// returns an owned file descriptor.
#[inline]
pub(super) unsafe fn ret_owned_fd(raw: RetReg<R0>) -> io::Result<OwnedFd> {
    let raw_fd = try_decode_raw_fd(raw)?;
    Ok(OwnedFd::from(crate::imp::fd::OwnedFd::from_raw_fd(raw_fd)))
}

/// Convert the return value of `dup2` and `dup3`.
///
/// When these functions succeed, they return the same value as their second
/// argument, so we don't construct a new `OwnedFd`.
///
/// # Safety
///
/// The caller must ensure that this is the return value of a syscall which
/// returns a file descriptor.
#[inline]
pub(super) unsafe fn ret_discarded_fd(raw: RetReg<R0>) -> io::Result<()> {
    let _raw_fd = try_decode_raw_fd(raw)?;
    Ok(())
}

/// Convert a `usize` returned from a syscall that effectively returns a
/// `*mut c_void` on success.
#[inline]
pub(super) fn ret_void_star(raw: RetReg<R0>) -> io::Result<*mut c::c_void> {
    try_decode_void_star(raw)
}
