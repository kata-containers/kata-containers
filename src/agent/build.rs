// Copyright (c) 2021 ...
//
// SPDX-License-Identifier: Apache-2.0
//

extern crate rustc_version;

use rustc_version::{version, Version};
use std::io::{self, Write};
use std::process::exit;

const MIN_VERSION: &str = "1.49.0";

fn main() {
    let version = version().unwrap();
    let min_version = Version::parse(MIN_VERSION).unwrap();

    if version < min_version {
        panic!("Need rustc version {} or newer, got {}", MIN_VERSION, version);
    }
}
