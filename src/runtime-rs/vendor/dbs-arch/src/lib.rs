// Copyright 2021-2022 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![deny(missing_docs)]

//! CPU architecture specific constants, structures and utilities.
//!
//! This crate provides CPU architecture specific constants, structures and utilities to abstract
//! away CPU architecture specific details from the Dragonball Secure Sandbox or other VMMs.
//!
//! # Supported CPU Architectures
//! - **x86_64**: x86_64 (also known as x64, x86-64, AMD64, and Intel 64) is a 64-bit
//!   version of the x86 instruction set.
//! - **ARM64**: AArch64 or ARM64 is the 64-bit extension of the ARM architecture.

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::*;

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
pub use aarch64::*;
