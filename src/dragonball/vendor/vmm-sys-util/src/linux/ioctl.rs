// Copyright 2019 Intel Corporation. All Rights Reserved.
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Macros and functions for working with
//! [`ioctl`](http://man7.org/linux/man-pages/man2/ioctl.2.html).

use std::os::raw::{c_int, c_uint, c_ulong, c_void};
use std::os::unix::io::AsRawFd;

// The only reason
// [_IOC](https://elixir.bootlin.com/linux/v5.10.129/source/arch/alpha/include/uapi/asm/ioctl.h#L40)
// is a macro in C is because C doesn't have const functions, it is always better when possible to
// use a const function over a macro in Rust.
/// Function to calculate icotl number. Mimic of
/// [_IOC](https://elixir.bootlin.com/linux/v5.10.129/source/arch/alpha/include/uapi/asm/ioctl.h#L40)
/// ```
/// # use std::os::raw::c_uint;
/// # use vmm_sys_util::ioctl::{ioctl_expr, _IOC_NONE};
/// const KVMIO: c_uint = 0xAE;
/// ioctl_expr(_IOC_NONE, KVMIO, 0x01, 0);
/// ```
pub const fn ioctl_expr(
    dir: c_uint,
    ty: c_uint,
    nr: c_uint,
    size: c_uint,
) -> ::std::os::raw::c_ulong {
    (dir << crate::ioctl::_IOC_DIRSHIFT
        | ty << crate::ioctl::_IOC_TYPESHIFT
        | nr << crate::ioctl::_IOC_NRSHIFT
        | size << crate::ioctl::_IOC_SIZESHIFT) as ::std::os::raw::c_ulong
}

/// Declare a function that returns an ioctl number.
///
/// ```
/// # #[macro_use] extern crate vmm_sys_util;
/// # use std::os::raw::c_uint;
/// use vmm_sys_util::ioctl::_IOC_NONE;
///
/// const KVMIO: c_uint = 0xAE;
/// ioctl_ioc_nr!(KVM_CREATE_VM, _IOC_NONE, KVMIO, 0x01, 0);
/// ```
#[macro_export]
macro_rules! ioctl_ioc_nr {
    ($name:ident, $dir:expr, $ty:expr, $nr:expr, $size:expr) => {
        #[allow(non_snake_case)]
        #[allow(clippy::cast_lossless)]
        pub fn $name() -> ::std::os::raw::c_ulong {
            $crate::ioctl::ioctl_expr($dir, $ty, $nr, $size)
        }
    };
    ($name:ident, $dir:expr, $ty:expr, $nr:expr, $size:expr, $($v:ident),+) => {
        #[allow(non_snake_case)]
        #[allow(clippy::cast_lossless)]
        pub fn $name($($v: ::std::os::raw::c_uint),+) -> ::std::os::raw::c_ulong {
            $crate::ioctl::ioctl_expr($dir, $ty, $nr, $size)
        }
    };
}

/// Declare an ioctl that transfers no data.
///
/// ```
/// # #[macro_use] extern crate vmm_sys_util;
/// # use std::os::raw::c_uint;
/// const KVMIO: c_uint = 0xAE;
/// ioctl_io_nr!(KVM_CREATE_VM, KVMIO, 0x01);
/// ```
#[macro_export]
macro_rules! ioctl_io_nr {
    ($name:ident, $ty:expr, $nr:expr) => {
        ioctl_ioc_nr!($name, $crate::ioctl::_IOC_NONE, $ty, $nr, 0);
    };
    ($name:ident, $ty:expr, $nr:expr, $($v:ident),+) => {
        ioctl_ioc_nr!($name, $crate::ioctl::_IOC_NONE, $ty, $nr, 0, $($v),+);
    };
}

/// Declare an ioctl that reads data.
///
/// ```
/// # #[macro_use] extern crate vmm_sys_util;
/// const TUNTAP: ::std::os::raw::c_uint = 0x54;
/// ioctl_ior_nr!(TUNGETFEATURES, TUNTAP, 0xcf, ::std::os::raw::c_uint);
/// ```
#[macro_export]
macro_rules! ioctl_ior_nr {
    ($name:ident, $ty:expr, $nr:expr, $size:ty) => {
        ioctl_ioc_nr!(
            $name,
            $crate::ioctl::_IOC_READ,
            $ty,
            $nr,
            ::std::mem::size_of::<$size>() as u32
        );
    };
    ($name:ident, $ty:expr, $nr:expr, $size:ty, $($v:ident),+) => {
        ioctl_ioc_nr!(
            $name,
            $crate::ioctl::_IOC_READ,
            $ty,
            $nr,
            ::std::mem::size_of::<$size>() as u32,
            $($v),+
        );
    };
}

