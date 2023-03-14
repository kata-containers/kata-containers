#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    dead_code,
    non_snake_case,
    unused_qualifications
)]
#![allow(clippy::unreadable_literal, clippy::missing_safety_doc)]

use libc::*;

#[cfg(all(feature = "bindgen", not(feature = "overwrite")))]
include!(concat!(env!("OUT_DIR"), "/sys.rs"));

#[cfg(any(
    not(feature = "bindgen"),
    all(feature = "bindgen", feature = "overwrite")
))]
include!("sys.rs");

#[cfg(feature = "bindgen")]
const SYSCALL_REGISTER: c_long = __NR_io_uring_register as _;

#[cfg(not(feature = "bindgen"))]
const SYSCALL_REGISTER: c_long = libc::SYS_io_uring_register;

#[cfg(feature = "bindgen")]
const SYSCALL_SETUP: c_long = __NR_io_uring_setup as _;

#[cfg(not(feature = "bindgen"))]
const SYSCALL_SETUP: c_long = libc::SYS_io_uring_setup;

#[cfg(feature = "bindgen")]
const SYSCALL_ENTER: c_long = __NR_io_uring_enter as _;

#[cfg(not(feature = "bindgen"))]
const SYSCALL_ENTER: c_long = libc::SYS_io_uring_enter;

#[cfg(not(feature = "direct-syscall"))]
pub unsafe fn io_uring_register(
    fd: c_int,
    opcode: c_uint,
    arg: *const c_void,
    nr_args: c_uint,
) -> c_int {
    syscall(
        SYSCALL_REGISTER,
        fd as c_long,
        opcode as c_long,
        arg as c_long,
        nr_args as c_long,
    ) as _
}

#[cfg(feature = "direct-syscall")]
pub unsafe fn io_uring_register(
    fd: c_int,
    opcode: c_uint,
    arg: *const c_void,
    nr_args: c_uint,
) -> c_int {
    sc::syscall4(
        SYSCALL_REGISTER as usize,
        fd as usize,
        opcode as usize,
        arg as usize,
        nr_args as usize,
    ) as _
}

#[cfg(not(feature = "direct-syscall"))]
pub unsafe fn io_uring_setup(entries: c_uint, p: *mut io_uring_params) -> c_int {
    syscall(SYSCALL_SETUP, entries as c_long, p as c_long) as _
}

#[cfg(feature = "direct-syscall")]
pub unsafe fn io_uring_setup(entries: c_uint, p: *mut io_uring_params) -> c_int {
    sc::syscall2(SYSCALL_SETUP as usize, entries as usize, p as usize) as _
}

#[cfg(not(feature = "direct-syscall"))]
pub unsafe fn io_uring_enter(
    fd: c_int,
    to_submit: c_uint,
    min_complete: c_uint,
    flags: c_uint,
    arg: *const libc::c_void,
    size: usize,
) -> c_int {
    syscall(
        SYSCALL_ENTER,
        fd as c_long,
        to_submit as c_long,
        min_complete as c_long,
        flags as c_long,
        arg as c_long,
        size as c_long,
    ) as _
}

#[cfg(feature = "direct-syscall")]
pub unsafe fn io_uring_enter(
    fd: c_int,
    to_submit: c_uint,
    min_complete: c_uint,
    flags: c_uint,
    arg: *const libc::c_void,
    size: usize,
) -> c_int {
    sc::syscall6(
        SYSCALL_ENTER as usize,
        fd as usize,
        to_submit as usize,
        min_complete as usize,
        flags as usize,
        arg as usize,
        size,
    ) as _
}
