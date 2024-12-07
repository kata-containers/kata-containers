// Copyright (C) 2023 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use chrono::{DateTime, LocalResult, TimeZone, Utc};
use protobuf::well_known_types::timestamp::Timestamp;

pub fn datatime_to_timestamp(dt: DateTime<Utc>) -> Timestamp {
    let seconds = dt.timestamp();
    let nanos = dt.timestamp_subsec_nanos();

    Timestamp {
        seconds,
        nanos: nanos as i32,
        ..Default::default()
    }
}

#[allow(dead_code)]
pub fn timestamp_to_datetime(timestamp: Timestamp) -> Result<DateTime<Utc>> {
    let seconds = timestamp.seconds;
    let nanos = timestamp.nanos;

    match Utc.timestamp_opt(seconds, nanos as u32) {
        LocalResult::Single(t) => Ok(t),
        _ => Err(anyhow!("Utc.timestamp_opt {} fail", timestamp)),
    }
}
