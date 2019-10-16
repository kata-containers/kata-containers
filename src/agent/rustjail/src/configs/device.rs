// Copyright (c) 2019 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use libc::*;
use serde;
#[macro_use]
use serde_derive;
use serde_json;

#[derive(Serialize, Deserialize, Debug)]
pub struct Device {
    #[serde(default)]
    r#type: char,
    #[serde(default)]
    path: String,
    #[serde(default)]
    major: i64,
    #[serde(default)]
    minor: i64,
    #[serde(default)]
    permissions: String,
    #[serde(default)]
    file_mode: mode_t,
    #[serde(default)]
    uid: i32,
    #[serde(default)]
    gid: i32,
    #[serde(default)]
    allow: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BlockIODevice {
    #[serde(default)]
    major: i64,
    #[serde(default)]
    minor: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct WeightDevice {
    block: BlockIODevice,
    #[serde(default)]
    weight: u16,
    #[serde(default, rename = "leafWeight")]
    leaf_weight: u16,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ThrottleDevice {
    block: BlockIODevice,
    #[serde(default)]
    rate: u64,
}
