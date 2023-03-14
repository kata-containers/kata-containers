// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use crate::cvt;
use crate::error::Result;
use crate::{ScmpArch, ScmpSyscall, ScmpVersion};
use libseccomp_sys::*;

/// Resets the libseccomp library's global state.
///
/// This function resets the (internal) global state of the libseccomp library,
/// this includes any notification file descriptors retrieved by
/// [`get_notify_fd`](crate::ScmpFilterContext::get_notify_fd).
/// Normally you do not need this but it may be required to continue using
/// the libseccomp library after a `fork()`/`clone()` to ensure the API level
/// and user notification state is properly reset.
///
/// This function corresponds to
/// [`seccomp_reset`](https://man7.org/linux/man-pages/man3/seccomp_reset.3.html).
///
/// # Errors
///
/// If the linked libseccomp library is older than v2.5.1 this function will
/// return an error.
///
/// # Examples
///
/// ```
/// # use libseccomp::*;
/// # if check_version(ScmpVersion::from((2, 5, 1)))? {
/// reset_global_state()?;
/// # }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn reset_global_state() -> Result<()> {
    cvt(unsafe { seccomp_reset(std::ptr::null_mut(), 0) })
}

/// Retrieves the name of a syscall from its number for a given architecture.
///
/// This function returns a string containing the name of the syscall.
///
/// # Arguments
///
/// * `arch` - A valid architecture token
/// * `syscall` - The number of syscall
///
/// # Errors
///
/// If the syscall is unrecognized or an issue occurs or an issue is
/// encountered getting the name of the syscall, an error will be returned.
#[deprecated(since = "0.2.3", note = "Use ScmpSyscall::get_name_by_arch instead.")]
pub fn get_syscall_name_from_arch(arch: ScmpArch, syscall: i32) -> Result<String> {
    ScmpSyscall::from_sys(syscall).get_name_by_arch(arch)
}

/// Gets the number of a syscall by name for a given architecture's ABI.
///
/// This function returns the number of the syscall.
///
/// # Arguments
///
/// * `name` - The name of a syscall
/// * `arch` - An architecture token as `Option` type
/// If arch argument is `None`, the functions returns the number of a syscall
/// on the kernel's native architecture.
///
/// # Errors
///
/// If an invalid string for the syscall name is specified or a syscall with that
/// name is not found, an error will be returned.
#[deprecated(since = "0.2.3", note = "Use ScmpSyscall::from_name* instead.")]
pub fn get_syscall_from_name(name: &str, arch: Option<ScmpArch>) -> Result<i32> {
    Ok(ScmpSyscall::from_name_by_arch(name, arch.unwrap_or(ScmpArch::Native))?.to_sys())
}

/// Deprecated alias for [`ScmpVersion::current()`].
#[deprecated(since = "0.2.0", note = "Use ScmpVersion::current().")]
pub fn get_library_version() -> Result<ScmpVersion> {
    ScmpVersion::current()
}
