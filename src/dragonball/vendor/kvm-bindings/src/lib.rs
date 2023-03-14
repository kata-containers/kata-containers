// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Rust FFI bindings to KVM, generated using [bindgen](https://crates.io/crates/bindgen).

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

#[macro_use]
#[cfg(feature = "fam-wrappers")]
extern crate vmm_sys_util;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use self::x86::*;

#[cfg(any(target_arch = "aarch", target_arch = "aarch64"))]
mod arm64;
#[cfg(any(target_arch = "aarch", target_arch = "aarch64"))]
pub use self::arm64::*;
