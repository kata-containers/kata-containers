// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::variant::Variant;
use crate::version::Version;

/// Structure that contains the decoded data.
#[derive(Debug, Eq, PartialEq)]
pub struct Decoded {
    /// The variant.
    pub variant: Variant,

    /// The version.
    pub version: Version,

    /// The amount of memory requested (KiB).
    pub mem_cost: u32,

    /// The number of passes.
    pub time_cost: u32,

    /// The parallelism.
    pub parallelism: u32,

    /// The salt.
    pub salt: Vec<u8>,

    /// The hash.
    pub hash: Vec<u8>,
}
