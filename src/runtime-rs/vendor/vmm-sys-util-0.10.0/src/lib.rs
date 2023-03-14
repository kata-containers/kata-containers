// Copyright 2019 Intel Corporation. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Collection of modules that provides helpers and utilities used by multiple
//! [rust-vmm](https://github.com/rust-vmm/community) components.

#![deny(missing_docs)]

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub use crate::linux::*;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use crate::unix::*;

pub mod errno;
pub mod fam;
pub mod metric;
pub mod rand;
pub mod syscall;
pub mod tempfile;