/// Declare an ioctl that writes data.
///
/// ```
/// # #[macro_use] extern crate vmm_sys_util;
/// const TUNTAP: ::std::os::raw::c_uint = 0x54;
/// ioctl_iow_nr!(TUNSETQUEUE, TUNTAP, 0xd9, ::std::os::raw::c_int);
/// ```
#[macro_export]
macro_rules! ioctl_iow_nr {
    ($name:ident, $ty:expr, $nr:expr, $size:ty) => {
        ioctl_ioc_nr!(
            $name,
            $crate::ioctl::_IOC_WRITE,
            $ty,
            $nr,
            ::std::mem::size_of::<$size>() as u32
        );
    };
    ($name:ident, $ty:expr, $nr:expr, $size:ty, $($v:ident),+) => {
        ioctl_ioc_nr!(
            $name,
            $crate::ioctl::_IOC_WRITE,
            $ty,
            $nr,
            ::std::mem::size_of::<$size>() as u32,
            $($v),+
        );
    };
}

/// Declare an ioctl that reads and writes data.
///
/// ```
/// # #[macro_use] extern crate vmm_sys_util;
/// const VHOST: ::std::os::raw::c_uint = 0xAF;
/// ioctl_iowr_nr!(VHOST_GET_VRING_BASE, VHOST, 0x12, ::std::os::raw::c_int);
/// ```
#[macro_export]
macro_rules! ioctl_iowr_nr {
    ($name:ident, $ty:expr, $nr:expr, $size:ty) => {
        ioctl_ioc_nr!(
            $name,
            $crate::ioctl::_IOC_READ | $crate::ioctl::_IOC_WRITE,
            $ty,
            $nr,
            ::std::mem::size_of::<$size>() as u32
        );
    };
    ($name:ident, $ty:expr, $nr:expr, $size:ty, $($v:ident),+) => {
        ioctl_ioc_nr!(
            $name,
            $crate::ioctl::_IOC_READ | $crate::ioctl::_IOC_WRITE,
            $ty,
            $nr,
            ::std::mem::size_of::<$size>() as u32,
            $($v),+
        );
    };
}

// Define IOC_* constants in a module so that we can allow missing docs on it.
// There is not much value in documenting these as it is code generated from
// kernel definitions.
#[allow(missing_docs)]
mod ioc {
    use std::os::raw::c_uint;

    pub const _IOC_NRBITS: c_uint = 8;
    pub const _IOC_TYPEBITS: c_uint = 8;
    pub const _IOC_SIZEBITS: c_uint = 14;
    pub const _IOC_DIRBITS: c_uint = 2;
    pub const _IOC_NRMASK: c_uint = 255;
    pub const _IOC_TYPEMASK: c_uint = 255;
    pub const _IOC_SIZEMASK: c_uint = 16383;
    pub const _IOC_DIRMASK: c_uint = 3;
    pub const _IOC_NRSHIFT: c_uint = 0;
    pub const _IOC_TYPESHIFT: c_uint = 8;
    pub const _IOC_SIZESHIFT: c_uint = 16;
    pub const _IOC_DIRSHIFT: c_uint = 30;
    pub const _IOC_NONE: c_uint = 0;
    pub const _IOC_WRITE: c_uint = 1;
    pub const _IOC_READ: c_uint = 2;
    pub const IOC_IN: c_uint = 1_073_741_824;
    pub const IOC_OUT: c_uint = 2_147_483_648;
    pub const IOC_INOUT: c_uint = 3_221_225_472;
    pub const IOCSIZE_MASK: c_uint = 1_073_676_288;
    pub const IOCSIZE_SHIFT: c_uint = 16;
}
pub use self::ioc::*;

