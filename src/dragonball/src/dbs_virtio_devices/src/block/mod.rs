// Copyright 2019-2020 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

mod device;
pub use self::device::*;
mod handler;
pub(crate) use self::handler::*;
mod request;
pub(crate) use self::request::*;
mod ufile;
pub use self::ufile::*;

use dbs_utils::rate_limiter::BucketUpdate;

/// Block deriver name.
pub const BLK_DRIVER_NAME: &str = "virtio-blk";

pub(crate) const SECTOR_SHIFT: u8 = 9;
/// The size of sector
pub const SECTOR_SIZE: u64 = (0x01u64) << (SECTOR_SHIFT as u64);

pub(crate) enum KillEvent {
    Kill,
    BucketUpdate(BucketUpdate, BucketUpdate),
}
