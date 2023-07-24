// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

#![deny(missing_docs)]
//! # Network-related utilities
//!
//! Provides tools for representing and handling network related concepts like MAC addresses and
//! network interfaces.

mod mac;
pub use self::mac::{MacAddr, MAC_ADDR_LEN};

mod tap;
pub use self::tap::{Error as TapError, Tap};

pub mod net_gen;
