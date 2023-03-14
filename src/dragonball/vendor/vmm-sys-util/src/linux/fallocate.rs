// Copyright 2019 Intel Corporation. All Rights Reserved.
//
// Copyright 2017 The Chromium OS Authors. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Enum and function for dealing with an allocated disk space
//! by [`fallocate`](http://man7.org/linux/man-pages/man2/fallocate.2.html).

use std::os::unix::io::AsRawFd;

use crate::errno::{errno_result, Error, Result};

/// Operation to be performed on a given range when calling [`fallocate`]
///
/// [`fallocate`]: fn.fallocate.html
pub enum FallocateMode {
    /// Deallocating file space.
    PunchHole,
    /// Zeroing file space.
    ZeroRange,
}

/// A safe wrapper for [`fallocate`](http://man7.org/linux/man-pages/man2/fallocate.2.html).
///
/// Manipulate the file space with specified operation parameters.
///
/// # Arguments
///
/// * `file`: the file to be manipulate.
/// * `mode`: specify the operation to be performed on the given range.
/// * `keep_size`: file size won't be changed even if `offset` + `len` is greater
/// than the file size.
/// * `offset`: the position that manipulates the file from.
/// * `size`: the bytes of the operation range.
///
/// # Examples
///
/// ```
/// extern crate vmm_sys_util;
/// # use std::fs::OpenOptions;
/// # use std::path::PathBuf;
/// use vmm_sys_util::fallocate::{fallocate, FallocateMode};
/// use vmm_sys_util::tempdir::TempDir;
///
/// let tempdir = TempDir::new_with_prefix("/tmp/fallocate_test").unwrap();
/// let mut path = PathBuf::from(tempdir.as_path());
/// path.push("file");
/// let mut f = OpenOptions::new()
///     .read(true)
///     .write(true)
///     .create(true)
///     .open(&path)
///     .unwrap();
/// fallocate(&f, FallocateMode::PunchHole, true, 0, 1).unwrap();
/// ```
pub fn fallocate(
    file: &dyn AsRawFd,
    mode: FallocateMode,
    keep_size: bool,
    offset: u64,
    len: u64,
) -> Result<()> {
    let offset = if offset > libc::off64_t::max_value() as u64 {
        return Err(Error::new(libc::EINVAL));
    } else {
        offset as libc::off64_t
    };

    let len = if len > libc::off64_t::max_value() as u64 {
        return Err(Error::new(libc::EINVAL));
    } else {
        len as libc::off64_t
    };

    let mut mode = match mode {
        FallocateMode::PunchHole => libc::FALLOC_FL_PUNCH_HOLE,
        FallocateMode::ZeroRange => libc::FALLOC_FL_ZERO_RANGE,
    };

    if keep_size {
        mode |= libc::FALLOC_FL_KEEP_SIZE;
    }

    // Safe since we pass in a valid fd and fallocate mode, validate offset and len,
    // and check the return value.
    let ret = unsafe { libc::fallocate64(file.as_raw_fd(), mode, offset, len) };
    if ret < 0 {
        errno_result()
    } else {
        Ok(())
    }
}
