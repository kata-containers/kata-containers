// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

//! Shared OVSDB port operations and OVN binding checks.
//!
//! Calls use a Unix socket and block; async callers need a blocking worker.

mod binding;
mod client;
mod error;
mod ovsdb;
mod port;

pub use binding::{CreateOutcome, OvsBinding};
pub use client::{OvsDbClient, UnixOvsDbClient};
pub use error::OvsError;
pub use ovsdb::OvsDbControl;
pub use port::{OvsControl, OvsPortInfo, OvsPortSpec};
