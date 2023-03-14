// Copyright 2020 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Error handling utilities and helpers for Nydus.
//!
//! The `nydus-error` crate provides commonly used error handling utilities and helpers for Nydus,
//! including:
//! - [`fn make_error()`](error.fn.make_error.html): display error messages with line number,
//!   file path and optional backtrace.
//! - Macros for commonly used error code, such as `einval!()`, `enosys!()` etc.
//! - [`struct ErrorHolder`](logger.struct.ErrorHolder.html): a circular ring buffer to hold latest
//!   error messages.

#[macro_use]
extern crate log;

#[macro_use]
pub mod error;

pub mod logger;