// The type of the `req` parameter is different for the `musl` library. This will enable
// successful build for other non-musl libraries.
#[cfg(target_env = "musl")]
type IoctlRequest = c_int;
#[cfg(all(not(target_env = "musl"), not(target_os = "android")))]
type IoctlRequest = c_ulong;
#[cfg(all(not(target_env = "musl"), target_os = "android"))]
type IoctlRequest = c_int;
/// Run an [`ioctl`](http://man7.org/linux/man-pages/man2/ioctl.2.html)
/// with no arguments.
///
/// # Arguments
///
/// * `fd`: an open file descriptor corresponding to the device on which
/// to call the ioctl.
/// * `req`: a device-dependent request code.
///
/// # Safety
///
/// The caller should ensure to pass a valid file descriptor and have the
/// return value checked.
///
/// # Examples
///
/// ```
/// # extern crate libc;
/// # #[macro_use] extern crate vmm_sys_util;
/// #
/// # use libc::{open, O_CLOEXEC, O_RDWR};
/// # use std::fs::File;
/// # use std::os::raw::{c_char, c_uint};
/// # use std::os::unix::io::FromRawFd;
/// use vmm_sys_util::ioctl::ioctl;
///
/// const KVMIO: c_uint = 0xAE;
/// const KVM_API_VERSION: u32 = 12;
/// ioctl_io_nr!(KVM_GET_API_VERSION, KVMIO, 0x00);
///
/// let open_flags = O_RDWR | O_CLOEXEC;
/// let kvm_fd = unsafe { open("/dev/kvm\0".as_ptr() as *const c_char, open_flags) };
///
/// let ret = unsafe { ioctl(&File::from_raw_fd(kvm_fd), KVM_GET_API_VERSION()) };
///
/// assert_eq!(ret as u32, KVM_API_VERSION);
/// ```
pub unsafe fn ioctl<F: AsRawFd>(fd: &F, req: c_ulong) -> c_int {
    libc::ioctl(fd.as_raw_fd(), req as IoctlRequest, 0)
}

/// Run an [`ioctl`](http://man7.org/linux/man-pages/man2/ioctl.2.html)
/// with a single value argument.
///
/// # Arguments
///
/// * `fd`: an open file descriptor corresponding to the device on which
/// to call the ioctl.
/// * `req`: a device-dependent request code.
/// * `arg`: a single value passed to ioctl.
///
/// # Safety
///
/// The caller should ensure to pass a valid file descriptor and have the
/// return value checked.
///
/// # Examples
///
/// ```
/// # extern crate libc;
/// # #[macro_use] extern crate vmm_sys_util;
/// # use libc::{open, O_CLOEXEC, O_RDWR};
/// # use std::fs::File;
/// # use std::os::raw::{c_char, c_uint, c_ulong};
/// # use std::os::unix::io::FromRawFd;
/// use vmm_sys_util::ioctl::ioctl_with_val;
///
/// const KVMIO: c_uint = 0xAE;
/// const KVM_CAP_USER_MEMORY: u32 = 3;
/// ioctl_io_nr!(KVM_CHECK_EXTENSION, KVMIO, 0x03);
///
/// let open_flags = O_RDWR | O_CLOEXEC;
/// let kvm_fd = unsafe { open("/dev/kvm\0".as_ptr() as *const c_char, open_flags) };
///
/// let ret = unsafe {
///     ioctl_with_val(
///         &File::from_raw_fd(kvm_fd),
///         KVM_CHECK_EXTENSION(),
///         KVM_CAP_USER_MEMORY as c_ulong,
///     )
/// };
/// assert!(ret > 0);
/// ```
pub unsafe fn ioctl_with_val<F: AsRawFd>(fd: &F, req: c_ulong, arg: c_ulong) -> c_int {
    libc::ioctl(fd.as_raw_fd(), req as IoctlRequest, arg)
}

/// Run an [`ioctl`](http://man7.org/linux/man-pages/man2/ioctl.2.html)
/// with an immutable reference.
///
/// # Arguments
///
/// * `fd`: an open file descriptor corresponding to the device on which
/// to call the ioctl.
/// * `req`: a device-dependent request code.
/// * `arg`: an immutable reference passed to ioctl.
///
/// # Safety
///
/// The caller should ensure to pass a valid file descriptor and have the
/// return value checked.
pub unsafe fn ioctl_with_ref<F: AsRawFd, T>(fd: &F, req: c_ulong, arg: &T) -> c_int {
    libc::ioctl(
        fd.as_raw_fd(),
        req as IoctlRequest,
        arg as *const T as *const c_void,
    )
}

/// Run an [`ioctl`](http://man7.org/linux/man-pages/man2/ioctl.2.html)
/// with a mutable reference.
///
/// # Arguments
///
/// * `fd`: an open file descriptor corresponding to the device on which
/// to call the ioctl.
/// * `req`: a device-dependent request code.
/// * `arg`: a mutable reference passed to ioctl.
///
/// # Safety
///
/// The caller should ensure to pass a valid file descriptor and have the
/// return value checked.
pub unsafe fn ioctl_with_mut_ref<F: AsRawFd, T>(fd: &F, req: c_ulong, arg: &mut T) -> c_int {
    libc::ioctl(
        fd.as_raw_fd(),
        req as IoctlRequest,
        arg as *mut T as *mut c_void,
    )
}

/// Run an [`ioctl`](http://man7.org/linux/man-pages/man2/ioctl.2.html)
/// with a raw pointer.
///
/// # Arguments
///
/// * `fd`: an open file descriptor corresponding to the device on which
/// to call the ioctl.
/// * `req`: a device-dependent request code.
/// * `arg`: a raw pointer passed to ioctl.
///
/// # Safety
///
/// The caller should ensure to pass a valid file descriptor and have the
/// return value checked.
pub unsafe fn ioctl_with_ptr<F: AsRawFd, T>(fd: &F, req: c_ulong, arg: *const T) -> c_int {
    libc::ioctl(fd.as_raw_fd(), req as IoctlRequest, arg as *const c_void)
}

/// Run an [`ioctl`](http://man7.org/linux/man-pages/man2/ioctl.2.html)
/// with a mutable raw pointer.
///
/// # Arguments
///
/// * `fd`: an open file descriptor corresponding to the device on which
/// to call the ioctl.
/// * `req`: a device-dependent request code.
/// * `arg`: a mutable raw pointer passed to ioctl.
///
/// # Safety
///
/// The caller should ensure to pass a valid file descriptor and have the
/// return value checked.
pub unsafe fn ioctl_with_mut_ptr<F: AsRawFd, T>(fd: &F, req: c_ulong, arg: *mut T) -> c_int {
    libc::ioctl(fd.as_raw_fd(), req as IoctlRequest, arg as *mut c_void)
}

#[cfg(test)]
mod tests {
    const TUNTAP: ::std::os::raw::c_uint = 0x54;
    const VHOST: ::std::os::raw::c_uint = 0xAF;
    const EVDEV: ::std::os::raw::c_uint = 0x45;

    const KVMIO: ::std::os::raw::c_uint = 0xAE;

    ioctl_io_nr!(KVM_CREATE_VM, KVMIO, 0x01);
    ioctl_ior_nr!(TUNGETFEATURES, TUNTAP, 0xcf, ::std::os::raw::c_uint);
    ioctl_iow_nr!(TUNSETQUEUE, TUNTAP, 0xd9, ::std::os::raw::c_int);
    ioctl_io_nr!(VHOST_SET_OWNER, VHOST, 0x01);
    ioctl_iowr_nr!(VHOST_GET_VRING_BASE, VHOST, 0x12, ::std::os::raw::c_int);
    ioctl_iowr_nr!(KVM_GET_MSR_INDEX_LIST, KVMIO, 0x2, ::std::os::raw::c_int);

    ioctl_ior_nr!(EVIOCGBIT, EVDEV, 0x20 + evt, [u8; 128], evt);
    ioctl_io_nr!(FAKE_IOCTL_2_ARG, EVDEV, 0x01 + x + y, x, y);

    #[test]
    fn test_ioctl_macros() {
        assert_eq!(0x0000_AE01, KVM_CREATE_VM());
        assert_eq!(0x0000_AF01, VHOST_SET_OWNER());
        assert_eq!(0x8004_54CF, TUNGETFEATURES());
        assert_eq!(0x4004_54D9, TUNSETQUEUE());
        assert_eq!(0xC004_AE02, KVM_GET_MSR_INDEX_LIST());
        assert_eq!(0xC004_AF12, VHOST_GET_VRING_BASE());

        assert_eq!(0x8080_4522, EVIOCGBIT(2));
        assert_eq!(0x0000_4509, FAKE_IOCTL_2_ARG(3, 5));
    }
}
